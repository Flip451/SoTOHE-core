//! Serde codec for `impl-plan.json` (ImplPlanDocument SSoT).
//!
//! Schema version 1: introduced by ADR 2026-04-19-1242 §D1.4.
//! Schema version 2 adds each task's declared dependencies; both versions are
//! read and version 2 is written, so an existing schema-1 plan keeps decoding
//! and simply declares no dependencies (IN-19, AC-05, AC-21).
//! All DTOs are defined locally with `deny_unknown_fields` to enforce
//! the strict schema boundary at every nesting level.

use domain::{
    CommitHash, DomainError, ImplPlanDocument, NonEmptyString, PlanSection, PlanView, TaskId,
    TaskStatus, TrackTask, ValidationError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The schema version [`encode`] emits.
const WRITTEN_SCHEMA_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for `impl-plan.json` serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum ImplPlanCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported schema_version: expected 1 or 2, got {0}")]
    UnsupportedSchemaVersion(u32),

    #[error("validation error: {0}")]
    Validation(String),
}

impl From<DomainError> for ImplPlanCodecError {
    fn from(e: DomainError) -> Self {
        Self::Validation(e.to_string())
    }
}

impl From<ValidationError> for ImplPlanCodecError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// DTOs (all with deny_unknown_fields for strict schema enforcement)
// ---------------------------------------------------------------------------

/// Version envelope used to select the strict DTO for the declared wire schema.
#[derive(Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: u32,
    #[serde(flatten)]
    _remaining_fields: Map<String, Value>,
}

/// Top-level DTO for schema-version 1 `impl-plan.json` documents.
///
/// Schema 1 has no dependency-declaration field, so its task DTO intentionally
/// does not include `depends_on`. This lets `deny_unknown_fields` reject a
/// schema-2 field that is incorrectly supplied under schema version 1.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImplPlanDocumentV1Dto {
    #[serde(rename = "schema_version")]
    _schema_version: u32,
    #[serde(default)]
    tasks: Vec<ImplPlanTaskV1Dto>,
    plan: ImplPlanPlanDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImplPlanTaskV1Dto {
    id: String,
    description: String,
    status: String,
    commit_hash: Option<String>,
}

impl From<ImplPlanTaskV1Dto> for ImplPlanTaskDto {
    fn from(dto: ImplPlanTaskV1Dto) -> Self {
        Self {
            id: dto.id,
            description: dto.description,
            status: dto.status,
            commit_hash: dto.commit_hash,
            depends_on: Vec::new(),
        }
    }
}

/// Top-level DTO for schema-version 2 `impl-plan.json` documents.
///
/// `deny_unknown_fields` rejects unrecognised fields at every nesting level
/// to enforce the schema contract at the codec boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplPlanDocumentDto {
    pub schema_version: u32,
    #[serde(default)]
    pub tasks: Vec<ImplPlanTaskDto>,
    pub plan: ImplPlanPlanDto,
}

/// DTO for a single task entry in `impl-plan.json`.
///
/// `deny_unknown_fields` rejects unrecognised fields to enforce the schema
/// contract at this nesting level.
///
/// `depends_on` is what schema 2 adds (IN-19, AC-21). Within schema 2, its serde
/// default makes an omitted list equivalent to an empty one; the empty case is
/// not written back out. Its element type is the domain task identifier through
/// a field codec rather than a raw `String`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplPlanTaskDto {
    pub id: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_task_ids",
        deserialize_with = "deserialize_task_ids"
    )]
    pub depends_on: Vec<TaskId>,
}

fn serialize_task_ids<S>(task_ids: &[TaskId], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.collect_seq(task_ids.iter().map(TaskId::as_ref))
}

fn deserialize_task_ids<'de, D>(deserializer: D) -> Result<Vec<TaskId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;

    let raw = Vec::<String>::deserialize(deserializer)?;
    raw.into_iter().map(|id| TaskId::try_new(id).map_err(D::Error::custom)).collect()
}

/// DTO for the `plan` field in `impl-plan.json`.
///
/// `deny_unknown_fields` rejects unrecognised fields to enforce the schema
/// contract at this nesting level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplPlanPlanDto {
    #[serde(default)]
    pub summary: Vec<String>,
    #[serde(default)]
    pub sections: Vec<ImplPlanSectionDto>,
}

/// DTO for a single section entry in `impl-plan.json`.
///
/// `deny_unknown_fields` rejects unrecognised fields to enforce the schema
/// contract at this nesting level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplPlanSectionDto {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Vec<String>,
    #[serde(default)]
    pub task_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Decode: JSON -> domain
// ---------------------------------------------------------------------------

/// Deserializes an `impl-plan.json` string into an [`ImplPlanDocument`].
///
/// Both schema versions are accepted: a schema-1 document declares no task
/// dependencies and reads as declaring none (IN-19, AC-05, AC-21).
///
/// # Errors
///
/// Returns `ImplPlanCodecError::Json` if the JSON is malformed.
/// Returns `ImplPlanCodecError::UnsupportedSchemaVersion` if `schema_version`
/// is neither 1 nor 2.
/// Returns `ImplPlanCodecError::Validation` if any domain type construction
/// fails, including the declared-dependency invariants the plan document
/// enforces (IN-20, AC-22).
pub fn decode(json: &str) -> Result<ImplPlanDocument, ImplPlanCodecError> {
    let envelope: SchemaVersionEnvelope = serde_json::from_str(json)?;
    let (tasks, plan) = match envelope.schema_version {
        1 => {
            let dto: ImplPlanDocumentV1Dto = serde_json::from_str(json)?;
            let tasks = tasks_from_dtos(dto.tasks.into_iter().map(Into::into).collect())?;
            (tasks, plan_from_dto(dto.plan)?)
        }
        2 => {
            let dto: ImplPlanDocumentDto = serde_json::from_str(json)?;
            (tasks_from_dtos(dto.tasks)?, plan_from_dto(dto.plan)?)
        }
        version => return Err(ImplPlanCodecError::UnsupportedSchemaVersion(version)),
    };

    Ok(ImplPlanDocument::new(tasks, plan)?)
}

fn tasks_from_dtos(dtos: Vec<ImplPlanTaskDto>) -> Result<Vec<TrackTask>, ImplPlanCodecError> {
    dtos.into_iter()
        .map(|t| {
            let task_id = TaskId::try_new(&t.id)?;
            let status = parse_task_status(&t.status, t.commit_hash.as_deref())?;
            let description = NonEmptyString::try_new(t.description).map_err(|_| {
                ImplPlanCodecError::Validation("task description is empty".to_owned())
            })?;
            Ok(TrackTask::with_dependencies(task_id, description, status, t.depends_on))
        })
        .collect()
}

fn parse_task_status(
    status: &str,
    commit_hash: Option<&str>,
) -> Result<TaskStatus, ImplPlanCodecError> {
    match status {
        "todo" => {
            if commit_hash.is_some() {
                return Err(ImplPlanCodecError::Validation(
                    "commit_hash is only valid for 'done' status, got 'todo'".to_owned(),
                ));
            }
            Ok(TaskStatus::Todo)
        }
        "in_progress" => {
            if commit_hash.is_some() {
                return Err(ImplPlanCodecError::Validation(
                    "commit_hash is only valid for 'done' status, got 'in_progress'".to_owned(),
                ));
            }
            Ok(TaskStatus::InProgress)
        }
        "done" => match commit_hash {
            Some(h) => {
                let hash = CommitHash::try_new(h)?;
                Ok(TaskStatus::DoneTraced { commit_hash: hash })
            }
            None => Ok(TaskStatus::DonePending),
        },
        "skipped" => {
            if commit_hash.is_some() {
                return Err(ImplPlanCodecError::Validation(
                    "commit_hash is only valid for 'done' status, got 'skipped'".to_owned(),
                ));
            }
            Ok(TaskStatus::Skipped)
        }
        other => Err(ImplPlanCodecError::Validation(format!(
            "unknown task status '{other}': expected 'todo', 'in_progress', 'done', or 'skipped'"
        ))),
    }
}

fn plan_from_dto(dto: ImplPlanPlanDto) -> Result<PlanView, ImplPlanCodecError> {
    let sections =
        dto.sections.into_iter().map(plan_section_from_dto).collect::<Result<Vec<_>, _>>()?;
    Ok(PlanView::new(dto.summary, sections))
}

fn plan_section_from_dto(dto: ImplPlanSectionDto) -> Result<PlanSection, ImplPlanCodecError> {
    let task_ids = dto
        .task_ids
        .into_iter()
        .map(|id| TaskId::try_new(id).map_err(ImplPlanCodecError::from))
        .collect::<Result<Vec<_>, _>>()?;
    PlanSection::new(dto.id, dto.title, dto.description, task_ids)
        .map_err(|e| ImplPlanCodecError::Validation(e.to_string()))
}

// ---------------------------------------------------------------------------
// Encode: domain -> JSON
// ---------------------------------------------------------------------------

/// Serializes an [`ImplPlanDocument`] to a pretty-printed `impl-plan.json` string.
///
/// # Errors
///
/// Returns `ImplPlanCodecError::Json` if serialization fails.
pub fn encode(doc: &ImplPlanDocument) -> Result<String, ImplPlanCodecError> {
    let dto = impl_plan_to_dto(doc);
    Ok(serde_json::to_string_pretty(&dto)?)
}

fn impl_plan_to_dto(doc: &ImplPlanDocument) -> ImplPlanDocumentDto {
    ImplPlanDocumentDto {
        // The writer always emits the current wire schema, whatever version the
        // document was read from (IN-19, AC-21).
        schema_version: WRITTEN_SCHEMA_VERSION,
        tasks: doc.tasks().iter().map(task_to_dto).collect(),
        plan: plan_to_dto(doc.plan()),
    }
}

fn task_to_dto(task: &TrackTask) -> ImplPlanTaskDto {
    let (status, commit_hash) = match task.status() {
        TaskStatus::Todo => ("todo".to_owned(), None),
        TaskStatus::InProgress => ("in_progress".to_owned(), None),
        TaskStatus::DonePending => ("done".to_owned(), None),
        TaskStatus::DoneTraced { commit_hash } => {
            ("done".to_owned(), Some(commit_hash.to_string()))
        }
        TaskStatus::Skipped => ("skipped".to_owned(), None),
    };
    ImplPlanTaskDto {
        id: task.id().to_string(),
        description: task.description().to_owned(),
        status,
        commit_hash,
        depends_on: task.depends_on().to_vec(),
    }
}

fn plan_to_dto(plan: &PlanView) -> ImplPlanPlanDto {
    ImplPlanPlanDto {
        summary: plan.summary().to_vec(),
        sections: plan.sections().iter().map(section_to_dto).collect(),
    }
}

fn section_to_dto(s: &PlanSection) -> ImplPlanSectionDto {
    ImplPlanSectionDto {
        id: s.id().to_owned(),
        title: s.title().to_owned(),
        description: s.description().to_vec(),
        task_ids: s.task_ids().iter().map(|id| id.to_string()).collect(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const MINIMAL_JSON: &str = r#"{
  "schema_version": 1,
  "plan": {
    "summary": [],
    "sections": []
  }
}"#;

    const FULL_JSON: &str = r#"{
  "schema_version": 1,
  "tasks": [
    {"id": "T001", "description": "Write domain model", "status": "todo"},
    {"id": "T002", "description": "Write codec", "status": "done", "commit_hash": "abc1234"},
    {"id": "T003", "description": "Write tests", "status": "skipped"}
  ],
  "plan": {
    "summary": ["Overview line"],
    "sections": [
      {
        "id": "S1",
        "title": "Build",
        "description": ["Section desc"],
        "task_ids": ["T001", "T002", "T003"]
      }
    ]
  }
}"#;

    // --- decode: happy path ---

    #[test]
    fn test_decode_minimal_json_succeeds() {
        let doc = decode(MINIMAL_JSON).unwrap();
        assert_eq!(doc.schema_version(), 1);
        assert!(doc.tasks().is_empty());
        assert!(doc.plan().sections().is_empty());
    }

    #[test]
    fn test_decode_full_json_succeeds() {
        let doc = decode(FULL_JSON).unwrap();
        assert_eq!(doc.tasks().len(), 3);
        assert_eq!(doc.tasks()[0].id().as_ref(), "T001");
        assert_eq!(doc.tasks()[0].description(), "Write domain model");
        assert!(matches!(doc.tasks()[0].status(), domain::TaskStatus::Todo));
        assert!(matches!(doc.tasks()[1].status(), domain::TaskStatus::DoneTraced { .. }));
        assert!(matches!(doc.tasks()[2].status(), domain::TaskStatus::Skipped));
        assert_eq!(doc.plan().sections().len(), 1);
        assert_eq!(doc.plan().sections()[0].title(), "Build");
        assert_eq!(doc.plan().summary(), &["Overview line"]);
    }

    // --- decode: schema_version validation ---

    #[test]
    fn test_decode_with_unsupported_schema_version_returns_error() {
        let json = r#"{"schema_version": 3, "plan": {"summary": [], "sections": []}}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, ImplPlanCodecError::UnsupportedSchemaVersion(3)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_accepts_both_schema_one_and_schema_two() {
        for version in [1, 2] {
            let json = format!(
                r#"{{"schema_version": {version}, "plan": {{"summary": [], "sections": []}}}}"#
            );
            assert!(decode(&json).is_ok(), "schema_version {version} must decode");
        }
    }

    #[test]
    fn test_decode_with_schema_version_zero_returns_error() {
        let json = r#"{"schema_version": 0, "plan": {"summary": [], "sections": []}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::UnsupportedSchemaVersion(0)));
    }

    // --- decode: unknown field rejection ---

    #[test]
    fn test_decode_with_unknown_field_is_rejected() {
        let json =
            r#"{"schema_version": 1, "plan": {"summary": [], "sections": []}, "extra": "bad"}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Json(_)), "expected Json error, got: {err}");
    }

    #[test]
    fn test_decode_with_unknown_field_in_task_is_rejected() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "todo", "extra": "bad"}],
          "plan": {"summary": [], "sections": []}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Json(_)), "expected Json error, got: {err}");
    }

    #[test]
    fn test_decode_with_unknown_field_in_plan_is_rejected() {
        let json =
            r#"{"schema_version": 1, "plan": {"summary": [], "sections": [], "extra": "bad"}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Json(_)), "expected Json error, got: {err}");
    }

    #[test]
    fn test_decode_with_unknown_field_in_section_is_rejected() {
        let json = r#"{
          "schema_version": 1,
          "plan": {
            "summary": [],
            "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": [], "extra": "bad"}]
          }
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Json(_)), "expected Json error, got: {err}");
    }

    // --- decode: domain validation errors ---

    #[test]
    fn test_decode_with_commit_hash_on_todo_task_returns_validation_error() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "todo", "commit_hash": "abc1234"}],
          "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T001"]}]}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Validation(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_with_commit_hash_on_in_progress_task_returns_validation_error() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "in_progress", "commit_hash": "abc1234"}],
          "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T001"]}]}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Validation(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_with_commit_hash_on_skipped_task_returns_validation_error() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "skipped", "commit_hash": "abc1234"}],
          "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T001"]}]}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Validation(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_with_invalid_task_status_returns_validation_error() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "unknown"}],
          "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T001"]}]}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Validation(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_with_invalid_task_id_returns_validation_error() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "INVALID", "description": "task", "status": "todo"}],
          "plan": {"summary": [], "sections": []}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Validation(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_with_mismatched_plan_and_tasks_returns_error() {
        // T001 in tasks, T002 in plan — mismatch
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "todo"}],
          "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T002"]}]}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Validation(_)), "unexpected error: {err}");
    }

    // --- decode: malformed JSON ---

    #[test]
    fn test_decode_invalid_json_returns_json_error() {
        let err = decode("{not json}").unwrap_err();
        assert!(matches!(err, ImplPlanCodecError::Json(_)));
    }

    // --- encode: happy path ---

    #[test]
    fn test_encode_empty_document_produces_valid_json() {
        let doc = ImplPlanDocument::new(vec![], PlanView::new(vec![], vec![])).unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 2);
    }

    #[test]
    fn test_encode_output_is_pretty_printed() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        assert!(json.contains('\n'));
    }

    // --- round-trip tests ---

    #[test]
    fn test_round_trip_minimal_json() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_round_trip_full_json() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_round_trip_preserves_task_statuses() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        assert_eq!(doc2.tasks().len(), 3);
        assert!(matches!(doc2.tasks()[0].status(), domain::TaskStatus::Todo));
        assert!(matches!(doc2.tasks()[1].status(), domain::TaskStatus::DoneTraced { .. }));
        assert!(matches!(doc2.tasks()[2].status(), domain::TaskStatus::Skipped));
    }

    #[test]
    fn test_round_trip_in_progress_status() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "in_progress"}],
          "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T001"]}]}
        }"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert!(matches!(doc2.tasks()[0].status(), domain::TaskStatus::InProgress));
    }

    #[test]
    fn test_round_trip_done_pending_status() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [{"id": "T001", "description": "task", "status": "done"}],
          "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T001"]}]}
        }"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert!(matches!(doc2.tasks()[0].status(), domain::TaskStatus::DonePending));
    }

    #[test]
    fn test_encode_writes_schema_two_even_for_a_document_read_as_schema_one() {
        // FULL_JSON declares schema_version 1.
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 2);
    }

    // --- declared task dependencies (schema 2) ---

    #[test]
    fn test_decode_reads_a_schema_one_task_as_declaring_no_dependency() {
        let doc = decode(FULL_JSON).unwrap();

        assert!(doc.tasks().iter().all(|task| task.depends_on().is_empty()));
    }

    #[test]
    fn test_decode_reads_a_schema_one_task_document_that_carries_no_depends_on_key() {
        // A schema-1 task entry has no depends_on field; the schema-1 DTO
        // translates it to the domain's undeclared-dependency state (AC-05, AC-21).
        let json = r#"{
          "schema_version": 1,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo"},
            {"id": "T002", "description": "Second", "status": "done", "commit_hash": "a1b2c3d"}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
          ]}
        }"#;

        let doc = decode(json).unwrap();

        assert_eq!(doc.tasks().len(), 2);
        assert!(doc.tasks()[0].depends_on().is_empty());
        assert!(doc.tasks()[1].depends_on().is_empty());
    }

    #[test]
    fn test_decode_rejects_dependency_field_in_schema_one_document() {
        let json = r#"{
          "schema_version": 1,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo", "depends_on": []}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001"]}
          ]}
        }"#;

        let err = decode(json).unwrap_err();

        assert!(matches!(err, ImplPlanCodecError::Json(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_reads_an_explicit_empty_dependency_list_as_declaring_nothing() {
        let json = r#"{
          "schema_version": 2,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo", "depends_on": []}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001"]}
          ]}
        }"#;

        let doc = decode(json).unwrap();

        assert!(
            doc.tasks()[0].depends_on().is_empty(),
            "an empty list is the same unstated state as an omitted one"
        );
        // And it round-trips as an omission rather than as an empty array.
        let parsed: serde_json::Value = serde_json::from_str(&encode(&doc).unwrap()).unwrap();
        assert!(parsed["tasks"][0].get("depends_on").is_none());
    }

    #[test]
    fn test_decode_reads_the_dependencies_a_schema_two_task_declares() {
        let json = r#"{
          "schema_version": 2,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo"},
            {"id": "T002", "description": "Second", "status": "todo", "depends_on": ["T001"]}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
          ]}
        }"#;

        let doc = decode(json).unwrap();

        assert!(doc.tasks()[0].depends_on().is_empty());
        assert_eq!(doc.tasks()[1].depends_on(), &[TaskId::try_new("T001").unwrap()]);
    }

    #[test]
    fn test_decode_rejects_a_declared_dependency_the_plan_does_not_contain() {
        let json = r#"{
          "schema_version": 2,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo", "depends_on": ["T404"]}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001"]}
          ]}
        }"#;

        let err = decode(json).unwrap_err();

        assert!(
            matches!(err, ImplPlanCodecError::Validation(ref message) if message.contains("T404")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_rejects_a_cycle_in_declared_dependencies() {
        let json = r#"{
          "schema_version": 2,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo", "depends_on": ["T002"]},
            {"id": "T002", "description": "Second", "status": "todo", "depends_on": ["T001"]}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
          ]}
        }"#;

        let err = decode(json).unwrap_err();

        assert!(
            matches!(err, ImplPlanCodecError::Validation(ref message) if message.contains("cycle")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_rejects_a_self_dependency() {
        let json = r#"{
          "schema_version": 2,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo", "depends_on": ["T001"]}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001"]}
          ]}
        }"#;

        let err = decode(json).unwrap_err();

        assert!(
            matches!(err, ImplPlanCodecError::Validation(ref message) if message.contains("cycle")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_rejects_plan_order_that_precedes_a_declared_dependency() {
        let json = r#"{
          "schema_version": 2,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo", "depends_on": ["T002"]},
            {"id": "T002", "description": "Second", "status": "todo"}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
          ]}
        }"#;

        let err = decode(json).unwrap_err();

        assert!(
            matches!(err, ImplPlanCodecError::Validation(ref message) if message.contains("before its declared dependency")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_encode_omits_an_empty_dependency_list_and_writes_a_declared_one() {
        let json = r#"{
          "schema_version": 2,
          "tasks": [
            {"id": "T001", "description": "First", "status": "todo"},
            {"id": "T002", "description": "Second", "status": "todo", "depends_on": ["T001"]}
          ],
          "plan": {"summary": [], "sections": [
            {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
          ]}
        }"#;
        let doc = decode(json).unwrap();

        let encoded = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&encoded).unwrap();

        assert!(parsed["tasks"][0].get("depends_on").is_none(), "encoded as: {encoded}");
        assert_eq!(parsed["tasks"][1]["depends_on"], serde_json::json!(["T001"]));
        assert_eq!(decode(&encoded).unwrap().tasks()[1].depends_on().len(), 1);
    }
}
