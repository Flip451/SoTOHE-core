//! Serde codec for `impl-plan.json` (ImplPlanDocument SSoT).
//!
//! Schema version 1: introduced by ADR 2026-04-19-1242 §D1.4.
//! All DTOs are defined locally with `deny_unknown_fields` to enforce
//! the strict schema boundary at every nesting level.

use domain::{
    CommitHash, DomainError, ImplPlanDocument, PlanSection, PlanView, TaskId, TaskStatus,
    TrackTask, ValidationError,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for `impl-plan.json` serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum ImplPlanCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported schema_version: expected 1, got {0}")]
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

/// Top-level DTO for `impl-plan.json` (schema_version 1).
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplPlanTaskDto {
    pub id: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
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
/// # Errors
///
/// Returns `ImplPlanCodecError::Json` if the JSON is malformed.
/// Returns `ImplPlanCodecError::UnsupportedSchemaVersion` if `schema_version != 1`.
/// Returns `ImplPlanCodecError::Validation` if any domain type construction fails.
pub fn decode(json: &str) -> Result<ImplPlanDocument, ImplPlanCodecError> {
    let dto: ImplPlanDocumentDto = serde_json::from_str(json)?;

    if dto.schema_version != 1 {
        return Err(ImplPlanCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }

    let tasks = tasks_from_dtos(dto.tasks)?;
    let plan = plan_from_dto(dto.plan)?;

    Ok(ImplPlanDocument::new(tasks, plan)?)
}

fn tasks_from_dtos(dtos: Vec<ImplPlanTaskDto>) -> Result<Vec<TrackTask>, ImplPlanCodecError> {
    dtos.into_iter()
        .map(|t| {
            let task_id = TaskId::try_new(&t.id)?;
            let status = parse_task_status(&t.status, t.commit_hash.as_deref())?;
            TrackTask::with_status(task_id, t.description, status).map_err(|e| e.into())
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
        schema_version: doc.schema_version(),
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
        let json = r#"{"schema_version": 2, "plan": {"summary": [], "sections": []}}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, ImplPlanCodecError::UnsupportedSchemaVersion(2)),
            "unexpected error: {err}"
        );
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
        assert_eq!(parsed["schema_version"], 1);
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
    fn test_encode_schema_version_is_always_1() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 1);
    }
}
