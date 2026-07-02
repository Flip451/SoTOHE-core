//! Serde types for metadata.json (TrackDocumentV2) — identity-only shape.
//!
//! Per ADR 2026-04-19-1242 §D1.4, `tasks` and `plan` fields are removed from
//! `TrackDocumentV2`; they now live in `impl-plan.json` (ImplPlanDocument).
//! The `status` field is also removed from metadata.json; track status is
//! derived on demand via `domain::derive_track_status`. Schema version is 6.
//!
//! v6 adds `branch_strategy_snapshot` (a required non-optional field).
//! All previous schema versions are rejected on decode — no backward-compat.

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

/// Serde-deserializable mirror of domain::branch_strategy::MergeMethod.
/// R9: finite set must be a typed enum, not String.
///
/// Serialize and Deserialize are implemented manually (not via derive) to avoid
/// a nightly-rustdoc `TrivialClone` auto-impl appearing as an undeclared extra
/// item in the TDDD signal checker when Copy + derive(Serialize, Deserialize)
/// are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethodDocument {
    Squash,
    Merge,
    Rebase,
}

impl serde::Serialize for MergeMethodDocument {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            MergeMethodDocument::Squash => "squash",
            MergeMethodDocument::Merge => "merge",
            MergeMethodDocument::Rebase => "rebase",
        })
    }
}

impl<'de> serde::Deserialize<'de> for MergeMethodDocument {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "squash" => Ok(MergeMethodDocument::Squash),
            "merge" => Ok(MergeMethodDocument::Merge),
            "rebase" => Ok(MergeMethodDocument::Rebase),
            other => Err(serde::de::Error::unknown_variant(other, &["squash", "merge", "rebase"])),
        }
    }
}

/// Serde DTO for the branch_strategy_snapshot sub-field of metadata.json.
/// base_branch/merge_target are String (arbitrary branch names validated at
/// domain conversion); merge_method is typed as MergeMethodDocument (R9 finite set).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchStrategySnapshotDocument {
    pub base_branch: String,
    pub merge_target: String,
    pub merge_method: MergeMethodDocument,
}

fn merge_method_from_document(doc: MergeMethodDocument) -> domain::branch_strategy::MergeMethod {
    match doc {
        MergeMethodDocument::Squash => domain::branch_strategy::MergeMethod::Squash,
        MergeMethodDocument::Merge => domain::branch_strategy::MergeMethod::Merge,
        MergeMethodDocument::Rebase => domain::branch_strategy::MergeMethod::Rebase,
    }
}

fn merge_method_to_document(m: domain::branch_strategy::MergeMethod) -> MergeMethodDocument {
    match m {
        domain::branch_strategy::MergeMethod::Squash => MergeMethodDocument::Squash,
        domain::branch_strategy::MergeMethod::Merge => MergeMethodDocument::Merge,
        domain::branch_strategy::MergeMethod::Rebase => MergeMethodDocument::Rebase,
    }
}

fn snapshot_from_document(
    doc: BranchStrategySnapshotDocument,
) -> Result<domain::branch_strategy::BranchStrategySnapshot, CodecError> {
    let base = domain::NonEmptyString::try_new(&doc.base_branch)
        .map_err(|e| CodecError::Domain(domain::DomainError::Validation(e)))?;
    let target = domain::NonEmptyString::try_new(&doc.merge_target)
        .map_err(|e| CodecError::Domain(domain::DomainError::Validation(e)))?;
    let method = merge_method_from_document(doc.merge_method);
    Ok(domain::branch_strategy::BranchStrategySnapshot::new(base, target, method))
}

fn snapshot_to_document(
    snap: &domain::branch_strategy::BranchStrategySnapshot,
) -> BranchStrategySnapshotDocument {
    BranchStrategySnapshotDocument {
        base_branch: snap.base_branch().to_owned(),
        merge_target: snap.merge_target().to_owned(),
        merge_method: merge_method_to_document(snap.merge_method()),
    }
}

/// Identity-only DTO for `metadata.json` (schema_version = 6).
///
/// Per ADR 2026-04-19-1242 §D1.4:
/// - `status` is removed; track status is derived on demand from `impl-plan.json`.
/// - `status_override` is kept for Blocked/Cancelled semantics.
/// - `tasks` and `plan` are removed — they live in `impl-plan.json`.
/// - `branch_strategy_snapshot` is required (added in v6).
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
    pub branch_strategy_snapshot: BranchStrategySnapshotDocument,
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
/// Accepts only `schema_version = 6` (identity-only with `branch_strategy_snapshot`).
/// All previous schema versions are rejected with a clear error message.
///
/// # Errors
/// Returns `CodecError` on JSON parse failure, schema version mismatch, or domain
/// validation failure.
pub fn decode(json: &str) -> Result<(TrackMetadata, DocumentMeta), CodecError> {
    // Peek at schema_version before full decode. Use `u32::try_from` to reject
    // values that overflow u32 instead of silently wrapping (e.g., `4294967302`
    // must not truncate to `6` and be misclassified as valid v6 metadata).
    let raw: serde_json::Value = serde_json::from_str(json)?;
    let schema_version_u64 = raw.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let schema_version = u32::try_from(schema_version_u64).map_err(|_| {
        CodecError::Validation(format!(
            "metadata.json schema_version {schema_version_u64} overflows u32; \
             schema v6 is required"
        ))
    })?;
    if schema_version != 6 {
        return Err(CodecError::Validation(format!(
            "metadata.json schema_version {schema_version} is not supported; \
             schema v6 is required (add branch_strategy_snapshot field and \
             set schema_version to 6)"
        )));
    }

    // Reject any file that still carries legacy fields from pre-v6 schemas.
    // `status`, `tasks`, and `plan` are the three v4/v3 fields that must not
    // appear in v6 identity-only metadata. Any of these indicates an
    // incomplete migration; fail closed rather than silently ignoring them.
    for legacy_field in ["status", "tasks", "plan"] {
        if raw.get(legacy_field).is_some() {
            return Err(CodecError::Validation(format!(
                "metadata.json has a '{legacy_field}' field which is not valid in schema v6; \
                 remove '{legacy_field}' (and any other legacy fields) and set schema_version to 6"
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
/// Always writes `schema_version = 6` (with `branch_strategy_snapshot`, no `status` field).
///
/// # Errors
/// Returns `CodecError` on JSON serialization failure.
pub fn encode(track: &TrackMetadata, meta: &DocumentMeta) -> Result<String, CodecError> {
    let doc = document_from_track_metadata(track, meta);
    let mut value = serde_json::to_value(&doc)?;
    // Preserve `branch: null` for planning-only tracks (schema_version 6 + no branch).
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

    let branch_strategy_snapshot = snapshot_from_document(doc.branch_strategy_snapshot)?;

    let track = TrackMetadata::with_branch(
        id,
        branch,
        doc.title,
        status_override,
        branch_strategy_snapshot,
    )?;

    Ok(track)
}

fn document_from_track_metadata(track: &TrackMetadata, meta: &DocumentMeta) -> TrackDocumentV2 {
    TrackDocumentV2 {
        schema_version: 6,
        id: track.id().to_string(),
        branch: track.branch().map(|b| b.to_string()),
        title: track.title().to_string(),
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        status_override: track.status_override().map(override_to_document),
        branch_strategy_snapshot: snapshot_to_document(track.branch_strategy_snapshot()),
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

    /// Minimal identity-only metadata.json (schema_version = 6, with branch_strategy_snapshot).
    fn sample_json_v6() -> &'static str {
        r#"{
  "schema_version": 6,
  "id": "test-track",
  "title": "Test Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"}
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
    fn test_decode_v6_identity_only_json_returns_track_metadata() {
        let (track, meta) = decode(sample_json_v6()).unwrap();
        assert_eq!(track.id().as_ref(), "test-track");
        assert_eq!(track.title(), "Test Track");
        assert!(track.status_override().is_none());
        assert_eq!(meta.schema_version, 6);
        assert_eq!(meta.created_at, "2026-03-11T00:00:00Z");
    }

    #[test]
    fn test_decode_v4_legacy_with_status_field_returns_error() {
        // v4 files have a `status` field — must be rejected in schema v6.
        let result = decode(sample_json_v4_legacy());
        assert!(result.is_err(), "v4 metadata with status field must be rejected");
        if let Err(CodecError::Validation(msg)) = result {
            assert!(
                msg.contains("schema_version 4") || msg.contains("schema v6"),
                "error must mention v4 or schema v6: {msg}"
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
    fn test_encode_always_writes_schema_version_6() {
        let (track, meta) = decode(sample_json_v6()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(doc["schema_version"].as_u64().unwrap(), 6);
    }

    #[test]
    fn test_encode_v6_then_decode_round_trip() {
        let (track, meta) = decode(sample_json_v6()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let (track2, meta2) = decode(&json).unwrap();
        assert_eq!(track, track2);
        assert_eq!(meta.schema_version, meta2.schema_version);
    }

    #[test]
    fn test_encode_does_not_emit_status_field() {
        let (track, meta) = decode(sample_json_v6()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(doc.get("status").is_none(), "status must NOT be emitted in v6 output");
    }

    #[test]
    fn test_decode_with_status_override_blocked() {
        let json = r#"{
  "schema_version": 6,
  "id": "blocked-track",
  "title": "Blocked Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"},
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
        let (track, meta) = decode(sample_json_v6()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Branchless track must emit `"branch": null`
        assert!(doc.get("branch").is_some(), "branch field must be present");
        assert!(doc["branch"].is_null(), "branch must be null for branchless track");
    }

    #[test]
    fn test_encode_with_branch_emits_branch_string() {
        let json = r#"{
  "schema_version": 6,
  "id": "branched-track",
  "branch": "track/branched-track",
  "title": "Branched Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"}
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        assert_eq!(doc["branch"].as_str().unwrap(), "track/branched-track");
    }

    #[test]
    fn test_decode_no_tasks_or_plan_fields_in_v6_output() {
        let (track, meta) = decode(sample_json_v6()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Identity-only: tasks and plan must NOT appear in the encoded output.
        assert!(doc.get("tasks").is_none(), "tasks must not be emitted in v6 output");
        assert!(doc.get("plan").is_none(), "plan must not be emitted in v6 output");
    }

    #[test]
    fn test_decode_v6_with_stale_tasks_field_is_rejected() {
        // A metadata.json that was partially migrated (schema_version bumped to 6
        // but `tasks` not yet removed) must be rejected. This is an incomplete
        // migration — fail closed rather than silently ignoring legacy fields.
        let json = r#"{
  "schema_version": 6,
  "id": "partial-migration",
  "title": "Partial Migration",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"},
  "tasks": [{"id": "T1", "description": "leftover", "status": "todo"}]
}"#;
        let result = decode(json);
        assert!(result.is_err(), "v6 doc with stale `tasks` field must be rejected");
        if let Err(CodecError::Validation(msg)) = result {
            assert!(msg.contains("tasks"), "error must mention the offending field: {msg}");
        } else {
            panic!("expected Validation error, got: {result:?}");
        }
    }

    #[test]
    fn test_decode_v6_with_stale_plan_field_is_rejected() {
        // Similarly, a `plan` field in schema v6 indicates an incomplete migration.
        let json = r#"{
  "schema_version": 6,
  "id": "partial-migration-plan",
  "title": "Partial Migration Plan",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"},
  "plan": {"summary": [], "sections": []}
}"#;
        let result = decode(json);
        assert!(result.is_err(), "v6 doc with stale `plan` field must be rejected");
        if let Err(CodecError::Validation(msg)) = result {
            assert!(msg.contains("plan"), "error must mention the offending field: {msg}");
        } else {
            panic!("expected Validation error, got: {result:?}");
        }
    }

    #[test]
    fn test_decode_v6_with_unknown_top_level_field_is_rejected() {
        // `deny_unknown_fields` regression: any unknown top-level field that is not
        // one of the explicitly filtered legacy fields (status/tasks/plan) must still
        // be rejected. This guards against future typos or unanticipated extensions
        // silently passing through the codec.
        let json = r#"{
  "schema_version": 6,
  "id": "unknown-field-track",
  "title": "Unknown Field Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"},
  "extra_field": "should not be here"
}"#;
        let result = decode(json);
        assert!(result.is_err(), "v6 doc with unknown top-level field must be rejected");
    }

    #[test]
    fn test_decode_v6_with_unknown_field_in_status_override_is_rejected() {
        // `deny_unknown_fields` regression on nested `TrackStatusOverrideDocument`:
        // a status_override object with an unexpected field must be rejected rather
        // than silently dropped.
        let json = r#"{
  "schema_version": 6,
  "id": "override-unknown-track",
  "title": "Override Unknown Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"},
  "status_override": {"status": "blocked", "reason": "waiting on review", "extra": "surprise"}
}"#;
        let result = decode(json);
        assert!(result.is_err(), "status_override with unknown field must be rejected");
    }

    #[test]
    fn test_decode_schema_version_overflowing_u32_returns_validation_error() {
        // Regression guard for the `u32::try_from` fix: a schema_version value that
        // exceeds u32::MAX and wraps to the supported v6 value under the old `as u32`
        // cast must be rejected with a Validation error.
        //
        // 4294967302 = 2^32 + 6. Under `v as u32` this truncates to 6, which is the
        // valid schema v6 — the codec would silently accept the file as a legitimate
        // v6 metadata.json. With `u32::try_from` it must be rejected.
        let json = r#"{
  "schema_version": 4294967302,
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

    #[test]
    fn test_decode_missing_branch_strategy_snapshot_returns_err() {
        // AC-10: payload without branch_strategy_snapshot must fail
        let json = r#"{
  "schema_version": 6,
  "id": "test-track",
  "title": "Test Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z"
}"#;
        let result = decode(json);
        assert!(result.is_err(), "missing branch_strategy_snapshot must return Err");
    }

    #[test]
    fn test_merge_method_lowercase_round_trip() {
        // "squash" / "merge" / "rebase" must round-trip through MergeMethodDocument
        let cases = [
            (r#""squash""#, MergeMethodDocument::Squash),
            (r#""merge""#, MergeMethodDocument::Merge),
            (r#""rebase""#, MergeMethodDocument::Rebase),
        ];
        for (json_str, expected) in cases {
            let parsed: MergeMethodDocument = serde_json::from_str(json_str).unwrap();
            assert_eq!(parsed, expected);
            let re_serialized = serde_json::to_string(&parsed).unwrap();
            assert_eq!(re_serialized, json_str);
        }
    }

    #[test]
    fn test_decode_with_kind_field_is_rejected() {
        // AC-07 (D3 defer): a payload containing a `kind` field must be rejected
        // because TrackDocumentV2 uses deny_unknown_fields.
        let json = r#"{
  "schema_version": 6,
  "id": "test-track",
  "title": "Test Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {"base_branch": "main", "merge_target": "main", "merge_method": "squash"},
  "kind": "feature"
}"#;
        let result = decode(json);
        assert!(
            result.is_err(),
            "payload with `kind` field must be rejected by deny_unknown_fields"
        );
    }

    #[test]
    fn test_encode_does_not_emit_kind_field() {
        // AC-07: newly serialized metadata must not contain `kind` field
        let (track, meta) = decode(sample_json_v6()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(doc.get("kind").is_none(), "kind must NOT be emitted");
    }
}
