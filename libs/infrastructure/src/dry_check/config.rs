//! Loader for `.harness/config/dry-check.json` — DRY similarity-threshold configuration.
//!
//! Following the [`AgentProfiles::load`](crate::agent_profiles::AgentProfiles::load) pattern:
//! file read → schema-version check → serde parse → typed error enum (CN-07 / D9).

use std::path::Path;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from [`DryCheckConfig::load`]. Mirrors [`AgentProfilesError`](crate::agent_profiles::AgentProfilesError)
/// structure for the dry-check config loader (D9 / CN-07).
#[derive(Debug, thiserror::Error)]
pub enum DryCheckConfigError {
    /// The configuration file could not be read.
    #[error("failed to read dry-check config at {path}: {source}")]
    Io { path: String, source: std::io::Error },

    /// The configuration file contains invalid JSON.
    #[error("failed to parse dry-check config: {0}")]
    Parse(#[from] serde_json::Error),

    /// The configuration file uses an unsupported schema version.
    #[error("unsupported dry-check config schema version {found}; expected {expected}")]
    UnsupportedSchemaVersion { found: u32, expected: u32 },

    /// The `threshold` value in the config is not a valid similarity threshold.
    #[error("invalid threshold in dry-check config: {0}")]
    InvalidThreshold(String),

    /// The `max_parallelism` value is invalid (zero is rejected — D3 / CN-04).
    #[error("invalid max_parallelism in dry-check config: must be nonzero (got {0})")]
    InvalidParallelism(usize),
}

// ---------------------------------------------------------------------------
// Serde DTOs
// ---------------------------------------------------------------------------

/// Minimal envelope to extract `schema_version` before full deserialization.
/// This avoids `deny_unknown_fields` masking future-schema errors as parse errors.
#[derive(Debug, Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: u32,
}

/// Full DTO for `.harness/config/dry-check.json`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DryCheckConfigDto {
    #[allow(dead_code)]
    schema_version: u32,
    threshold: f32,
    /// D3 (T008): bounded judge fan-out parallelism. Defaults to
    /// [`DEFAULT_MAX_PARALLELISM`] when the field is omitted in a v2 config.
    #[serde(default = "default_max_parallelism")]
    max_parallelism: usize,
}

/// Default `max_parallelism` when the field is omitted in a v2 config.
///
/// D3 / CN-04: nonzero — chosen as 4 to match the worker-pool sweet spot for
/// the Codex provider's typical per-account concurrency budget.
const DEFAULT_MAX_PARALLELISM: usize = 4;

fn default_max_parallelism() -> usize {
    DEFAULT_MAX_PARALLELISM
}

// ---------------------------------------------------------------------------
// DryCheckConfig (public API)
// ---------------------------------------------------------------------------

/// Loaded `.harness/config/dry-check.json` configuration.
///
/// Private serde DTO internals; public API is [`load()`](DryCheckConfig::load) +
/// [`threshold()`](DryCheckConfig::threshold). Following
/// [`AgentProfiles::load`](crate::agent_profiles::AgentProfiles::load) pattern
/// (CN-07 / D9).
#[derive(Debug)]
pub struct DryCheckConfig {
    threshold: domain::semantic_dup::SimilarityThreshold,
    max_parallelism: usize,
}

impl DryCheckConfig {
    /// Loads the DRY-check configuration from a JSON file.
    ///
    /// Reads the file at `path`, validates `schema_version`, parses JSON, and
    /// validates the `threshold` value. Fails closed on any I/O or parse error
    /// (CN-04 / D9): no fallback to a default value.
    ///
    /// # Errors
    ///
    /// - [`DryCheckConfigError::Io`] if the file cannot be read.
    /// - [`DryCheckConfigError::Parse`] if the JSON is invalid.
    /// - [`DryCheckConfigError::UnsupportedSchemaVersion`] if `schema_version` is not `2`.
    /// - [`DryCheckConfigError::InvalidThreshold`] if `threshold` is outside `[0.0, 1.0]`.
    /// - [`DryCheckConfigError::InvalidParallelism`] if `max_parallelism` is zero.
    pub fn load(path: &Path) -> Result<DryCheckConfig, DryCheckConfigError> {
        const SUPPORTED_SCHEMA_VERSION: u32 = 2;

        let content = std::fs::read_to_string(path)
            .map_err(|e| DryCheckConfigError::Io { path: path.display().to_string(), source: e })?;

        // Parse schema_version first (without deny_unknown_fields) so future
        // schema versions produce UnsupportedSchemaVersion, not a Parse error.
        let envelope: SchemaVersionEnvelope = serde_json::from_str(&content)?;
        if envelope.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(DryCheckConfigError::UnsupportedSchemaVersion {
                found: envelope.schema_version,
                expected: SUPPORTED_SCHEMA_VERSION,
            });
        }

        let dto: DryCheckConfigDto = serde_json::from_str(&content)?;

        let threshold = domain::semantic_dup::SimilarityThreshold::new(dto.threshold)
            .map_err(|e| DryCheckConfigError::InvalidThreshold(e.to_string()))?;

        if dto.max_parallelism == 0 {
            return Err(DryCheckConfigError::InvalidParallelism(dto.max_parallelism));
        }

        Ok(DryCheckConfig { threshold, max_parallelism: dto.max_parallelism })
    }

    /// Returns the similarity threshold from the loaded configuration.
    pub fn threshold(&self) -> domain::semantic_dup::SimilarityThreshold {
        self.threshold
    }

    /// Returns the configured judge fan-out `max_parallelism` (D3 / IN-03).
    pub fn max_parallelism(&self) -> usize {
        self.max_parallelism
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_json(dir: &std::path::Path, filename: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(filename);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    const VALID_CONFIG: &str = r#"{
        "schema_version": 2,
        "threshold": 0.85,
        "max_parallelism": 4
    }"#;

    #[test]
    fn test_load_with_valid_config_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), "dry-check.json", VALID_CONFIG);
        let config = DryCheckConfig::load(&path).unwrap();
        let t = config.threshold();
        assert!((t.value() - 0.85_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_threshold_returns_expected_value() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 0.70}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!((config.threshold().value() - 0.70_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_load_with_missing_file_returns_io_error() {
        let path = std::path::Path::new("/nonexistent/dry-check.json");
        let err = DryCheckConfig::load(path).unwrap_err();
        assert!(matches!(err, DryCheckConfigError::Io { .. }), "expected Io, got: {err}");
    }

    #[test]
    fn test_load_with_invalid_json_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), "dry-check.json", "not valid json {{");
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(matches!(err, DryCheckConfigError::Parse(_)), "expected Parse, got: {err}");
    }

    #[test]
    fn test_load_with_unsupported_schema_version_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 99, "threshold": 0.85, "max_parallelism": 4}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 99, expected: 2 }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn test_load_with_schema_version_one_returns_unsupported_not_parse() {
        // Old v1 configs (no max_parallelism) now report UnsupportedSchemaVersion
        // rather than parsing. (Use the envelope-only path.)
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 1, "threshold": 0.85}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 1, expected: 2 }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn test_load_with_future_schema_version_returns_unsupported_not_parse() {
        // Even if future schema has new fields, we should get UnsupportedSchemaVersion,
        // not a Parse error from deny_unknown_fields.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 3,
            "threshold": 0.85,
            "new_future_field": "should not cause parse error"
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 3, .. }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    // ── D3 (T008) max_parallelism tests ────────────────────────────────────────

    #[test]
    fn test_load_with_valid_max_parallelism_returns_expected_value() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 0.85, "max_parallelism": 8}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert_eq!(config.max_parallelism(), 8);
    }

    #[test]
    fn test_load_v2_without_max_parallelism_field_uses_nonzero_default() {
        // Field omitted → serde default; the default must be nonzero (CN-04).
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 0.85}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!(config.max_parallelism() > 0);
    }

    #[test]
    fn test_load_with_zero_max_parallelism_returns_invalid_parallelism() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 0.85, "max_parallelism": 0}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::InvalidParallelism(0)),
            "expected InvalidParallelism(0), got: {err}"
        );
    }

    #[test]
    fn test_load_with_threshold_above_one_returns_invalid_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 1.5}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::InvalidThreshold(_)),
            "expected InvalidThreshold, got: {err}"
        );
    }

    #[test]
    fn test_load_with_threshold_below_zero_returns_invalid_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": -0.1}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::InvalidThreshold(_)),
            "expected InvalidThreshold, got: {err}"
        );
    }

    #[test]
    fn test_load_with_threshold_zero_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 0.0}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!((config.threshold().value() - 0.0_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_load_with_threshold_one_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 1.0}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!((config.threshold().value() - 1.0_f32).abs() < f32::EPSILON);
    }
}
