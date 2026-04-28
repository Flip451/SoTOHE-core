//! Error type for ADR YAML front-matter codec failures.

use thiserror::Error;

/// Errors raised by [`super::parse::parse_adr_frontmatter`] when an ADR file's
/// YAML front-matter cannot be decoded into the domain
/// [`domain::AdrFrontMatter`] aggregate.
///
/// Variants:
///
/// - [`AdrFrontMatterCodecError::YamlParse`] — `serde_yaml` raw parse failure
///   (syntax errors, unknown schema keys via `deny_unknown_fields`, type
///   mismatches, missing required fields).
/// - [`AdrFrontMatterCodecError::MissingAdrId`] — the ADR file has no
///   front-matter block at all (fail-closed per CN-04) **or** the parsed
///   `adr_id` field is empty.
/// - [`AdrFrontMatterCodecError::InvalidDecisionField`] — a per-decision
///   schema invariant was violated (unknown status string, forbidden /
///   missing typestate-specific field, empty domain identifier).
#[derive(Debug, Error)]
pub enum AdrFrontMatterCodecError {
    /// Raw YAML parse failure (syntax error, schema mismatch, etc.).
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    /// The ADR file has no YAML front-matter block, or the `adr_id` field
    /// is missing / empty.
    #[error("ADR front-matter is missing the required `adr_id` field")]
    MissingAdrId,

    /// A `decisions[]` entry violated a typestate-specific schema invariant.
    #[error("invalid decision field: {0}")]
    InvalidDecisionField(String),
}
