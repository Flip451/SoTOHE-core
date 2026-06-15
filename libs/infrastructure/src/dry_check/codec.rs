//! Private serde-only codec types for dry-check.json on-disk format.
//!
//! All serde derives live exclusively on these types. Domain types (`DryCheckEntry`,
//! `DryCheckRecord`, etc.) remain serde-free (hexagonal principle).
//!
//! On-disk format (v2): `{ "schema_version": 2, "records": [...] }` where each
//! record uses a FLAT 4-identifier layout: `low_path`, `low_hash`, `high_path`,
//! `high_hash` for the `DryCheckPairKey`, plus the remaining 7 entry fields,
//! `recorded_at`, and `config_fingerprint`.
//!
//! ## Schema migration
//!
//! v1 → v2: added `config_fingerprint` field. v1 records on disk do not carry
//! this field. On deserialization from v1, `config_fingerprint` defaults to the
//! all-zeros sentinel (`DryCheckConfigFingerprint::fail_closed()`) via the serde
//! `default = "fail_closed_fingerprint_string"` attribute. The sentinel will
//! never match any real config fingerprint, so the interactor skips all v1
//! records from the `verified_set` and re-judges them under the current config,
//! effectively forcing a full re-scan after the schema upgrade.
//!
//! Malformed persisted `config_fingerprint` values (valid UTF-8 but not 64-char
//! lowercase hex) are also mapped to the sentinel in `dto_to_domain` so that a
//! single corrupted historical row does not make the full dry-check history
//! unreadable.

// ── DryCheckVerdictDto ────────────────────────────────────────────────────────

/// Private serde-only representation of `DryCheckVerdict`.
///
/// - `NotAViolation` / `Accepted` serialize as bare string tags.
/// - `Violation` serializes as `{ "violation": { "refactor_proposal": "..." } }`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) enum DryCheckVerdictDto {
    #[serde(rename = "not-a-violation")]
    NotAViolation,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "violation")]
    Violation { refactor_proposal: String },
}

// ── DryCheckRecordDto ─────────────────────────────────────────────────────────

/// Private serde-only representation of a single dry-check history record.
///
/// The `DryCheckPairKey` is flattened into four sibling string fields:
/// `low_path`, `low_hash`, `high_path`, `high_hash`.
///
/// `config_fingerprint` was added in schema v2. v1 records that lack this field
/// on disk deserialize to the all-zeros sentinel via
/// `serde(default = "fail_closed_fingerprint_string")`. Malformed values (not
/// valid 64-char hex) are mapped to the sentinel in `dto_to_domain` so that one
/// corrupted historical row does not abort the entire `read_records()` call.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct DryCheckRecordDto {
    pub low_path: String,
    pub low_hash: String,
    pub high_path: String,
    pub high_hash: String,
    pub changed_path: String,
    pub verdict: DryCheckVerdictDto,
    pub similarity_score: f64,
    pub threshold: f64,
    pub base_commit: String,
    pub rationale: String,
    pub recorded_at: String,
    /// Config fingerprint embedded when this record was written (schema v2+).
    ///
    /// Defaults to the all-zeros fail-closed sentinel when absent (v1 records).
    /// Always serialised (no `skip_serializing_if`) so every record written by
    /// this implementation carries the field, regardless of whether it was
    /// originally read from a v1 file.
    #[serde(default = "fail_closed_fingerprint_string")]
    pub config_fingerprint: String,
}

/// Returns the all-zeros fail-closed sentinel used for missing `config_fingerprint`
/// fields (v1 records without the field on disk).
fn fail_closed_fingerprint_string() -> String {
    "0".repeat(64)
}

// ── DryCheckJsonV2 ────────────────────────────────────────────────────────────

/// Private serde-only envelope for dry-check.json.
///
/// Schema version 2 (bumped from v1 to add `config_fingerprint` per record).
/// Mirrors `ReviewJsonV2 { schema_version, scopes }` from `FsReviewStore`.
///
/// The type alias `DryCheckJsonV1` is kept to avoid cascading renames across the
/// store module; it now always writes and reads `schema_version: 2` records.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct DryCheckJsonV1 {
    pub schema_version: u32,
    pub records: Vec<DryCheckRecordDto>,
}

impl DryCheckJsonV1 {
    /// Create an empty envelope with `schema_version = 2`.
    pub(super) fn empty() -> Self {
        Self { schema_version: 2, records: Vec::new() }
    }
}
