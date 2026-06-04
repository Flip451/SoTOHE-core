//! Private serde-only codec types for dry-check.json on-disk format.
//!
//! All serde derives live exclusively on these types. Domain types (`DryCheckEntry`,
//! `DryCheckRecord`, etc.) remain serde-free (hexagonal principle).
//!
//! On-disk format: `{ "schema_version": 1, "records": [...] }` where each record
//! uses a FLAT 4-identifier layout: `low_path`, `low_hash`, `high_path`, `high_hash`
//! for the `DryCheckPairKey`, plus the remaining 7 entry fields and `recorded_at`.

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
}

// ── DryCheckJsonV1 ────────────────────────────────────────────────────────────

/// Private serde-only envelope for dry-check.json.
///
/// Schema version 1. Mirrors `ReviewJsonV2 { schema_version, scopes }` from
/// `FsReviewStore`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct DryCheckJsonV1 {
    pub schema_version: u32,
    pub records: Vec<DryCheckRecordDto>,
}

impl DryCheckJsonV1 {
    /// Create an empty envelope with `schema_version = 1`.
    pub(super) fn empty() -> Self {
        Self { schema_version: 1, records: Vec::new() }
    }
}
