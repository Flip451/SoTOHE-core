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
    /// Whether the DRY gate is enabled (IN-01 / IN-05). Defaults to `false` when
    /// the field is omitted, so existing configs without the key are treated as
    /// opt-out (the safe direction: no accidental gate enforcement).
    #[serde(default)]
    enabled: bool,
    threshold: f32,
    /// D3 (T008): bounded judge fan-out parallelism. Defaults to
    /// [`DEFAULT_MAX_PARALLELISM`] when the field is omitted.
    #[serde(default = "default_max_parallelism")]
    max_parallelism: usize,
    /// D4 (T011): injection rate for known-bad samples in calibration (percent).
    known_bad_injection_rate_percent: u8,
    /// D4 (T011): detection threshold for known-bad samples in calibration (percent).
    known_bad_detection_threshold_percent: u8,
}

/// Default `max_parallelism` when the field is omitted.
///
/// D3 / CN-04: nonzero — chosen as 4 to match the worker-pool sweet spot for
/// the Codex provider's typical per-account concurrency budget.
const DEFAULT_MAX_PARALLELISM: usize = 4;

fn default_max_parallelism() -> usize {
    DEFAULT_MAX_PARALLELISM
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
/// accessors. Following [`AgentProfiles::load`](crate::agent_profiles::AgentProfiles::load)
/// pattern (CN-07 / D9).
#[derive(Debug)]
pub struct DryCheckConfig {
    enabled: bool,
    threshold: domain::semantic_dup::SimilarityThreshold,
    max_parallelism: usize,
    known_bad_injection_rate_percent: u8,
    known_bad_detection_threshold_percent: u8,
}

impl DryCheckConfig {
    /// Loads the DRY-check configuration from a JSON file.
    ///
    /// Reads the file at `path`, validates `schema_version`, parses JSON, and
    /// validates the `threshold`, `max_parallelism`, and percent fields. Fails closed
    /// on any I/O or parse error (CN-04 / D9): no fallback to a default value.
    ///
    /// After D4: `fast_reasoning_effort` / `final_reasoning_effort` are no longer read
    /// from this file — they live in `agent-profiles.json` under the `dry-checker`
    /// capability. The schema rejects any config that still carries those fields
    /// (`deny_unknown_fields` on the DTO enforces the removal).
    ///
    /// # Errors
    ///
    /// - [`DryCheckConfigError::Io`] if the file cannot be read.
    /// - [`DryCheckConfigError::Parse`] if the JSON is invalid or contains unknown fields.
    /// - [`DryCheckConfigError::UnsupportedSchemaVersion`] if `schema_version` is not `4`.
    /// - [`DryCheckConfigError::InvalidThreshold`] if `threshold` is outside `[0.0, 1.0]`.
    /// - [`DryCheckConfigError::InvalidParallelism`] if `max_parallelism` is zero.
    /// - [`DryCheckConfigError::InvalidPercent`] if `known_bad_injection_rate_percent` or
    ///   `known_bad_detection_threshold_percent` is outside `1..=100`.
    pub fn load(path: &Path) -> Result<DryCheckConfig, DryCheckConfigError> {
        const SUPPORTED_SCHEMA_VERSION: u32 = 4;

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

        validate_percent("known_bad_injection_rate_percent", dto.known_bad_injection_rate_percent)?;
        validate_percent(
            "known_bad_detection_threshold_percent",
            dto.known_bad_detection_threshold_percent,
        )?;

        Ok(DryCheckConfig {
            enabled: dto.enabled,
            threshold,
            max_parallelism: dto.max_parallelism,
            known_bad_injection_rate_percent: dto.known_bad_injection_rate_percent,
            known_bad_detection_threshold_percent: dto.known_bad_detection_threshold_percent,
        })
    }

    /// Returns whether the DRY gate is enabled (IN-01 / IN-05 / IN-06).
    ///
    /// Defaults to `false` when the `enabled` key is absent from the config file,
    /// so callers that have not opted in are unaffected by the gate.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the similarity threshold from the loaded configuration.
    pub fn threshold(&self) -> domain::semantic_dup::SimilarityThreshold {
        self.threshold
    }

    /// Returns the configured judge fan-out `max_parallelism` (D3 / IN-03).
    pub fn max_parallelism(&self) -> usize {
        self.max_parallelism
    }

    /// Returns the injection rate for known-bad calibration samples, as a percent (D4 / IN-04).
    pub fn known_bad_injection_rate_percent(&self) -> u8 {
        self.known_bad_injection_rate_percent
    }

    /// Returns the detection threshold for known-bad calibration samples, as a percent (D4 / IN-04).
    pub fn known_bad_detection_threshold_percent(&self) -> u8 {
        self.known_bad_detection_threshold_percent
    }

    /// Compute a SHA-256 fingerprint over all fields that affect `dry write` semantics,
    /// using the provided `effective_threshold` in place of `self.threshold`.
    ///
    /// This is the canonical fingerprint implementation. Call [`fingerprint`](Self::fingerprint)
    /// when the file-config threshold is the effective threshold, or call this method directly
    /// when `--threshold` has been overridden at the CLI so the stored fingerprint reflects
    /// the actual threshold used during the run.
    ///
    /// Canonical encoding: `field=value` pairs joined with `\n`, in the fixed
    /// order below. The threshold is encoded as its raw IEEE 754 bits (`f32::to_bits()`)
    /// to guarantee lossless, round-trip-stable serialization — decimal formatting would
    /// silently collapse distinct values within the same rounding bucket. Negative zero
    /// (`-0.0`) is normalized to positive zero (`+0.0`) before the bit conversion because
    /// `SimilarityThreshold::new` accepts `-0.0` (IEEE 754 equality treats it as `0.0`),
    /// and the two bit patterns must map to the same fingerprint.
    ///
    /// Fields included (all that affect which pairs are judged or how they are judged):
    /// - `enabled`
    /// - `threshold`
    /// - `max_parallelism`
    /// - `known_bad_injection_rate_percent`
    /// - `known_bad_detection_threshold_percent`
    ///
    /// After D4: `fast_reasoning_effort` / `final_reasoning_effort` are excluded from
    /// the canonical encoding (CN-08 / IN-10) because they now live in `agent-profiles.json`
    /// and do not belong to the dry-check config fingerprint.
    pub fn fingerprint_with_threshold(
        &self,
        effective_threshold: domain::semantic_dup::SimilarityThreshold,
    ) -> domain::dry_check::DryCheckConfigFingerprint {
        let canonical = format!(
            "enabled={}\nthreshold={}\nmax_parallelism={}\nknown_bad_injection_rate_percent={}\nknown_bad_detection_threshold_percent={}",
            self.enabled,
            (effective_threshold.value() + 0.0_f32).to_bits(),
            self.max_parallelism,
            self.known_bad_injection_rate_percent,
            self.known_bad_detection_threshold_percent,
        );

        // Compute SHA-256 using the sha2 crate (already a dependency of infrastructure).
        // sha2::Digest::update takes bytes directly (no Write trait needed).
        use sha2::Digest as _;
        let hash_bytes = sha2::Sha256::digest(canonical.as_bytes());
        let hex = hash_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();

        // The hex string is always 64 lowercase chars (SHA-256 = 32 bytes = 64 hex chars).
        // DryCheckConfigFingerprint::new only fails when the string is not 64 lowercase hex
        // chars; sha2 always produces exactly that, so this unwrap cannot be reached at
        // runtime. We use an unreachable fallback to avoid a hard panic.
        domain::dry_check::DryCheckConfigFingerprint::new(hex)
            .unwrap_or_else(|_| domain::dry_check::DryCheckConfigFingerprint::fail_closed())
    }

    /// Compute a SHA-256 fingerprint over all fields that affect `dry write` semantics.
    ///
    /// Delegates to [`fingerprint_with_threshold`](Self::fingerprint_with_threshold) using
    /// `self.threshold` as the effective threshold. Use [`fingerprint_with_threshold`](Self::fingerprint_with_threshold)
    /// directly when a CLI `--threshold` override changes the effective threshold for a run.
    pub fn fingerprint(&self) -> domain::dry_check::DryCheckConfigFingerprint {
        self.fingerprint_with_threshold(self.threshold)
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
        "schema_version": 4,
        "threshold": 0.85,
        "max_parallelism": 4,
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

    /// Builds the JSON body for a minimal v4 config with the given `threshold`.
    ///
    /// Used by both success and error-path tests so the fixture shape is defined once.
    fn config_json_with_threshold(threshold: f32) -> String {
        format!(
            r#"{{
                "schema_version": 4,
                "threshold": {threshold},
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
        let content = r#"{"schema_version": 99, "threshold": 0.85, "max_parallelism": 4, "known_bad_injection_rate_percent": 10, "known_bad_detection_threshold_percent": 90}"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 99, expected: 4 }),
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
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 1, expected: 4 }),
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
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 2, expected: 4 }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn test_load_with_schema_version_three_returns_unsupported() {
        // v3 configs are no longer accepted; schema_version 4 is the only supported version.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 3,
            "threshold": 0.85,
            "max_parallelism": 4,
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 3, expected: 4 }),
            "expected UnsupportedSchemaVersion for schema_version 3, got: {err}"
        );
    }

    #[test]
    fn test_load_with_future_schema_version_returns_unsupported_not_parse() {
        // Even if future schema has new fields, we should get UnsupportedSchemaVersion,
        // not a Parse error from deny_unknown_fields.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 5,
            "threshold": 0.85,
            "new_future_field": "should not cause parse error"
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::UnsupportedSchemaVersion { found: 5, .. }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    // ── enabled field tests (IN-01 / IN-05 / IN-06) ────────────────────────────

    #[test]
    fn test_load_v4_without_enabled_field_defaults_to_false() {
        // `enabled` key absent → serde default → false (opt-out by default).
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "threshold": 0.85,
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!(!config.enabled(), "enabled must default to false when the key is absent");
    }

    #[test]
    fn test_load_v4_with_enabled_false_returns_false() {
        // Explicit `"enabled": false` — same as the shipped `.harness/config/dry-check.json`.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "enabled": false,
            "threshold": 0.85,
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!(!config.enabled(), "enabled must be false when explicitly set to false");
    }

    #[test]
    fn test_load_v4_with_enabled_true_returns_true() {
        // Explicit `"enabled": true` — consumer opt-in.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "enabled": true,
            "threshold": 0.85,
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert!(config.enabled(), "enabled must be true when explicitly set to true");
    }

    // ── D4 (T013/T015): v4 + reasoning_effort residual fields are rejected ────

    #[test]
    fn test_load_v4_with_residual_fast_reasoning_effort_returns_parse_error() {
        // After D4, fast_reasoning_effort is no longer a field in dry-check.json.
        // deny_unknown_fields must reject a v4 config that still carries it.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "threshold": 0.85,
            "fast_reasoning_effort": "medium",
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::Parse(_)),
            "v4 config with residual fast_reasoning_effort must fail with Parse, got: {err}"
        );
        assert!(
            err.to_string().contains("fast_reasoning_effort"),
            "error must mention the unknown field name, got: {err}"
        );
    }

    #[test]
    fn test_load_v4_with_residual_final_reasoning_effort_returns_parse_error() {
        // After D4, final_reasoning_effort is no longer a field in dry-check.json.
        // deny_unknown_fields must reject a v4 config that still carries it.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "threshold": 0.85,
            "final_reasoning_effort": "high",
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let err = DryCheckConfig::load(&path).unwrap_err();
        assert!(
            matches!(err, DryCheckConfigError::Parse(_)),
            "v4 config with residual final_reasoning_effort must fail with Parse, got: {err}"
        );
        assert!(
            err.to_string().contains("final_reasoning_effort"),
            "error must mention the unknown field name, got: {err}"
        );
    }

    // ── D3 (T008) max_parallelism tests ────────────────────────────────────────

    #[test]
    fn test_load_with_valid_max_parallelism_returns_expected_value() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "threshold": 0.85,
            "max_parallelism": 8,
            "known_bad_injection_rate_percent": 10,
            "known_bad_detection_threshold_percent": 90
        }"#;
        let path = write_json(dir.path(), "dry-check.json", content);
        let config = DryCheckConfig::load(&path).unwrap();
        assert_eq!(config.max_parallelism(), 8);
    }

    #[test]
    fn test_load_v4_without_max_parallelism_field_uses_nonzero_default() {
        // Field omitted → serde default; the default must be nonzero (CN-04).
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{
            "schema_version": 4,
            "threshold": 0.85,
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
            "schema_version": 4,
            "threshold": 0.85,
            "max_parallelism": 0,
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

    // ── D4 (T011) percent validation tests ─────────────────────────────────────

    #[test]
    fn test_load_with_invalid_injection_rate_returns_invalid_percent() {
        // Table-driven: covers both boundary violations (0 = below minimum, 101 = above maximum).
        // Note: 101 fits in u8 (u8::MAX = 255), so serde parses it; our validate_percent rejects it.
        for invalid_value in [0u8, 101u8] {
            let content = format!(
                r#"{{
                    "schema_version": 4,
                    "threshold": 0.85,
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
            "schema_version": 4,
            "threshold": 0.85,
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
                    "schema_version": 4,
                    "threshold": 0.85,
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

    // ── fingerprint() tests ─────────────────────────────────────────────────────

    /// Load a config from a JSON string and return the config.
    fn load_from_str(json: &str) -> DryCheckConfig {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), "dry-check.json", json);
        DryCheckConfig::load(&path).unwrap()
    }

    #[test]
    fn test_fingerprint_is_deterministic_for_same_config() {
        // Same config loaded twice must produce the same fingerprint.
        let fp1 = load_from_str(VALID_CONFIG).fingerprint();
        let fp2 = load_from_str(VALID_CONFIG).fingerprint();
        assert_eq!(fp1, fp2, "fingerprint must be deterministic for identical config");
    }

    #[test]
    fn test_fingerprint_changes_when_threshold_changes() {
        let cfg_085 = load_from_str(VALID_CONFIG).fingerprint(); // threshold 0.85
        let cfg_070 = load_from_str(&config_json_with_threshold(0.70)).fingerprint();
        assert_ne!(cfg_085, cfg_070, "fingerprint must differ when threshold changes");
    }

    #[test]
    fn test_fingerprint_changes_when_enabled_changes() {
        let disabled = load_from_str(VALID_CONFIG).fingerprint(); // enabled defaults to false
        let enabled = load_from_str(
            r#"{
                "schema_version": 4,
                "enabled": true,
                "threshold": 0.85,
                "max_parallelism": 4,
                "known_bad_injection_rate_percent": 10,
                "known_bad_detection_threshold_percent": 90
            }"#,
        )
        .fingerprint();
        assert_ne!(disabled, enabled, "fingerprint must differ when enabled changes");
    }

    #[test]
    fn test_fingerprint_changes_when_max_parallelism_changes() {
        let cfg_4 = load_from_str(VALID_CONFIG).fingerprint(); // max_parallelism 4
        let cfg_8 = load_from_str(
            r#"{
                "schema_version": 4,
                "threshold": 0.85,
                "max_parallelism": 8,
                "known_bad_injection_rate_percent": 10,
                "known_bad_detection_threshold_percent": 90
            }"#,
        )
        .fingerprint();
        assert_ne!(cfg_4, cfg_8, "fingerprint must differ when max_parallelism changes");
    }

    #[test]
    fn test_fingerprint_changes_when_known_bad_percents_change() {
        let cfg_10_90 = load_from_str(VALID_CONFIG).fingerprint(); // injection 10, threshold 90
        let cfg_20_80 = load_from_str(
            r#"{
                "schema_version": 4,
                "threshold": 0.85,
                "max_parallelism": 4,
                "known_bad_injection_rate_percent": 20,
                "known_bad_detection_threshold_percent": 80
            }"#,
        )
        .fingerprint();
        assert_ne!(cfg_10_90, cfg_20_80, "fingerprint must differ when known-bad percents change");
    }

    /// D4 / T013 / T015 / CN-08 / IN-10: reasoning effort fields must NOT affect
    /// the fingerprint — they now live in agent-profiles.json, not dry-check.json.
    ///
    /// This test confirms fingerprint stability: two configs that are otherwise
    /// identical produce the same fingerprint regardless of what is configured
    /// in agent-profiles.json for reasoning effort.
    ///
    /// Note: this property cannot be demonstrated by varying the dry-check.json
    /// fields (they were removed), so we confirm it indirectly by showing that
    /// VALID_CONFIG (which has no reasoning_effort fields) still produces a
    /// deterministic fingerprint when loaded twice.
    #[test]
    fn test_fingerprint_is_stable_without_reasoning_effort_fields() {
        // Same config without reasoning_effort fields must produce identical fingerprints.
        let fp1 = load_from_str(VALID_CONFIG).fingerprint();
        let fp2 = load_from_str(VALID_CONFIG).fingerprint();
        assert_eq!(
            fp1, fp2,
            "fingerprint must be stable after reasoning_effort fields are removed (CN-08/IN-10)"
        );
    }

    #[test]
    fn test_fingerprint_returns_64_char_lowercase_hex() {
        let fp = load_from_str(VALID_CONFIG).fingerprint();
        let s = fp.as_str();
        assert_eq!(s.len(), 64, "fingerprint must be exactly 64 chars");
        assert!(
            s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "fingerprint must be lowercase hex"
        );
    }

    #[test]
    fn test_fingerprint_treats_negative_zero_threshold_as_positive_zero() {
        // SimilarityThreshold::new accepts -0.0 (IEEE 754: -0.0 == 0.0).
        // The two bit patterns are distinct (0x80000000 vs 0x00000000), so without
        // normalization a config with threshold=-0.0 would produce a different
        // fingerprint than one with threshold=+0.0 even though they are semantically
        // identical, falsely invalidating an otherwise unchanged coverage manifest.
        //
        // We test by directly building two SimilarityThreshold values with different
        // bit patterns (but equal semantics) and confirming the fingerprints match.
        let pos_zero = domain::semantic_dup::SimilarityThreshold::new(0.0_f32).unwrap();
        let neg_zero = domain::semantic_dup::SimilarityThreshold::new(-0.0_f32).unwrap();
        // Confirm the two values are semantically equal but have different bit patterns.
        assert_eq!(pos_zero.value(), neg_zero.value(), "test setup: +0.0 == -0.0");
        assert_ne!(
            pos_zero.value().to_bits(),
            neg_zero.value().to_bits(),
            "test setup: bit patterns must differ"
        );
        // Load a config and compute fingerprint_with_threshold for both bit-pattern variants.
        let config = load_from_str(VALID_CONFIG);
        let fp_pos = config.fingerprint_with_threshold(pos_zero);
        let fp_neg = config.fingerprint_with_threshold(neg_zero);
        assert_eq!(fp_pos, fp_neg, "fingerprint must be identical for +0.0 and -0.0 thresholds");
    }

    #[test]
    fn test_fingerprint_distinguishes_adjacent_f32_threshold_values() {
        // Regression guard: decimal formatting with {:.6} silently collapses distinct f32 values
        // within the same 1e-6 bucket (e.g. 0.8500001 and 0.8500004 both format as "0.850000").
        // The canonical encoding now uses f32::to_bits() which is lossless.
        let a = 0.8500001_f32;
        let b = 0.8500004_f32;
        // Confirm the two values are distinct f32 bit patterns but collapse under {:.6}.
        assert_ne!(a.to_bits(), b.to_bits(), "test setup: a and b must be distinct f32 values");
        assert_eq!(
            format!("{:.6}", a),
            format!("{:.6}", b),
            "test setup: both must format identically under 6-decimal formatting"
        );
        let fp_a = load_from_str(&config_json_with_threshold(a)).fingerprint();
        let fp_b = load_from_str(&config_json_with_threshold(b)).fingerprint();
        assert_ne!(
            fp_a, fp_b,
            "fingerprint must differ for adjacent f32 threshold values that 6-decimal formatting collapses"
        );
    }
}
