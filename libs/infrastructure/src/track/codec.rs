//! Serde types for metadata.json (TrackDocumentV2) — identity-only shape.
//!
//! Per ADR 2026-04-19-1242 §D1.4, `tasks` and `plan` fields are removed from
//! `TrackDocumentV2`; they now live in `impl-plan.json` (ImplPlanDocument).
//! The `status` field is also removed from metadata.json; track status is
//! derived on demand via `domain::derive_track_status`. Schema version is 5.
//!
//! Legacy v4 (has `status`) and older are rejected on decode — no backward-compat.

use domain::{DomainError, StatusOverride, TrackBranch, TrackId, TrackMetadata};

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

/// Identity-only DTO for `metadata.json` (schema_version = 5).
///
/// Per ADR 2026-04-19-1242 §D1.4:
/// - `status` is removed; track status is derived on demand from `impl-plan.json`.
/// - `status_override` is kept for Blocked/Cancelled semantics.
/// - `tasks` and `plan` are removed — they live in `impl-plan.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackDocumentV2 {
    pub schema_version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_override: Option<TrackStatusOverrideDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
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
/// Accepts only `schema_version = 5` (identity-only, no `status` field).
/// All previous schema versions (v2/v3/v4) are rejected with a clear error message.
///
/// # Errors
/// Returns `CodecError` on JSON parse failure, schema version mismatch, or domain
/// validation failure.
pub fn decode(json: &str) -> Result<(TrackMetadata, DocumentMeta), CodecError> {
    // Peek at schema_version before full decode. Use `u32::try_from` to reject
    // values that overflow u32 instead of silently wrapping (e.g., `4294967301`
    // must not truncate to `5` and be misclassified as valid v5 metadata).
    let raw: serde_json::Value = serde_json::from_str(json)?;
    let schema_version_u64 = raw.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let schema_version = u32::try_from(schema_version_u64).map_err(|_| {
        CodecError::Validation(format!(
            "metadata.json schema_version {schema_version_u64} overflows u32; \
             schema v5 is required"
        ))
    })?;
    if schema_version != 5 {
        return Err(CodecError::Validation(format!(
            "metadata.json schema_version {schema_version} is not supported; \
             schema v5 is required (migrate by removing the 'status' field and \
             setting schema_version to 5)"
        )));
    }

    // Reject any file that still carries legacy fields from pre-v5 schemas.
    // `status`, `tasks`, and `plan` are the three v4/v3 fields that must not
    // appear in v5 identity-only metadata. Any of these indicates an
    // incomplete migration; fail closed rather than silently ignoring them.
    for legacy_field in ["status", "tasks", "plan"] {
        if raw.get(legacy_field).is_some() {
            return Err(CodecError::Validation(format!(
                "metadata.json has a '{legacy_field}' field which is not valid in schema v5; \
                 remove '{legacy_field}' (and any other v4 fields) and set schema_version to 5"
            )));
        }
    }

    let doc: TrackDocumentV2 = serde_json::from_value(raw)?;

    let meta = DocumentMeta {
        schema_version: doc.schema_version,
        created_at: doc.created_at.clone(),
        updated_at: doc.updated_at.clone(),
    };
    let track = track_metadata_from_document(doc)?;
    Ok((track, meta))
}

/// Encodes a domain `TrackMetadata` and infrastructure `DocumentMeta` into a JSON string.
///
/// Always writes `schema_version = 5` (no `status` field).
///
/// # Errors
/// Returns `CodecError` on JSON serialization failure.
pub fn encode(track: &TrackMetadata, meta: &DocumentMeta) -> Result<String, CodecError> {
    let doc = document_from_track_metadata(track, meta);
    let mut value = serde_json::to_value(&doc)?;
    // Preserve `branch: null` for planning-only tracks (schema_version 5 + no branch).
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

    let status_override =
        doc.status_override.map(|o| parse_status_override(&o.status, o.reason)).transpose()?;

    let track = TrackMetadata::with_branch(id, branch, doc.title, status_override)?;

    Ok(track)
}

fn document_from_track_metadata(track: &TrackMetadata, meta: &DocumentMeta) -> TrackDocumentV2 {
    TrackDocumentV2 {
        schema_version: 5,
        id: track.id().to_string(),
        branch: track.branch().map(|b| b.to_string()),
        title: track.title().to_string(),
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        status_override: track.status_override().map(override_to_document),
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

    /// Minimal identity-only metadata.json (schema_version = 5, no status field).
    fn sample_json_v5() -> &'static str {
        r#"{
  "schema_version": 5,
  "id": "test-track",
  "title": "Test Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#
    }

    /// Legacy v4 metadata.json (has `status` field) — must be rejected.
    fn sample_json_v4_legacy() -> &'static str {
        r#"{
  "schema_version": 4,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#
    }

    /// Legacy v2 metadata.json (has tasks/plan) — must be rejected.
    fn sample_json_legacy_v2() -> &'static str {
        r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "First task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": []
  }
}"#
    }

    #[test]
    fn test_decode_v5_identity_only_json_returns_track_metadata() {
        let (track, meta) = decode(sample_json_v5()).unwrap();
        assert_eq!(track.id().as_ref(), "test-track");
        assert_eq!(track.title(), "Test Track");
        assert!(track.status_override().is_none());
        assert_eq!(meta.schema_version, 5);
        assert_eq!(meta.created_at, "2026-03-11T00:00:00Z");
    }

    #[test]
    fn test_decode_v4_legacy_with_status_field_returns_error() {
        // v4 files have a `status` field — must be rejected in schema v5.
        let result = decode(sample_json_v4_legacy());
        assert!(result.is_err(), "v4 metadata with status field must be rejected");
        if let Err(CodecError::Validation(msg)) = result {
            assert!(
                msg.contains("schema_version 4") || msg.contains("schema v5"),
                "error must mention v4 or schema v5: {msg}"
            );
        } else {
            panic!("expected Validation error");
        }
    }

    #[test]
    fn test_decode_legacy_v2_returns_error() {
        // Legacy v2 files are no longer accepted.
        let result = decode(sample_json_legacy_v2());
        assert!(result.is_err(), "legacy v2 must be rejected");
    }

    #[test]
    fn test_encode_always_writes_schema_version_5() {
        let (track, meta) = decode(sample_json_v5()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(doc["schema_version"].as_u64().unwrap(), 5);
    }

    #[test]
    fn test_encode_v5_then_decode_round_trip() {
        let (track, meta) = decode(sample_json_v5()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let (track2, meta2) = decode(&json).unwrap();
        assert_eq!(track, track2);
        assert_eq!(meta.schema_version, meta2.schema_version);
    }

    #[test]
    fn test_encode_does_not_emit_status_field() {
        let (track, meta) = decode(sample_json_v5()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(doc.get("status").is_none(), "status must NOT be emitted in v5 output");
    }

    #[test]
    fn test_decode_with_status_override_blocked() {
        let json = r#"{
  "schema_version": 5,
  "id": "blocked-track",
  "title": "Blocked Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "status_override": {"status": "blocked", "reason": "waiting on review"}
}"#;
        let (track, _meta) = decode(json).unwrap();
        // derive_track_status with override = Blocked
        assert_eq!(
            domain::derive_track_status(None, track.status_override()),
            TrackStatus::Blocked
        );
        assert!(track.status_override().is_some());
        assert_eq!(track.status_override().unwrap().reason(), "waiting on review");
    }

    #[test]
    fn test_decode_invalid_json_returns_error() {
        let result = decode("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_branchless_track_has_null_branch_field() {
        let (track, meta) = decode(sample_json_v5()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Branchless track must emit `"branch": null`
        assert!(doc.get("branch").is_some(), "branch field must be present");
        assert!(doc["branch"].is_null(), "branch must be null for branchless track");
    }

    #[test]
    fn test_encode_with_branch_emits_branch_string() {
        let json = r#"{
  "schema_version": 5,
  "id": "branched-track",
  "branch": "track/branched-track",
  "title": "Branched Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        assert_eq!(doc["branch"].as_str().unwrap(), "track/branched-track");
    }

    #[test]
    fn test_decode_no_tasks_or_plan_fields_in_v5_output() {
        let (track, meta) = decode(sample_json_v5()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Identity-only: tasks and plan must NOT appear in the encoded output.
        assert!(doc.get("tasks").is_none(), "tasks must not be emitted in v5 output");
        assert!(doc.get("plan").is_none(), "plan must not be emitted in v5 output");
    }

    #[test]
    fn test_decode_v5_with_stale_tasks_field_is_rejected() {
        // A metadata.json that was partially migrated (schema_version bumped to 5
        // but `tasks` not yet removed) must be rejected. This is an incomplete
        // migration — fail closed rather than silently ignoring legacy fields.
        let json = r#"{
  "schema_version": 5,
  "id": "partial-migration",
  "title": "Partial Migration",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [{"id": "T1", "description": "leftover", "status": "todo"}]
}"#;
        let result = decode(json);
        assert!(result.is_err(), "v5 doc with stale `tasks` field must be rejected");
        if let Err(CodecError::Validation(msg)) = result {
            assert!(msg.contains("tasks"), "error must mention the offending field: {msg}");
        } else {
            panic!("expected Validation error, got: {result:?}");
        }
    }

    #[test]
    fn test_decode_v5_with_stale_plan_field_is_rejected() {
        // Similarly, a `plan` field in schema v5 indicates an incomplete migration.
        let json = r#"{
  "schema_version": 5,
  "id": "partial-migration-plan",
  "title": "Partial Migration Plan",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "plan": {"summary": [], "sections": []}
}"#;
        let result = decode(json);
        assert!(result.is_err(), "v5 doc with stale `plan` field must be rejected");
        if let Err(CodecError::Validation(msg)) = result {
            assert!(msg.contains("plan"), "error must mention the offending field: {msg}");
        } else {
            panic!("expected Validation error, got: {result:?}");
        }
    }

    #[test]
    fn test_decode_v5_with_unknown_top_level_field_is_rejected() {
        // `deny_unknown_fields` regression: any unknown top-level field that is not
        // one of the explicitly filtered legacy fields (status/tasks/plan) must still
        // be rejected. This guards against future typos or unanticipated extensions
        // silently passing through the codec.
        let json = r#"{
  "schema_version": 5,
  "id": "unknown-field-track",
  "title": "Unknown Field Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "extra_field": "should not be here"
}"#;
        let result = decode(json);
        assert!(result.is_err(), "v5 doc with unknown top-level field must be rejected");
    }

    #[test]
    fn test_decode_v5_with_unknown_field_in_status_override_is_rejected() {
        // `deny_unknown_fields` regression on nested `TrackStatusOverrideDocument`:
        // a status_override object with an unexpected field must be rejected rather
        // than silently dropped.
        let json = r#"{
  "schema_version": 5,
  "id": "override-unknown-track",
  "title": "Override Unknown Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "status_override": {"status": "blocked", "reason": "waiting on review", "extra": "surprise"}
}"#;
        let result = decode(json);
        assert!(result.is_err(), "status_override with unknown field must be rejected");
    }

    #[test]
    fn test_decode_schema_version_overflowing_u32_returns_validation_error() {
        // Regression guard for the `u32::try_from` fix: a schema_version value that
        // exceeds u32::MAX and wraps to the supported v5 value under the old `as u32`
        // cast must be rejected with a Validation error.
        //
        // 4294967301 = 2^32 + 5. Under `v as u32` this truncates to 5, which is the
        // valid schema v5 — the codec would silently accept the file as a legitimate
        // v5 metadata.json. With `u32::try_from` it must be rejected.
        let json = r#"{
  "schema_version": 4294967301,
  "id": "overflow-track",
  "title": "Overflow Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#;
        let result = decode(json);
        assert!(result.is_err(), "schema_version > u32::MAX must be rejected");
        assert!(
            matches!(result, Err(CodecError::Validation(_))),
            "expected Validation error for overflow schema_version, got: {result:?}"
        );
    }
}
