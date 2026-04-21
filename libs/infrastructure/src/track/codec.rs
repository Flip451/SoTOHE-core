//! Serde types for metadata.json (TrackDocumentV2) — identity-only shape.
//!
//! T005 (ADR 2026-04-19-1242 §D1.4): `tasks` and `plan` fields removed from
//! `TrackDocumentV2`; they now live in `impl-plan.json` (ImplPlanDocument).
//! `status` is now stored explicitly rather than task-derived.
//! Schema version bumped to 4 (no backward-compat per the ADR no-backward-compat rule).
//!
//! Legacy fields `tasks`/`plan` in old metadata.json files will cause a decode
//! error — migration to the new shape is the caller's responsibility.

use domain::{DomainError, StatusOverride, TrackBranch, TrackId, TrackMetadata, TrackStatus};

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

/// Identity-only DTO for `metadata.json` (schema_version = 4).
///
/// Per ADR 2026-04-19-1242 §D1.4: retained fields are
/// `schema_version, id, branch, title, status, created_at, updated_at`.
/// Optional `status_override` is kept for Blocked/Cancelled semantics.
/// `tasks` and `plan` are removed — they now live in `impl-plan.json`.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_override: Option<TrackStatusOverrideDocument>,
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
}

/// Decodes a JSON string into a domain `TrackMetadata` and infrastructure `DocumentMeta`.
///
/// Accepts both `schema_version = 4` (identity-only, new shape) and
/// `schema_version = 2` / `schema_version = 3` (legacy shape — `tasks`/`plan`
/// fields are silently ignored via serde's `#[serde(default)]`-style lenient
/// parsing so that existing metadata.json files can still be read during the
/// migration window, even though the tasks/plan data is not loaded here).
///
/// # Errors
/// Returns `CodecError` on JSON parse failure or domain validation failure.
pub fn decode(json: &str) -> Result<(TrackMetadata, DocumentMeta), CodecError> {
    // Lenient decode: unknown fields (tasks, plan) are silently ignored so
    // legacy v2/v3 files round-trip without error during the migration window.
    // We use serde_json::Value first to strip any unknown keys before feeding
    // into the strict TrackDocumentV2 DTO.
    let raw: serde_json::Value = serde_json::from_str(json)?;
    let stripped = strip_legacy_fields(raw);
    let doc: TrackDocumentV2 = serde_json::from_value(stripped)?;

    let meta = DocumentMeta {
        schema_version: doc.schema_version,
        created_at: doc.created_at.clone(),
        updated_at: doc.updated_at.clone(),
    };
    let track = track_metadata_from_document(doc)?;
    Ok((track, meta))
}

/// Removes legacy fields (`tasks`, `plan`) from a JSON value so that
/// `TrackDocumentV2` can be deserialized without unknown-field errors
/// during the migration window.
fn strip_legacy_fields(mut value: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(ref mut map) = value {
        map.remove("tasks");
        map.remove("plan");
        map.remove("extra");
        // Remove any other unknown fields that old versions may have written.
    }
    value
}

/// Encodes a domain `TrackMetadata` and infrastructure `DocumentMeta` into a JSON string.
///
/// Always writes `schema_version = 4` (the identity-only shape).
///
/// # Errors
/// Returns `CodecError` on JSON serialization failure.
pub fn encode(track: &TrackMetadata, meta: &DocumentMeta) -> Result<String, CodecError> {
    let doc = document_from_track_metadata(track, meta);
    let mut value = serde_json::to_value(&doc)?;
    // Preserve `branch: null` for planning-only tracks (schema_version 3/4 + no branch).
    if track.branch().is_none() {
        if let serde_json::Value::Object(ref mut object) = value {
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

    let status = parse_track_status(&doc.status)?;

    let status_override =
        doc.status_override.map(|o| parse_status_override(&o.status, o.reason)).transpose()?;

    let track = TrackMetadata::with_branch(id, branch, doc.title, status, status_override)?;

    Ok(track)
}

fn document_from_track_metadata(track: &TrackMetadata, meta: &DocumentMeta) -> TrackDocumentV2 {
    TrackDocumentV2 {
        schema_version: 4,
        id: track.id().to_string(),
        branch: track.branch().map(|b| b.to_string()),
        title: track.title().to_string(),
        status: track.status().to_string(),
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        status_override: track.status_override().map(override_to_document),
    }
}

fn parse_track_status(status: &str) -> Result<TrackStatus, CodecError> {
    match status {
        "planned" => Ok(TrackStatus::Planned),
        "in_progress" => Ok(TrackStatus::InProgress),
        "done" => Ok(TrackStatus::Done),
        "blocked" => Ok(TrackStatus::Blocked),
        "cancelled" => Ok(TrackStatus::Cancelled),
        "archived" => Ok(TrackStatus::Archived),
        other => Err(CodecError::InvalidField {
            field: "status".into(),
            reason: format!("unknown track status: {other}"),
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

    /// Minimal identity-only metadata.json (schema_version = 4).
    fn sample_json_v4() -> &'static str {
        r#"{
  "schema_version": 4,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#
    }

    /// Legacy metadata.json (schema_version = 2 with tasks/plan).
    /// The codec must accept this during the migration window (tasks/plan silently ignored).
    fn sample_json_legacy_v2() -> &'static str {
        r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "First task", "status": "todo"},
    {"id": "T2", "description": "Second task", "status": "done", "commit_hash": "abc1234"}
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
    fn test_decode_v4_identity_only_json_returns_track_metadata() {
        let (track, meta) = decode(sample_json_v4()).unwrap();
        assert_eq!(track.id().as_ref(), "test-track");
        assert_eq!(track.title(), "Test Track");
        assert_eq!(track.status(), TrackStatus::Planned);
        assert_eq!(meta.schema_version, 4);
        assert_eq!(meta.created_at, "2026-03-11T00:00:00Z");
    }

    #[test]
    fn test_decode_legacy_v2_succeeds_ignoring_tasks_and_plan() {
        // Migration window: legacy v2 files can be read; tasks/plan are dropped.
        let (track, meta) = decode(sample_json_legacy_v2()).unwrap();
        assert_eq!(track.id().as_ref(), "test-track");
        assert_eq!(track.status(), TrackStatus::Planned);
        assert_eq!(meta.schema_version, 2);
    }

    #[test]
    fn test_encode_always_writes_schema_version_4() {
        let (track, meta) = decode(sample_json_legacy_v2()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(doc["schema_version"].as_u64().unwrap(), 4);
    }

    #[test]
    fn test_encode_v4_then_decode_round_trip() {
        let (track, meta) = decode(sample_json_v4()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let (track2, meta2) = decode(&json).unwrap();
        assert_eq!(track, track2);
        assert_eq!(meta.schema_version, meta2.schema_version);
    }

    #[test]
    fn test_decode_with_status_override_blocked() {
        let json = r#"{
  "schema_version": 4,
  "id": "blocked-track",
  "title": "Blocked Track",
  "status": "blocked",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "status_override": {"status": "blocked", "reason": "waiting on review"}
}"#;
        let (track, _meta) = decode(json).unwrap();
        assert_eq!(track.status(), TrackStatus::Blocked);
        assert!(track.status_override().is_some());
        assert_eq!(track.status_override().unwrap().reason(), "waiting on review");
    }

    #[test]
    fn test_decode_archived_status_round_trips() {
        let json = r#"{
  "schema_version": 4,
  "id": "archived-track",
  "title": "Archived Track",
  "status": "archived",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#;
        let (track, meta) = decode(json).unwrap();
        assert_eq!(track.status(), TrackStatus::Archived);

        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        assert_eq!(doc["status"].as_str().unwrap(), "archived");
    }

    #[test]
    fn test_decode_invalid_status_returns_error() {
        let json = r#"{
  "schema_version": 4,
  "id": "bad-track",
  "title": "Bad Track",
  "status": "unknown_status",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
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
    fn test_encode_branchless_track_has_null_branch_field() {
        let (track, meta) = decode(sample_json_v4()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Branchless track must emit `"branch": null`
        assert!(doc.get("branch").is_some(), "branch field must be present");
        assert!(doc["branch"].is_null(), "branch must be null for branchless track");
    }

    #[test]
    fn test_encode_with_branch_emits_branch_string() {
        let json = r#"{
  "schema_version": 4,
  "id": "branched-track",
  "branch": "track/branched-track",
  "title": "Branched Track",
  "status": "in_progress",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        assert_eq!(doc["branch"].as_str().unwrap(), "track/branched-track");
    }

    #[test]
    fn test_decode_no_tasks_or_plan_fields_in_v4_output() {
        let (track, meta) = decode(sample_json_v4()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Identity-only: tasks and plan must NOT appear in the encoded output.
        assert!(doc.get("tasks").is_none(), "tasks must not be emitted in v4 output");
        assert!(doc.get("plan").is_none(), "plan must not be emitted in v4 output");
    }

    #[test]
    fn test_decode_legacy_v2_encodes_to_v4_without_tasks_or_plan() {
        let (track, meta) = decode(sample_json_legacy_v2()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(doc["schema_version"].as_u64().unwrap(), 4);
        assert!(doc.get("tasks").is_none());
        assert!(doc.get("plan").is_none());
    }
}
