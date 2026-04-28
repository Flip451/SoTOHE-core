//! Serde DTOs mirroring the ADR YAML front-matter schema.
//!
//! These DTOs are infrastructure-only: serde derives live here per the CN-05
//! hexagonal rule. Domain types ([`domain::AdrFrontMatter`],
//! [`domain::AdrDecisionEntry`], …) are constructed from these DTOs by the
//! [`super::parse::parse_adr_frontmatter`] free function.

use serde::Deserialize;

/// Top-level DTO for an ADR file's YAML front-matter block.
///
/// `deny_unknown_fields` rejects unrecognised top-level keys to enforce the
/// schema contract at the codec boundary.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdrFrontMatterDto {
    /// Stable identifier for the ADR file (typically the filename slug).
    pub adr_id: String,
    /// Lifecycle decision entries in the order they appear in the front-matter.
    #[serde(default)]
    pub decisions: Vec<AdrDecisionDto>,
}

/// DTO for a single element of the `decisions[]` array.
///
/// `deny_unknown_fields` rejects unrecognised keys so schema drift is caught
/// at the codec boundary. Note that `serde` cannot distinguish `key: null`
/// from `key absent` for `Option<T>` fields — the key-presence check in
/// [`super::parse::parse_adr_frontmatter`] inspects the raw `serde_yaml::Value`
/// before deserialization to enforce typestate-specific field invariants.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdrDecisionDto {
    /// Short decision identifier (e.g. `"D1"`). Required.
    pub id: String,
    /// Reference to the user's explicit approval, if any.
    #[serde(default)]
    pub user_decision_ref: Option<String>,
    /// Reference to the review-process finding that produced this decision, if any.
    #[serde(default)]
    pub review_finding_ref: Option<String>,
    /// Free-form note on candidate selection rationale, if any.
    #[serde(default)]
    pub candidate_selection: Option<String>,
    /// Lifecycle state string. One of: `proposed` / `accepted` / `implemented`
    /// / `superseded` / `deprecated`. Validated and dispatched by
    /// [`super::parse::parse_adr_frontmatter`].
    pub status: String,
    /// ADR anchor reference of the superseding decision. Required when
    /// `status == "superseded"`, forbidden otherwise.
    #[serde(default)]
    pub superseded_by: Option<String>,
    /// Commit hash or reference where this decision was actualized. Required
    /// when `status == "implemented"`, forbidden otherwise.
    #[serde(default)]
    pub implemented_in: Option<String>,
    /// When `true`, the `verify-adr-signals` check skips this decision.
    /// Defaults to `false` when absent.
    #[serde(default)]
    pub grandfathered: Option<bool>,
}
