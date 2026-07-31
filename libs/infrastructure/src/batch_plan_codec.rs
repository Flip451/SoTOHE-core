//! Serde codec for `batch-plan.json` (the Phase 3 batch plan).
//!
//! Decode only: the impl-planner authors the artifact, and every consumer here
//! reads it. All DTOs carry `deny_unknown_fields` to enforce the schema
//! boundary at every nesting level, and each field is typed as the domain
//! concept it stands for through a field deserializer, so no bare `String` or
//! `u32` reaches the arithmetic. Every file-internal invariant is delegated to
//! `BatchPlanDocument::new` rather than re-implemented here (IN-05, AC-04).

use domain::batch_plan::{
    BatchDeclaration, BatchId, BatchPlanDocument, BatchPlanValidationError,
    IndivisibilityJustification, LineCount, ScopeLineEstimate, TaskDecomposition, TaskEstimate,
};
use domain::review_v2::{MainScopeName, ScopeName};
use domain::{FreeText, TaskId, TrackId};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

/// The `schema_version` this codec accepts.
pub const BATCH_PLAN_SCHEMA_VERSION: u32 = 1;

/// The reserved wire spelling of the unnamed review scope.
const OTHER_SCOPE: &str = "other";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Rejection produced while decoding `batch-plan.json` (IN-05, AC-04).
#[derive(Debug, thiserror::Error)]
pub enum BatchPlanCodecError {
    /// The text is not the JSON this schema describes.
    #[error("batch-plan.json could not be parsed: {message}")]
    InvalidJson {
        /// Opaque parser diagnostic.
        message: FreeText,
    },
    /// The document declares a schema version this codec does not read.
    #[error(
        "unsupported batch-plan schema_version: expected {BATCH_PLAN_SCHEMA_VERSION}, got {found}"
    )]
    UnsupportedSchemaVersion {
        /// The version the document declared.
        found: u32,
    },
    /// The document parsed, but the domain refused it.
    #[error("batch-plan.json is not a valid batch plan: {source}")]
    InvalidDocument {
        /// The domain rejection, carried unchanged.
        source: BatchPlanValidationError,
    },
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Minimal envelope used to select the strict DTO after version validation.
#[derive(Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: u32,
    #[serde(flatten)]
    _remaining_fields: Map<String, Value>,
}

/// Deserialization mirror of `batch-plan.json` (IN-01, IN-05, AC-01, AC-04).
///
/// `schema_version` lives here and nowhere else: the domain document carries no
/// wire-format metadata.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchPlanDocumentDto {
    /// The declared schema version.
    pub schema_version: u32,
    /// The track the plan belongs to.
    #[serde(deserialize_with = "deserialize_track_id")]
    pub track_id: TrackId,
    /// One estimate per planned task.
    pub task_estimates: Vec<TaskEstimateDto>,
    /// The ordered batches, declaring member task ids only.
    pub batches: Vec<BatchDeclarationDto>,
}

/// Deserialization mirror of one `task_estimates` entry (IN-02, IN-04, IN-05,
/// AC-02, AC-03).
///
/// `oversize_justification` keeps the null-or-string wire shape; an omitted key
/// reads the same as an explicit `null`. [`decode`] converts it into the
/// domain's two-state decomposition, which cannot represent an indivisible task
/// without a reason.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEstimateDto {
    /// The estimated task.
    #[serde(deserialize_with = "deserialize_task_id")]
    pub task_id: TaskId,
    /// The per-scope figures the task declares.
    pub scope_estimates: Vec<ScopeLineEstimateDto>,
    /// Why the task cannot be split, when it declares that it cannot.
    #[serde(default, deserialize_with = "deserialize_optional_justification")]
    pub oversize_justification: Option<IndivisibilityJustification>,
}

/// Deserialization mirror of one per-scope estimate (IN-02, IN-05, AC-02).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeLineEstimateDto {
    /// The review scope estimated.
    #[serde(deserialize_with = "deserialize_scope_name")]
    pub scope: ScopeName,
    /// The declared production-code figure.
    #[serde(deserialize_with = "deserialize_line_count")]
    pub production_lines: LineCount,
    /// The declared test-code figure.
    #[serde(deserialize_with = "deserialize_line_count")]
    pub test_lines: LineCount,
}

/// Deserialization mirror of one `batches` entry (IN-03, IN-05, AC-03, CN-01).
///
/// No line figures on the wire: a batch's per-scope total is derived from its
/// members.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchDeclarationDto {
    /// The batch identifier.
    #[serde(deserialize_with = "deserialize_batch_id")]
    pub id: BatchId,
    /// The member task ids, in declaration order.
    #[serde(deserialize_with = "deserialize_task_ids")]
    pub task_ids: Vec<TaskId>,
}

// ---------------------------------------------------------------------------
// Decode: JSON -> domain
// ---------------------------------------------------------------------------

/// Decodes `batch-plan.json` text into the validated domain document.
///
/// # Errors
///
/// Returns [`BatchPlanCodecError::InvalidJson`] when the text does not parse,
/// [`BatchPlanCodecError::UnsupportedSchemaVersion`] when the declared version
/// is not the one this codec reads, and
/// [`BatchPlanCodecError::InvalidDocument`] when the domain refuses the
/// assembled plan.
pub fn decode(json: &str) -> Result<BatchPlanDocument, BatchPlanCodecError> {
    let envelope: SchemaVersionEnvelope = serde_json::from_str(json).map_err(|error| {
        BatchPlanCodecError::InvalidJson { message: FreeText::new(error.to_string()) }
    })?;

    if envelope.schema_version != BATCH_PLAN_SCHEMA_VERSION {
        return Err(BatchPlanCodecError::UnsupportedSchemaVersion {
            found: envelope.schema_version,
        });
    }

    let dto: BatchPlanDocumentDto = serde_json::from_str(json).map_err(|error| {
        BatchPlanCodecError::InvalidJson { message: FreeText::new(error.to_string()) }
    })?;

    let task_estimates = dto
        .task_estimates
        .into_iter()
        .map(task_estimate_from_dto)
        .collect::<Result<Vec<_>, _>>()?;
    let batches =
        dto.batches.into_iter().map(batch_declaration_from_dto).collect::<Result<Vec<_>, _>>()?;

    BatchPlanDocument::new(dto.track_id, task_estimates, batches)
        .map_err(|source| BatchPlanCodecError::InvalidDocument { source })
}

fn task_estimate_from_dto(dto: TaskEstimateDto) -> Result<TaskEstimate, BatchPlanCodecError> {
    let scope_estimates = dto
        .scope_estimates
        .into_iter()
        .map(|estimate| {
            ScopeLineEstimate::new(estimate.scope, estimate.production_lines, estimate.test_lines)
        })
        .collect();
    let decomposition = match dto.oversize_justification {
        Some(justification) => TaskDecomposition::Indivisible(justification),
        None => TaskDecomposition::Decomposable,
    };

    TaskEstimate::new(dto.task_id, scope_estimates, decomposition)
        .map_err(|source| BatchPlanCodecError::InvalidDocument { source })
}

fn batch_declaration_from_dto(
    dto: BatchDeclarationDto,
) -> Result<BatchDeclaration, BatchPlanCodecError> {
    BatchDeclaration::new(dto.id, dto.task_ids)
        .map_err(|source| BatchPlanCodecError::InvalidDocument { source })
}

// ---------------------------------------------------------------------------
// Field deserializers: wire text -> domain concepts
// ---------------------------------------------------------------------------

fn deserialize_track_id<'de, D>(deserializer: D) -> Result<TrackId, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    TrackId::try_new(raw).map_err(D::Error::custom)
}

fn deserialize_task_id<'de, D>(deserializer: D) -> Result<TaskId, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    TaskId::try_new(raw).map_err(D::Error::custom)
}

fn deserialize_task_ids<'de, D>(deserializer: D) -> Result<Vec<TaskId>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<String>::deserialize(deserializer)?;
    raw.into_iter().map(|id| TaskId::try_new(id).map_err(D::Error::custom)).collect()
}

fn deserialize_batch_id<'de, D>(deserializer: D) -> Result<BatchId, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    BatchId::try_new(&raw).map_err(D::Error::custom)
}

fn deserialize_scope_name<'de, D>(deserializer: D) -> Result<ScopeName, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    if raw == OTHER_SCOPE {
        return Ok(ScopeName::Other);
    }
    MainScopeName::new(raw).map(ScopeName::Main).map_err(D::Error::custom)
}

fn deserialize_line_count<'de, D>(deserializer: D) -> Result<LineCount, D::Error>
where
    D: Deserializer<'de>,
{
    u32::deserialize(deserializer).map(LineCount::new)
}

fn deserialize_optional_justification<'de, D>(
    deserializer: D,
) -> Result<Option<IndivisibilityJustification>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    raw.map(|text| IndivisibilityJustification::try_new(&text).map_err(D::Error::custom))
        .transpose()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{BatchPlanCodecError, decode};
    use domain::TaskId;
    use domain::batch_plan::{BatchPlanValidationError, LineCount};
    use domain::review_v2::{MainScopeName, ScopeName};

    const VALID_JSON: &str = r#"{
      "schema_version": 1,
      "track_id": "scope-diff-ceiling-admission-enforcement-2026-07-29",
      "task_estimates": [
        {
          "task_id": "T001",
          "scope_estimates": [
            {"scope": "domain", "production_lines": 100, "test_lines": 50}
          ],
          "oversize_justification": null
        },
        {
          "task_id": "T002",
          "scope_estimates": [
            {"scope": "usecase", "production_lines": 700, "test_lines": 300},
            {"scope": "other", "production_lines": 5, "test_lines": 0}
          ],
          "oversize_justification": "the transition table cannot be split"
        }
      ],
      "batches": [
        {"id": "B1", "task_ids": ["T001"]},
        {"id": "B2", "task_ids": ["T002"]}
      ]
    }"#;

    fn scope(name: &str) -> ScopeName {
        ScopeName::Main(MainScopeName::new(name).unwrap())
    }

    #[test]
    fn test_decode_reads_a_declared_batch_plan_into_the_domain_document() {
        let plan = decode(VALID_JSON).unwrap();

        assert_eq!(plan.track_id().as_ref(), "scope-diff-ceiling-admission-enforcement-2026-07-29");
        assert_eq!(plan.task_estimates().len(), 2);
        assert_eq!(
            plan.batches().iter().map(|batch| batch.id().as_str()).collect::<Vec<_>>(),
            vec!["B1", "B2"]
        );
        let first = plan.estimate_for(&TaskId::try_new("T001").unwrap()).unwrap();
        let domain_estimate = first.estimate_for(&scope("domain")).unwrap();
        assert_eq!(domain_estimate.production_lines(), LineCount::new(100));
        assert_eq!(domain_estimate.test_lines(), LineCount::new(50));
    }

    #[test]
    fn test_decode_reads_the_reserved_other_scope_name() {
        let plan = decode(VALID_JSON).unwrap();

        let second = plan.estimate_for(&TaskId::try_new("T002").unwrap()).unwrap();
        assert_eq!(
            second.estimate_for(&ScopeName::Other).map(|estimate| estimate.total()),
            Some(LineCount::new(5))
        );
    }

    #[test]
    fn test_decode_reads_a_null_justification_as_a_decomposable_task() {
        let plan = decode(VALID_JSON).unwrap();

        let decomposable = plan.estimate_for(&TaskId::try_new("T001").unwrap()).unwrap();
        assert!(!decomposable.decomposition().is_indivisible());
        assert!(decomposable.decomposition().justification().is_none());
    }

    #[test]
    fn test_decode_reads_a_stated_justification_as_an_indivisible_task() {
        let plan = decode(VALID_JSON).unwrap();

        let indivisible = plan.estimate_for(&TaskId::try_new("T002").unwrap()).unwrap();
        assert!(indivisible.decomposition().is_indivisible());
        assert_eq!(
            indivisible.decomposition().justification().map(|reason| reason.as_str()),
            Some("the transition table cannot be split")
        );
    }

    #[test]
    fn test_decode_reads_an_omitted_justification_the_same_as_an_explicit_null() {
        // The estimate names a scope: the omitted justification is what this case
        // is about, and an estimate naming none is refused outright.
        let json = r#"{
          "schema_version": 1,
          "track_id": "some-track",
          "task_estimates": [
            {"task_id": "T001", "scope_estimates": [
              {"scope": "domain", "production_lines": 10, "test_lines": 5}
            ]}
          ],
          "batches": [{"id": "B1", "task_ids": ["T001"]}]
        }"#;

        let plan = decode(json).unwrap();

        assert!(
            !plan
                .estimate_for(&TaskId::try_new("T001").unwrap())
                .unwrap()
                .decomposition()
                .is_indivisible()
        );
    }

    #[test]
    fn test_decode_rejects_an_estimate_that_names_no_scope() {
        let json = r#"{
          "schema_version": 1,
          "track_id": "some-track",
          "task_estimates": [
            {"task_id": "T001", "scope_estimates": []}
          ],
          "batches": [{"id": "B1", "task_ids": ["T001"]}]
        }"#;

        let err = decode(json).unwrap_err();

        // The domain rejection travels out unchanged: the codec re-implements no
        // invariant of its own for it.
        let BatchPlanCodecError::InvalidDocument { source } = err else {
            panic!("an estimate naming no scope must be refused: {err}");
        };
        assert!(
            matches!(
                &source,
                BatchPlanValidationError::EmptyScopeEstimates { task_id }
                    if task_id.as_ref() == "T001"
            ),
            "unexpected rejection: {source}"
        );
    }

    #[test]
    fn test_decode_rejects_a_schema_version_it_does_not_read() {
        let json = VALID_JSON.replace("\"schema_version\": 1", "\"schema_version\": 9");

        let err = decode(&json).unwrap_err();

        assert!(
            matches!(err, BatchPlanCodecError::UnsupportedSchemaVersion { found: 9 }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_rejects_an_unsupported_schema_before_strictly_reading_its_fields() {
        let json = r#"{"schema_version": 9, "future_field": true}"#;

        let err = decode(json).unwrap_err();

        assert!(
            matches!(err, BatchPlanCodecError::UnsupportedSchemaVersion { found: 9 }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_rejects_text_that_is_not_the_declared_json() {
        let err = decode("{ not json").unwrap_err();

        assert!(matches!(err, BatchPlanCodecError::InvalidJson { .. }), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_rejects_an_unrecognised_field() {
        let json = VALID_JSON.replace("\"batches\":", "\"surprise\": 1, \"batches\":");

        let err = decode(&json).unwrap_err();

        assert!(matches!(err, BatchPlanCodecError::InvalidJson { .. }), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_refuses_a_value_that_is_not_the_identifier_it_stands_for() {
        let json = VALID_JSON.replace("\"task_id\": \"T001\"", "\"task_id\": \"not-a-task\"");

        let err = decode(&json).unwrap_err();

        assert!(matches!(err, BatchPlanCodecError::InvalidJson { .. }), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_reads_each_batch_member_list_into_domain_task_ids() {
        let plan = decode(VALID_JSON).unwrap();

        let first = plan.batches().first().unwrap();
        let second = plan.batches().get(1).unwrap();
        assert_eq!(first.id().as_str(), "B1");
        assert_eq!(first.task_ids(), &[TaskId::try_new("T001").unwrap()]);
        assert_eq!(second.id().as_str(), "B2");
        assert_eq!(second.task_ids(), &[TaskId::try_new("T002").unwrap()]);
    }

    #[test]
    fn test_decode_rejects_a_batch_entry_that_declares_a_line_figure() {
        let json = VALID_JSON.replace(
            "{\"id\": \"B1\", \"task_ids\": [\"T001\"]}",
            "{\"id\": \"B1\", \"task_ids\": [\"T001\"], \"lines\": 150}",
        );

        let err = decode(&json).unwrap_err();

        assert!(
            matches!(err, BatchPlanCodecError::InvalidJson { .. }),
            "a batch declares member ids only: {err}"
        );
    }

    #[test]
    fn test_decode_rejects_a_per_scope_estimate_it_cannot_read() {
        let unknown_key = VALID_JSON.replace(
            "{\"scope\": \"domain\", \"production_lines\": 100, \"test_lines\": 50}",
            "{\"scope\": \"domain\", \"production_lines\": 100, \"test_lines\": 50, \"doc_lines\": 5}",
        );
        let blank_scope = VALID_JSON.replace("\"scope\": \"domain\"", "\"scope\": \"\"");
        let unreadable_figure =
            VALID_JSON.replace("\"production_lines\": 100", "\"production_lines\": \"many\"");

        for json in [unknown_key, blank_scope, unreadable_figure] {
            let err = decode(&json).unwrap_err();
            assert!(
                matches!(err, BatchPlanCodecError::InvalidJson { .. }),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn test_decode_rejects_a_batch_member_with_no_declared_estimate() {
        let json = VALID_JSON.replace(
            "{\"id\": \"B1\", \"task_ids\": [\"T001\"]}",
            "{\"id\": \"B1\", \"task_ids\": [\"T001\", \"T003\"]}",
        );

        let err = decode(&json).unwrap_err();

        assert!(
            matches!(
                err,
                BatchPlanCodecError::InvalidDocument {
                    source: BatchPlanValidationError::MissingTaskEstimate { .. }
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_rejects_a_task_claimed_by_more_than_one_batch() {
        let json = VALID_JSON.replace(
            "{\"id\": \"B2\", \"task_ids\": [\"T002\"]}",
            "{\"id\": \"B2\", \"task_ids\": [\"T002\", \"T001\"]}",
        );

        let err = decode(&json).unwrap_err();

        assert!(
            matches!(
                err,
                BatchPlanCodecError::InvalidDocument {
                    source: BatchPlanValidationError::DuplicateBatchMembership { .. }
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_hands_a_file_internal_inconsistency_to_the_domain_unchanged() {
        // T002 is estimated but no batch claims it.
        let json = VALID_JSON.replace("{\"id\": \"B2\", \"task_ids\": [\"T002\"]}", "");
        let json = json.replace(
            "{\"id\": \"B1\", \"task_ids\": [\"T001\"]},",
            "{\"id\": \"B1\", \"task_ids\": [\"T001\"]}",
        );

        let err = decode(&json).unwrap_err();

        assert!(
            matches!(
                err,
                BatchPlanCodecError::InvalidDocument {
                    source: BatchPlanValidationError::UnassignedTask { .. }
                }
            ),
            "unexpected error: {err}"
        );
    }
}
