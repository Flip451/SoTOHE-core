//! Serde types for metadata.json (TrackDocumentV2) matching Python track_schema.py.

use domain::{
    CommitHash, DomainError, PlanSection, PlanView, StatusOverride, TaskId, TaskStatus,
    TrackBranch, TrackId, TrackMetadata, TrackTask,
};
use serde::{Deserialize, Deserializer};

/// Codec error for metadata.json serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("domain validation error: {0}")]
    Domain(#[from] DomainError),

    #[error("invalid field '{field}': {reason}")]
    InvalidField { field: String, reason: String },

    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackDocumentV2 {
    pub schema_version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub tasks: Vec<TrackTaskDocument>,
    pub plan: PlanDocument,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_override: Option<TrackStatusOverrideDocument>,
    /// Unknown fields captured during deserialization and preserved on re-serialization.
    #[serde(flatten)]
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackTaskDocument {
    pub id: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanDocument {
    #[serde(default, deserialize_with = "deserialize_string_vec_relaxed")]
    pub summary: Vec<String>,
    #[serde(default)]
    pub sections: Vec<PlanSectionDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanSectionDocument {
    pub id: String,
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_vec_relaxed")]
    pub description: Vec<String>,
    #[serde(default)]
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackStatusOverrideDocument {
    pub status: String,
    pub reason: String,
}

/// Metadata not part of the domain aggregate (infrastructure concern).
#[derive(Debug, Clone)]
pub struct DocumentMeta {
    pub schema_version: u32,
    pub created_at: String,
    pub updated_at: String,
    /// Original JSON status string, preserved for values the domain model
    /// cannot compute (e.g., "archived" which is a workflow-level state).
    pub original_status: Option<String>,
    /// Unknown fields captured from the original JSON and preserved on re-serialization.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn deserialize_string_vec_relaxed<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) => Ok(vec![s]),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(s) => Ok(s),
                other => {
                    Err(serde::de::Error::custom(format!("expected string item, got {other}")))
                }
            })
            .collect(),
        other => Err(serde::de::Error::custom(format!("expected string or sequence, got {other}"))),
    }
}

/// Decodes a JSON string into a domain `TrackMetadata` and infrastructure `DocumentMeta`.
///
/// # Errors
/// Returns `CodecError` on JSON parse failure or domain validation failure.
pub fn decode(json: &str) -> Result<(TrackMetadata, DocumentMeta), CodecError> {
    let doc: TrackDocumentV2 = serde_json::from_str(json)?;
    let meta = DocumentMeta {
        schema_version: doc.schema_version,
        created_at: doc.created_at.clone(),
        updated_at: doc.updated_at.clone(),
        original_status: Some(doc.status.clone()),
        extra: doc.extra.clone(),
    };
    let track = track_metadata_from_document(doc)?;
    Ok((track, meta))
}

/// Encodes a domain `TrackMetadata` and infrastructure `DocumentMeta` into a JSON string.
///
/// # Errors
/// Returns `CodecError` on JSON serialization failure.
pub fn encode(track: &TrackMetadata, meta: &DocumentMeta) -> Result<String, CodecError> {
    let doc = document_from_track_metadata(track, meta);
    let mut value = serde_json::to_value(&doc)?;
    if meta.schema_version == 3 && track.branch().is_none() {
        if let serde_json::Value::Object(object) = &mut value {
            object.insert("branch".to_owned(), serde_json::Value::Null);
        }
    }
    let json = serde_json::to_string_pretty(&value)?;
    Ok(json)
}

fn track_metadata_from_document(doc: TrackDocumentV2) -> Result<TrackMetadata, CodecError> {
    let id = TrackId::try_new(&doc.id).map_err(DomainError::from)?;

    let branch = doc
        .branch
        .map(TrackBranch::try_new)
        .transpose()
        .map_err(|e| CodecError::Domain(e.into()))?;

    let tasks: Vec<TrackTask> = doc
        .tasks
        .into_iter()
        .map(|t| {
            let task_id = TaskId::try_new(&t.id).map_err(DomainError::from)?;
            let status = parse_task_status(&t.status, t.commit_hash.as_deref())?;
            TrackTask::with_status(task_id, t.description, status)
                .map_err(|e| CodecError::Domain(e.into()))
        })
        .collect::<Result<Vec<_>, CodecError>>()?;

    let plan = plan_from_document(doc.plan)?;

    let status_override =
        doc.status_override.map(|o| parse_status_override(&o.status, o.reason)).transpose()?;

    let track = TrackMetadata::with_branch(id, branch, doc.title, tasks, plan, status_override)?;

    Ok(track)
}

fn document_from_track_metadata(track: &TrackMetadata, meta: &DocumentMeta) -> TrackDocumentV2 {
    // Preserve "archived" status from the original JSON when the domain model
    // cannot compute it (archived is a workflow-level state, not task-derived).
    let status = match meta.original_status.as_deref() {
        Some("archived") => "archived".to_owned(),
        _ => track.status().to_string(),
    };

    TrackDocumentV2 {
        schema_version: meta.schema_version,
        id: track.id().to_string(),
        branch: track.branch().map(|b| b.to_string()),
        title: track.title().to_string(),
        status,
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        tasks: track.tasks().iter().map(task_to_document).collect(),
        plan: plan_to_document(track.plan()),
        status_override: track.status_override().map(override_to_document),
        extra: meta.extra.clone(),
    }
}

fn parse_task_status(status: &str, commit_hash: Option<&str>) -> Result<TaskStatus, CodecError> {
    match status {
        "todo" => Ok(TaskStatus::Todo),
        "in_progress" => Ok(TaskStatus::InProgress),
        "done" => match commit_hash {
            Some(h) => {
                let hash = CommitHash::try_new(h).map_err(|e| CodecError::Domain(e.into()))?;
                Ok(TaskStatus::DoneTraced { commit_hash: hash })
            }
            None => Ok(TaskStatus::DonePending),
        },
        "skipped" => Ok(TaskStatus::Skipped),
        other => Err(CodecError::InvalidField {
            field: "status".into(),
            reason: format!("unknown task status: {other}"),
        }),
    }
}

fn parse_status_override(status: &str, reason: String) -> Result<StatusOverride, CodecError> {
    match status {
        "blocked" => StatusOverride::blocked(reason).map_err(|e| CodecError::Domain(e.into())),
        "cancelled" => StatusOverride::cancelled(reason).map_err(|e| CodecError::Domain(e.into())),
        other => Err(CodecError::InvalidField {
            field: "status_override.status".into(),
            reason: format!("unknown override status: {other}"),
        }),
    }
}

fn plan_from_document(doc: PlanDocument) -> Result<PlanView, CodecError> {
    let sections = doc
        .sections
        .into_iter()
        .map(|s| {
            let task_ids = s
                .task_ids
                .into_iter()
                .map(|id| TaskId::try_new(id).map_err(|e| CodecError::Domain(e.into())))
                .collect::<Result<Vec<_>, _>>()?;
            PlanSection::new(s.id, s.title, s.description, task_ids)
                .map_err(|e| CodecError::Domain(e.into()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PlanView::new(doc.summary, sections))
}

fn task_to_document(task: &TrackTask) -> TrackTaskDocument {
    let (status, commit_hash) = match task.status() {
        TaskStatus::Todo => ("todo".to_owned(), None),
        TaskStatus::InProgress => ("in_progress".to_owned(), None),
        TaskStatus::DonePending => ("done".to_owned(), None),
        TaskStatus::DoneTraced { commit_hash } => {
            ("done".to_owned(), Some(commit_hash.to_string()))
        }
        TaskStatus::Skipped => ("skipped".to_owned(), None),
    };

    TrackTaskDocument {
        id: task.id().to_string(),
        description: task.description().to_owned(),
        status,
        commit_hash,
    }
}

fn plan_to_document(plan: &PlanView) -> PlanDocument {
    PlanDocument {
        summary: plan.summary().to_vec(),
        sections: plan
            .sections()
            .iter()
            .map(|s| PlanSectionDocument {
                id: s.id().to_owned(),
                title: s.title().to_owned(),
                description: s.description().to_vec(),
                task_ids: s.task_ids().iter().map(|id| id.to_string()).collect(),
            })
            .collect(),
    }
}

fn override_to_document(override_: &StatusOverride) -> TrackStatusOverrideDocument {
    TrackStatusOverrideDocument {
        status: override_.kind().to_string(),
        reason: override_.reason().to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use domain::TrackStatus;

    fn sample_json() -> &'static str {
        r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {
      "id": "T1",
      "description": "First task",
      "status": "todo"
    },
    {
      "id": "T2",
      "description": "Second task",
      "status": "done",
      "commit_hash": "abc1234"
    }
  ],
  "plan": {
    "summary": ["Test plan summary"],
    "sections": [
      {
        "id": "S1",
        "title": "Section 1",
        "description": ["Description line"],
        "task_ids": ["T1", "T2"]
      }
    ]
  }
}"#
    }

    #[test]
    fn test_decode_valid_json_returns_track_metadata() {
        let (track, meta) = decode(sample_json()).unwrap();
        assert_eq!(track.id().as_ref(), "test-track");
        assert_eq!(track.title(), "Test Track");
        assert_eq!(track.tasks().len(), 2);
        assert_eq!(meta.schema_version, 2);
        assert_eq!(meta.created_at, "2026-03-11T00:00:00Z");
    }

    #[test]
    fn test_encode_then_decode_round_trip() {
        let (track, meta) = decode(sample_json()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let (track2, meta2) = decode(&json).unwrap();
        assert_eq!(track, track2);
        assert_eq!(meta.schema_version, meta2.schema_version);
    }

    #[test]
    fn test_done_pending_round_trips_without_commit_hash() {
        let json = r#"{
  "schema_version": 2,
  "id": "pending-track",
  "title": "Pending Test",
  "status": "in_progress",
  "created_at": "2026-03-20T00:00:00Z",
  "updated_at": "2026-03-20T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Done without hash", "status": "done"}
  ],
  "plan": {
    "summary": [],
    "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T1"]}]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        assert!(matches!(track.tasks()[0].status(), TaskStatus::DonePending));

        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        let task = &doc["tasks"][0];
        assert_eq!(task["status"], "done");
        assert!(task.get("commit_hash").is_none() || task["commit_hash"].is_null());

        let (track2, _) = decode(&re_encoded).unwrap();
        assert_eq!(track, track2);
    }

    #[test]
    fn test_decode_with_status_override() {
        let json = r#"{
  "schema_version": 2,
  "id": "blocked-track",
  "title": "Blocked Track",
  "status": "blocked",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  },
  "status_override": {"status": "blocked", "reason": "waiting on review"}
}"#;
        let (track, _meta) = decode(json).unwrap();
        assert_eq!(track.status(), TrackStatus::Blocked);
        assert!(track.status_override().is_some());
    }

    #[test]
    fn test_decode_accepts_missing_section_description() {
        let json = r#"{
  "schema_version": 3,
  "id": "compat-track",
  "branch": "track/compat-track",
  "title": "Compat Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {
      "id": "T1",
      "description": "First task",
      "status": "todo"
    }
  ],
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Section 1",
        "task_ids": ["T1"]
      }
    ]
  }
        }"#;

        let (track, _) = decode(json).unwrap();
        assert!(track.plan().sections()[0].description().is_empty());
    }

    #[test]
    fn test_decode_accepts_string_summary() {
        let json = r#"{
  "schema_version": 2,
  "id": "string-summary-track",
  "title": "String Summary Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {
      "id": "T1",
      "description": "First task",
      "status": "todo"
    }
  ],
  "plan": {
    "summary": "single summary line",
    "sections": [
      {
        "id": "S1",
        "title": "Section 1",
        "task_ids": ["T1"]
      }
    ]
  }
}"#;

        let (track, _) = decode(json).unwrap();
        assert_eq!(track.plan().summary(), &["single summary line".to_owned()]);
    }

    #[test]
    fn test_decode_invalid_task_status_returns_error() {
        let json = r#"{
  "schema_version": 2,
  "id": "bad-track",
  "title": "Bad Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "unknown_status"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let result = decode(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_json_returns_error() {
        let result = decode("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_archived_status_preserved_through_round_trip() {
        let json = r#"{
  "schema_version": 2,
  "id": "archived-track",
  "title": "Archived Track",
  "status": "archived",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Done task", "status": "done", "commit_hash": "abc1234"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

        // "archived" must be preserved, not rewritten to "done".
        assert_eq!(doc["status"].as_str().unwrap(), "archived");
    }

    #[test]
    fn test_decode_encode_preserves_unknown_fields() {
        let json = r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "custom_field": "preserved_value",
  "tasks": [
    {"id": "T1", "description": "First task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section 1", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        assert_eq!(doc["custom_field"].as_str().unwrap(), "preserved_value");
    }

    #[test]
    fn test_decode_encode_without_extra_fields_round_trips_correctly() {
        let (track, meta) = decode(sample_json()).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let (track2, _) = decode(&re_encoded).unwrap();
        assert_eq!(track, track2);
    }

    #[test]
    fn test_known_fields_are_not_in_extra_map() {
        let json = sample_json();
        let doc: TrackDocumentV2 = serde_json::from_str(json).unwrap();
        // Known fields like "id", "title", "tasks" should NOT appear in the extra map
        assert!(!doc.extra.contains_key("id"));
        assert!(!doc.extra.contains_key("title"));
        assert!(!doc.extra.contains_key("tasks"));
        assert!(!doc.extra.contains_key("schema_version"));
    }

    #[test]
    fn test_encode_v3_branchless_track_preserves_null_branch_field() {
        let json = r#"{
  "schema_version": 3,
  "id": "plan-only-track",
  "branch": null,
  "title": "Plan Only Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Todo task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

        assert!(doc.get("branch").is_some());
        assert!(doc["branch"].is_null());
    }
}
