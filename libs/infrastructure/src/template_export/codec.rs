//! Serde codec for the template boundary manifest (boundary SSoT).
//!
//! The manifest is the single machine-readable source of truth that classifies
//! every workspace path as `include` / `exclude` / `overlay` (spec IN-02,
//! AC-02, CN-02). Decoding is fail-closed (spec IN-12): an unsupported schema
//! version, an unknown field, an invalid path pattern, or a duplicate/empty
//! entry set all surface as an error rather than a silently degraded manifest.
//! See ADR `2026-07-06-1717-template-extraction-boundary.md` D4.

use domain::{
    FreeText, TemplateBoundaryManifest, TemplateBoundaryManifestError, TemplatePathClassification,
    TemplatePathEntry, TemplatePathPattern, TemplatePathPatternError,
};
use serde::Deserialize;

/// Schema version understood by [`decode_manifest`].
///
/// The manifest file carries an explicit `schema_version` so the reader can
/// reject an incompatible layout fail-closed (spec IN-02, CN-02).
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error produced by [`decode_manifest`] (spec IN-02, AC-02, CN-02).
///
/// Every variant is a fail-closed decode rejection (spec IN-12): the caller
/// never receives a partially valid manifest.
#[derive(Debug, thiserror::Error)]
pub enum TemplateBoundaryManifestCodecError {
    /// The manifest declares a schema version this reader does not understand.
    #[error("unsupported manifest schema_version: expected {expected}, got {actual}")]
    SchemaVersion {
        /// Schema version this reader supports ([`MANIFEST_SCHEMA_VERSION`]).
        expected: u32,
        /// Schema version found in the manifest.
        actual: u32,
    },

    /// The manifest JSON is malformed or violates the wire schema.
    #[error("manifest JSON error: {reason}")]
    Json {
        /// Human-readable diagnostic from the JSON deserializer.
        reason: FreeText,
    },

    /// A manifest entry carries a pattern that is not workspace-relative.
    #[error("invalid manifest path pattern: {source}")]
    Pattern {
        /// The underlying domain validation error.
        #[from]
        source: TemplatePathPatternError,
    },

    /// The decoded entries violate a manifest invariant (empty or duplicate).
    #[error("invalid manifest: {source}")]
    Manifest {
        /// The underlying domain validation error.
        #[from]
        source: TemplateBoundaryManifestError,
    },
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Manifest wire DTO (spec IN-02, AC-02, CN-02).
///
/// `deny_unknown_fields` rejects unrecognised keys so an out-of-schema manifest
/// fails closed (spec IN-12). `entries` is required: a missing field is an
/// error rather than an implicit empty manifest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateBoundaryManifestDto {
    /// Declared manifest schema version, checked against [`MANIFEST_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The classified path rows in declaration order.
    pub entries: Vec<TemplatePathEntryDto>,
}

/// Manifest-entry wire DTO (spec IN-02, AC-02).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplatePathEntryDto {
    /// Workspace-relative path pattern, validated on decode into
    /// [`TemplatePathPattern`].
    pub pattern: String,
    /// Boundary classification for the pattern.
    pub classification: TemplatePathClassificationDto,
}

#[derive(Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: u32,
}

/// `domain::template_export::TemplatePathClassification` serde mirror
/// (spec IN-02, AC-02).
///
/// The wire form is the lowercase discriminant (`include` / `exclude` /
/// `overlay`) matching the manifest schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplatePathClassificationDto {
    /// Include the path in the exported template unchanged.
    Include,
    /// Exclude the path from the exported template.
    Exclude,
    /// Overwrite the path with the overlay directory's template version.
    Overlay,
}

impl TemplatePathClassificationDto {
    /// Maps the wire mirror onto its domain classification.
    fn into_domain(self) -> TemplatePathClassification {
        match self {
            Self::Include => TemplatePathClassification::Include,
            Self::Exclude => TemplatePathClassification::Exclude,
            Self::Overlay => TemplatePathClassification::Overlay,
        }
    }
}

// ---------------------------------------------------------------------------
// Decode: JSON -> domain
// ---------------------------------------------------------------------------

/// Decodes a boundary-manifest JSON string into a [`TemplateBoundaryManifest`]
/// (spec IN-02, AC-02, CN-02).
///
/// Decoding is fail-closed (spec IN-12): the manifest must declare the supported
/// schema version, carry only known fields, and yield a non-empty, duplicate-free
/// set of workspace-relative patterns.
///
/// # Errors
///
/// - [`TemplateBoundaryManifestCodecError::Json`] if the JSON is malformed or
///   violates the wire schema (unknown field, missing field, wrong type).
/// - [`TemplateBoundaryManifestCodecError::SchemaVersion`] if `schema_version`
///   is not [`MANIFEST_SCHEMA_VERSION`].
/// - [`TemplateBoundaryManifestCodecError::Pattern`] if any entry pattern is not
///   workspace-relative.
/// - [`TemplateBoundaryManifestCodecError::Manifest`] if the entry set is empty
///   or contains a duplicate pattern.
pub fn decode_manifest(
    json: &str,
) -> Result<TemplateBoundaryManifest, TemplateBoundaryManifestCodecError> {
    let envelope: SchemaVersionEnvelope = serde_json::from_str(json).map_err(|e| {
        TemplateBoundaryManifestCodecError::Json { reason: FreeText::new(e.to_string()) }
    })?;

    if envelope.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(TemplateBoundaryManifestCodecError::SchemaVersion {
            expected: MANIFEST_SCHEMA_VERSION,
            actual: envelope.schema_version,
        });
    }

    let dto: TemplateBoundaryManifestDto = serde_json::from_str(json).map_err(|e| {
        TemplateBoundaryManifestCodecError::Json { reason: FreeText::new(e.to_string()) }
    })?;

    let entries = dto
        .entries
        .into_iter()
        .map(entry_from_dto)
        .collect::<Result<Vec<_>, TemplateBoundaryManifestCodecError>>()?;

    Ok(TemplateBoundaryManifest::try_new(entries)?)
}

fn entry_from_dto(
    dto: TemplatePathEntryDto,
) -> Result<TemplatePathEntry, TemplateBoundaryManifestCodecError> {
    let pattern = TemplatePathPattern::try_new(dto.pattern)?;
    Ok(TemplatePathEntry::new(pattern, dto.classification.into_domain()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const FULL_JSON: &str = r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "libs/domain", "classification": "include" },
    { "pattern": "Makefile.toml", "classification": "overlay" },
    { "pattern": "vendor/", "classification": "exclude" }
  ]
}"#;

    // --- decode: happy path ---

    #[test]
    fn test_decode_full_manifest_succeeds() {
        let manifest = decode_manifest(FULL_JSON).unwrap();
        assert_eq!(manifest.entries().len(), 3);

        let make = TemplatePathPattern::try_new("Makefile.toml".to_owned()).unwrap();
        assert_eq!(manifest.classify(&make), Some(TemplatePathClassification::Overlay));

        let domain = TemplatePathPattern::try_new("libs/domain".to_owned()).unwrap();
        assert_eq!(manifest.classify(&domain), Some(TemplatePathClassification::Include));

        let vendor = TemplatePathPattern::try_new("vendor/".to_owned()).unwrap();
        assert_eq!(manifest.classify(&vendor), Some(TemplatePathClassification::Exclude));
    }

    #[test]
    fn test_decode_preserves_entry_order() {
        let manifest = decode_manifest(FULL_JSON).unwrap();
        let patterns: Vec<&str> = manifest.entries().iter().map(|e| e.pattern().as_str()).collect();
        // `vendor/` in the JSON canonicalizes to `vendor` (trailing separator
        // stripped by `TemplatePathPattern::try_new`).
        assert_eq!(patterns, vec!["libs/domain", "Makefile.toml", "vendor"]);
    }

    #[test]
    fn test_decode_all_classifications_map_to_domain() {
        let json = r#"{
          "schema_version": 1,
          "entries": [
            { "pattern": "a", "classification": "include" },
            { "pattern": "b", "classification": "exclude" },
            { "pattern": "c", "classification": "overlay" }
          ]
        }"#;
        let manifest = decode_manifest(json).unwrap();
        let a = TemplatePathPattern::try_new("a".to_owned()).unwrap();
        let b = TemplatePathPattern::try_new("b".to_owned()).unwrap();
        let c = TemplatePathPattern::try_new("c".to_owned()).unwrap();
        assert_eq!(manifest.classify(&a), Some(TemplatePathClassification::Include));
        assert_eq!(manifest.classify(&b), Some(TemplatePathClassification::Exclude));
        assert_eq!(manifest.classify(&c), Some(TemplatePathClassification::Overlay));
    }

    // --- decode: schema_version validation ---

    #[test]
    fn test_decode_with_unsupported_schema_version_returns_error() {
        let json =
            r#"{"schema_version": 2, "entries": [{"pattern": "a", "classification": "include"}]}"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestCodecError::SchemaVersion { expected: 1, actual: 2 }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_with_future_schema_version_and_new_field_returns_schema_version_error() {
        let json = r#"{
          "schema_version": 2,
          "entries": [{"pattern": "a", "classification": "include"}],
          "future_field": "new schema payload"
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestCodecError::SchemaVersion { expected: 1, actual: 2 }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_with_schema_version_zero_returns_error() {
        let json =
            r#"{"schema_version": 0, "entries": [{"pattern": "a", "classification": "include"}]}"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(matches!(
            err,
            TemplateBoundaryManifestCodecError::SchemaVersion { expected: 1, actual: 0 }
        ));
    }

    // --- decode: wire-schema (Json) rejection ---

    #[test]
    fn test_decode_with_unknown_top_level_field_is_rejected() {
        let json = r#"{
          "schema_version": 1,
          "entries": [{"pattern": "a", "classification": "include"}],
          "extra": true
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(
            matches!(err, TemplateBoundaryManifestCodecError::Json { .. }),
            "expected Json error, got: {err}"
        );
    }

    #[test]
    fn test_decode_with_unknown_entry_field_is_rejected() {
        let json = r#"{
          "schema_version": 1,
          "entries": [{"pattern": "a", "classification": "include", "note": "x"}]
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(matches!(err, TemplateBoundaryManifestCodecError::Json { .. }));
    }

    #[test]
    fn test_decode_with_missing_entries_field_is_rejected() {
        let json = r#"{"schema_version": 1}"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(matches!(err, TemplateBoundaryManifestCodecError::Json { .. }));
    }

    #[test]
    fn test_decode_with_unknown_classification_is_rejected() {
        let json = r#"{
          "schema_version": 1,
          "entries": [{"pattern": "a", "classification": "vendor"}]
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(matches!(err, TemplateBoundaryManifestCodecError::Json { .. }));
    }

    #[test]
    fn test_decode_with_non_lowercase_classification_is_rejected() {
        // The wire form is lowercase; the capitalized variant name must not parse.
        let json = r#"{
          "schema_version": 1,
          "entries": [{"pattern": "a", "classification": "Include"}]
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(matches!(err, TemplateBoundaryManifestCodecError::Json { .. }));
    }

    #[test]
    fn test_decode_invalid_json_returns_json_error() {
        let err = decode_manifest("{not json}").unwrap_err();
        assert!(matches!(err, TemplateBoundaryManifestCodecError::Json { .. }));
    }

    // --- decode: domain-invariant (Pattern / Manifest) rejection ---

    #[test]
    fn test_decode_with_non_workspace_relative_pattern_returns_pattern_error() {
        let json = r#"{
          "schema_version": 1,
          "entries": [{"pattern": "../escape", "classification": "include"}]
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(
            matches!(err, TemplateBoundaryManifestCodecError::Pattern { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_with_empty_pattern_returns_pattern_error() {
        let json = r#"{
          "schema_version": 1,
          "entries": [{"pattern": "", "classification": "include"}]
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(matches!(err, TemplateBoundaryManifestCodecError::Pattern { .. }));
    }

    #[test]
    fn test_decode_empty_entries_returns_manifest_error() {
        let json = r#"{"schema_version": 1, "entries": []}"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(
            matches!(err, TemplateBoundaryManifestCodecError::Manifest { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_duplicate_pattern_returns_manifest_error() {
        let json = r#"{
          "schema_version": 1,
          "entries": [
            {"pattern": "libs/domain", "classification": "include"},
            {"pattern": "libs/domain", "classification": "exclude"}
          ]
        }"#;
        let err = decode_manifest(json).unwrap_err();
        assert!(matches!(err, TemplateBoundaryManifestCodecError::Manifest { .. }));
    }

    // --- decode: the real boundary SSoT file ---

    #[test]
    fn test_decode_real_repository_manifest_is_fail_closed_clean() {
        // The committed boundary SSoT (`.harness/config/template-boundary.json`)
        // must decode without error: supported schema version, only known
        // fields, and a non-empty, duplicate-free set of workspace-relative
        // patterns (spec IN-02, IN-12, AC-02, CN-02). This binds the codec to
        // the real manifest so a schema-incompatible edit fails the test suite.
        let manifest_path =
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../.harness/config/template-boundary.json");
        let json = std::fs::read_to_string(manifest_path)
            .unwrap_or_else(|e| panic!("failed to read real manifest {manifest_path}: {e}"));

        let manifest = decode_manifest(&json)
            .unwrap_or_else(|e| panic!("real manifest failed fail-closed decode: {e}"));

        assert!(!manifest.entries().is_empty(), "real manifest must classify at least one path");
        assert!(manifest.patterns_are_unique(), "real manifest patterns must be duplicate-free");
    }
}
