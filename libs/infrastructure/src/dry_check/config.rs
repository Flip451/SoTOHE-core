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

    /// A reasoning effort value is not one of the allowed values (D4 / IN-04).
    ///
    /// Allowed values: `"low"`, `"medium"`, `"high"`, `"minimal"`.
    #[error(
        "invalid reasoning_effort in dry-check config for field '{field}': '{value}' (allowed: low, medium, high, minimal)"
    )]
    InvalidReasoningEffort {
        /// The config field name that contained the invalid value (e.g., `"fast_reasoning_effort"`).
        field: String,
        /// The invalid value that was provided.
        value: String,
    },

    /// A percent value is outside the valid range `1..=100` (D4 / IN-04).
    #[error("invalid percent in dry-check config for field '{field}': {value} (must be 1..=100)")]
    InvalidPercent {
        /// The config field name that contained the invalid value (e.g., `"known_bad_injection_rate_percent"`).
        field: String,
        /// The invalid value that was provided.
        value: u8,
    },
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
    /// [`DEFAULT_MAX_PARALLELISM`] when the field is omitted in a v3 config.
    #[serde(default = "default_max_parallelism")]
    max_parallelism: usize,
    /// D4 (T011): reasoning effort for the fast-round DRY checker.
    /// Allowed values: "low", "medium", "high", "minimal".
    fast_reasoning_effort: String,
    /// D4 (T011): reasoning effort for the final-round DRY checker.
    /// Allowed values: "low", "medium", "high", "minimal".
    final_reasoning_effort: String,
    /// D4 (T011): injection rate for known-bad samples in calibration (percent).
    known_bad_injection_rate_percent: u8,
    /// D4 (T011): detection threshold for known-bad samples in calibration (percent).
    known_bad_detection_threshold_percent: u8,
}

/// Default `max_parallelism` when the field is omitted in a v3 config.
///
/// D3 / CN-04: nonzero — chosen as 4 to match the worker-pool sweet spot for
/// the Codex provider's typical per-account concurrency budget.
const DEFAULT_MAX_PARALLELISM: usize = 4;

fn default_max_parallelism() -> usize {
    DEFAULT_MAX_PARALLELISM
}

/// Allowed reasoning effort values for Codex CLI `model_reasoning_effort` (D4 / IN-04).
const ALLOWED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "minimal"];

/// Validates a reasoning effort string, returning `Err` with the field name and invalid value.
fn validate_reasoning_effort(field: &str, value: &str) -> Result<(), DryCheckConfigError> {
    if ALLOWED_REASONING_EFFORTS.contains(&value) {
        Ok(())
    } else {
        Err(DryCheckConfigError::InvalidReasoningEffort {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

/// Validates a percent value is in `1..=100`, returning `Err` with the field name and value.
fn validate_percent(field: &str, value: u8) -> Result<(), DryCheckConfigError> {
    if (1..=100).contains(&value) {
        Ok(())
    } else {
        Err(DryCheckConfigError::InvalidPercent { field: field.to_owned(), value })
    }
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
    fast_reasoning_effort: String,
    final_reasoning_effort: String,
    known_bad_injection_rate_percent: u8,
    known_bad_detection_threshold_percent: u8,
}

impl DryCheckConfig {
    /// Loads the DRY-check configuration from a JSON file.
    ///
    /// Reads the file at `path`, validates `schema_version`, parses JSON, and
    /// validates the `threshold`, `fast_reasoning_effort`, `final_reasoning_effort`,
    /// and percent fields. Fails closed on any I/O or parse error (CN-04 / D9): no
    /// fallback to a default value.
    ///
    /// # Errors
    ///
    /// - [`DryCheckConfigError::Io`] if the file cannot be read.
    /// - [`DryCheckConfigError::Parse`] if the JSON is invalid.
    /// - [`DryCheckConfigError::UnsupportedSchemaVersion`] if `schema_version` is not `3`.
    /// - [`DryCheckConfigError::InvalidThreshold`] if `threshold` is outside `[0.0, 1.0]`.
    /// - [`DryCheckConfigError::InvalidParallelism`] if `max_parallelism` is zero.
    /// - [`DryCheckConfigError::InvalidReasoningEffort`] if `fast_reasoning_effort` or
    ///   `final_reasoning_effort` is not one of `"low"`, `"medium"`, `"high"`, `"minimal"`.
    /// - [`DryCheckConfigError::InvalidPercent`] if `known_bad_injection_rate_percent` or
    ///   `known_bad_detection_threshold_percent` is outside `1..=100`.
    pub fn load(path: &Path) -> Result<DryCheckConfig, DryCheckConfigError> {
        const SUPPORTED_SCHEMA_VERSION: u32 = 3;

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

        validate_reasoning_effort("fast_reasoning_effort", &dto.fast_reasoning_effort)?;
        validate_reasoning_effort("final_reasoning_effort", &dto.final_reasoning_effort)?;

        validate_percent("known_bad_injection_rate_percent", dto.known_bad_injection_rate_percent)?;
        validate_percent(
            "known_bad_detection_threshold_percent",
            dto.known_bad_detection_threshold_percent,
        )?;

        Ok(DryCheckConfig {
            threshold,
            max_parallelism: dto.max_parallelism,
            fast_reasoning_effort: dto.fast_reasoning_effort,
            final_reasoning_effort: dto.final_reasoning_effort,
            known_bad_injection_rate_percent: dto.known_bad_injection_rate_percent,
            known_bad_detection_threshold_percent: dto.known_bad_detection_threshold_percent,
        })
    }

    /// Returns the similarity threshold from the loaded configuration.
    pub fn threshold(&self) -> domain::semantic_dup::SimilarityThreshold {
        self.threshold
    }

    /// Returns the configured judge fan-out `max_parallelism` (D3 / IN-03).
    pub fn max_parallelism(&self) -> usize {
        self.max_parallelism
    }

    /// Returns the reasoning effort for the fast-round DRY checker (D4 / IN-04).
    pub fn fast_reasoning_effort(&self) -> &str {
        &self.fast_reasoning_effort
    }

    /// Returns the reasoning effort for the final-round DRY checker (D4 / IN-04).
    pub fn final_reasoning_effort(&self) -> &str {
        &self.final_reasoning_effort
    }

    /// Returns the injection rate for known-bad calibration samples, as a percent (D4 / IN-04).
    pub fn known_bad_injection_rate_percent(&self) -> u8 {
        self.known_bad_injection_rate_percent
    }

    /// Returns the detection threshold for known-bad calibration samples, as a percent (D4 / IN-04).
    pub fn known_bad_detection_threshold_percent(&self) -> u8 {
        self.known_bad_detection_threshold_percent
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
        "schema_version": 3,
        "threshold": 0.85,
        "max_parallelism": 4,
        "fast_reasoning_effort": "medium",
        "final_reasoning_effort": "high",
        "known_bad_injection_rate_percent": 10,
        "known_bad_detection_threshold_percent": 90
    }"#;

    #[test]
    fn test_load_with_valid_config_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), "dry-check.json", VALID_CONFIG);
        let config = DryCheckConfig::load(&path).unwrap();
        let t = config.threshold();
        assert!((t.value() - 0.85_f32).abs() < f32::EPSILON);
    }

    /// Builds the JSON body for a minimal v3 config with the given reasoning efforts.
    ///
    /// Used by both valid-effort and invalid-effort tests so the fixture shape is defined once.
    fn config_json_with_reasoning_efforts(fast_effort: &str, final_effort: &str) -> String {
        format!(
            r#"{{
                "schema_version": 3,
                "threshold": 0.85,
                "fast_reasoning_effort": "{fast_effort}",
                "final_reasoning_effort": "{final_effort}",
                "known_bad_injection_rate_percent": 10,
                "known_bad_detection_threshold_percent": 90
            }}"#
        )
    }

    /// Builds the JSON body for a minimal v3 config with the given `threshold`.
    ///
    /// Used by both success and error-path tests so the fixture shape is defined once.
    fn config_json_with_threshold(threshold: f32) -> String {
        format!(
            r#"{{
                "schema_version": 3,
                "threshold": {threshold},
                "fast_reasoning_effort": "medium",
                "final_reasoning_effort": "high",
                "known_bad_injection_rate_percent": 10,
                "known_bad_detection_threshold_percent": 90
            }}"#
        )
    }

    /// Writes a minimal v3 config with the given `threshold` to a temp dir and loads it.
    fn load_config_with_threshold(threshold: f32) -> DryCheckConfig {
        let dir = tempfile::tempdir().unwrap();
        let content = config_json_with_threshold(threshold);
        let path = write_json(dir.path(), "dry-check.json", &content);
        DryCheckConfig::load(&path).unwrap()
    }

    #[test]
    fn test_threshold_returns_expected_value() {
        // Table-driven: boundary and representative values all round-trip correctly.
        for &expected in &[0.0_f32, 0.70_f32, 1.0_f32] {
            let config = load_config_with_threshold(expected);
            assert!(
                (config.threshold().value() - expected).abs() < f32::EPSILON,
                "threshold {expected} did not round-trip: got {}",
                config.threshold().value()
            );
        }
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
        let content = r#"{"schema_version": 99, "threshold": 0.85, "max_parallelism": 4, "fast_reasoning_effort": "medium", "final_reasoning_effort": "high", "known_bad_injection_rate_percent": 10, "known_bad_detection_threshold_percent": 90}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 99, expected: 3 }),
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
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 1, expected: 3 }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn test_load_with_schema_version_two_returns_unsupported() {
        // v2 configs (no reasoning_effort fields) must report UnsupportedSchemaVersion.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"schema_version": 2, "threshold": 0.85, "max_parallelism": 4}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 2, expected: 3 }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn test_load_with_future_schema_version_returns_unsupported_not_parse() {
        // Even if future schema has new fields, we should get UnsupportedSchemaVersion,
        // not a Parse error from deny_unknown_fields.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "threshold": 0.85,
            "new_future_field": "should not cause parse error"
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 4, .. }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    // ── D3 (T008) max_parallelism tests ────────────────────────────────────────

    #[test]
    fn test_load_with_valid_max_parallelism_returns_expected_value() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 3,
            "threshold": 0.85,
            "max_parallelism": 8,
            "fast_reasoning_effort": "medium",
            "final_reasoning_effort": "high",
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert_eq!(config.max_parallelism(), 8);
    }

    #[test]
    fn test_load_v3_without_max_parallelism_field_uses_nonzero_default() {
        // Field omitted → serde default; the default must be nonzero (CN-04).
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 3,
            "threshold": 0.85,
            "fast_reasoning_effort": "medium",
            "final_reasoning_effort": "high",
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!(config.max_parallelism() > 0);
    }

    #[test]
    fn test_load_with_zero_max_parallelism_returns_invalid_parallelism() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 3,
            "threshold": 0.85,
            "max_parallelism": 0,
            "fast_reasoning_effort": "medium",
            "final_reasoning_effort": "high",
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
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
        let content = config_json_with_threshold(1.5);
        let path = write_json(dir.path(), "dry-check.json", &content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::InvalidThreshold(_)),
            "expected InvalidThreshold, got: {err}"
        );
    }

    #[test]
    fn test_load_with_threshold_below_zero_returns_invalid_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let content = config_json_with_threshold(-0.1);
        let path = write_json(dir.path(), "dry-check.json", &content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::InvalidThreshold(_)),
            "expected InvalidThreshold, got: {err}"
        );
    }

    // ── D4 (T011) reasoning_effort tests ───────────────────────────────────────

    #[test]
    fn test_load_v3_with_valid_reasoning_efforts_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), "dry-check.json", VALID_CONFIG);
        let config = DryCheckConfig::load(&path).unwrap();
        assert_eq!(config.fast_reasoning_effort(), "medium");
        assert_eq!(config.final_reasoning_effort(), "high");
        assert_eq!(config.known_bad_injection_rate_percent(), 10);
        assert_eq!(config.known_bad_detection_threshold_percent(), 90);
    }

    #[test]
    fn test_load_with_all_valid_reasoning_effort_values_succeed() {
        for effort in ["low", "medium", "high", "minimal"] {
            let dir = tempfile::tempdir().unwrap();
            let content = config_json_with_reasoning_efforts(effort, effort);
            let path = write_json(dir.path(), "dry-check.json", &content);
            let result = DryCheckConfig::load(&path);
            assert!(result.is_ok(), "expected Ok for effort={effort}, got: {result:?}");
        }
    }

    #[test]
    fn test_load_with_invalid_reasoning_effort_returns_error() {
        // Table-driven: (fast_effort, final_effort, invalid_field, invalid_value)
        let cases = [
            ("turbo", "high", "fast_reasoning_effort", "turbo"),
            ("medium", "ultra", "final_reasoning_effort", "ultra"),
        ];
        for (fast, final_, expected_field, expected_value) in cases {
            let content = config_json_with_reasoning_efforts(fast, final_);
            let dir = tempfile::tempdir().unwrap();
            let path = write_json(dir.path(), "dry-check.json", &content);
            let err = DryCheckConfig::load(&path).unwrap_err();
            assert!(
                matches!(
                    &err,
                    DryCheckConfigError::InvalidReasoningEffort { field, value }
                        if field == expected_field && value == expected_value
                ),
                "expected InvalidReasoningEffort for {expected_field}={expected_value}, got: {err}"
            );
        }
    }

    // ── D4 (T011) percent validation tests ─────────────────────────────────────

    #[test]
    fn test_load_with_invalid_injection_rate_returns_invalid_percent() {
        // Table-driven: covers both boundary violations (0 = below minimum, 101 = above maximum).
        // Note: 101 fits in u8 (u8::MAX = 255), so serde parses it; our validate_percent rejects it.
        for invalid_value in [0u8, 101u8] {
            let content = format!(
                r#"{{
                    "schema_version": 3,
                    "threshold": 0.85,
                    "fast_reasoning_effort": "medium",
                    "final_reasoning_effort": "high",
                    "known_bad_injection_rate_percent": {invalid_value},
                    "known_bad_detection_threshold_percent": 90
                }}"#
            );
            let dir = tempfile::tempdir().unwrap();
            let path = write_json(dir.path(), "dry-check.json", &content);
            let err = DryCheckConfig::load(&path).unwrap_err();
            assert!(
                matches!(
                    &err,
                    DryCheckConfigError::InvalidPercent { field, .. }
                        if field == "known_bad_injection_rate_percent"
                ),
                "expected InvalidPercent for known_bad_injection_rate_percent={invalid_value}, got: {err}"
            );
        }
    }

    #[test]
    fn test_load_with_zero_detection_threshold_returns_invalid_percent() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 3,
            "threshold": 0.85,
            "fast_reasoning_effort": "medium",
            "final_reasoning_effort": "high",
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 0
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(
                &err,
                DryCheckConfigError::InvalidPercent { field, value }
                    if field == "known_bad_detection_threshold_percent" && *value == 0
            ),
            "expected InvalidPercent for known_bad_detection_threshold_percent=0, got: {err}"
        );
    }

    #[test]
    fn test_load_with_percent_boundary_values_succeed() {
        // 1 and 100 are both valid edge cases for the 1..=100 range.
        for (injection, threshold) in [(1u8, 1u8), (100u8, 100u8), (1u8, 100u8)] {
            let dir = tempfile::tempdir().unwrap();
            let content = format!(
                r#"{{
                    "schema_version": 3,
                    "threshold": 0.85,
                    "fast_reasoning_effort": "medium",
                    "final_reasoning_effort": "high",
                    "known_bad_injection_rate_percent": {injection},
                    "known_bad_detection_threshold_percent": {threshold}
                }}"#
            );
            let path = write_json(dir.path(), "dry-check.json", &content);
            let result = DryCheckConfig::load(&path);
            assert!(
                result.is_ok(),
                "expected Ok for injection={injection} threshold={threshold}, got: {result:?}"
            );
        }
    }
}
