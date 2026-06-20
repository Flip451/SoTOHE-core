//! Load and validate `.harness/config/signal-gates.json` into [`domain::SignalGateMatrix`].
//!
//! # Validation pipeline
//!
//! 1. Read the file — [`SignalGatesConfigError::FileNotFound`] on I/O failure.
//! 2. Parse JSON via `serde_json` — [`SignalGatesConfigError::ParseFailed`] on malformed JSON.
//! 3. Validate `$schema_version == 1` — [`SignalGatesConfigError::SchemaVersionUnknown`] on mismatch.
//! 4. Check all 8 required chain×gate cells are present — [`SignalGatesConfigError::MissingKey`].
//! 5. Convert each `StrictnessDto` to [`domain::Strictness`] — [`SignalGatesConfigError::InvalidValue`]
//!    on unknown string.
//!
//! No implicit fallback is applied at any step. Every error variant produces a hard failure.
//!
//! # Key note
//!
//! The JSON key `$schema_version` (dollar-prefixed) is mapped to the `schema_version` field via
//! `#[serde(rename = "$schema_version")]`. Standard serde struct deserialization handles this.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use domain::{ChainGateEntry, SignalGateMatrix, Strictness};

use crate::git_cli::show::{BlobResult, fetch_blob_safe};

// ── DTO types (serde boundary — never exposed to domain) ─────────────────────

/// Deserializable mirror of [`domain::Strictness`] for the serde boundary.
///
/// Variants map one-to-one to [`domain::Strictness`]. Unknown strings in the JSON are
/// detected after deserialization by the loader and reported as
/// [`SignalGatesConfigError::InvalidValue`]. The domain type carries no serde derive;
/// this DTO is the only serde-enabled representation in the system.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrictnessDto {
    /// Yellow signals produce a blocking error in addition to Red.
    Strict,
    /// Yellow signals produce a warning only; only Red signals block.
    Interim,
}

impl From<StrictnessDto> for Strictness {
    fn from(dto: StrictnessDto) -> Self {
        match dto {
            StrictnessDto::Strict => Strictness::Strict,
            StrictnessDto::Interim => Strictness::Interim,
        }
    }
}

/// Serde DTO for one gate row (`commit_gate` or `merge_gate`) in `signal-gates.json`.
///
/// Each field corresponds to one chain's strictness cell. The four cells cover all four
/// SoT Chain signal chains. Converts to a pair of [`domain::ChainGateEntry`] values
/// (one per chain) after schema validation.
#[derive(Debug, Clone, Deserialize)]
pub struct GateRowDto {
    /// Strictness for chain ⓪ (`adr-user`).
    pub adr_user: StrictnessDto,
    /// Strictness for chain ① (`spec-adr`).
    pub spec_adr: StrictnessDto,
    /// Strictness for chain ② (`catalog-spec`).
    pub catalog_spec: StrictnessDto,
    /// Strictness for chain ③ (`impl-catalog`).
    pub impl_catalog: StrictnessDto,
}

/// Serde DTO for the full `.harness/config/signal-gates.json` document.
///
/// Deserializes the on-disk config and validates `$schema_version == 1`. Converts to
/// the domain [`SignalGateMatrix`] after validation. All fields are required; missing
/// keys are reported as [`SignalGatesConfigError::MissingKey`] per CN-01/D4.
///
/// The `$schema_version` dollar-prefixed key is mapped via `#[serde(rename)]`.
#[derive(Debug, Clone, Deserialize)]
pub struct SignalGatesConfig {
    /// Config schema version — must be `1`.
    #[serde(rename = "$schema_version")]
    pub schema_version: u32,
    /// Strictness settings for the CI commit gate.
    pub commit_gate: GateRowDto,
    /// Strictness settings for the PR merge gate.
    pub merge_gate: GateRowDto,
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Hard error for `signal-gates.json` load/validate failures.
///
/// All variants produce a hard failure and halt gate execution per D4.
/// No silent fallback, no implicit default.
#[derive(Debug, Error)]
pub enum SignalGatesConfigError {
    /// The config file does not exist at the given path.
    #[error(
        "signal-gates.json not found at {path}: \
         place the recommended default config at that path and retry"
    )]
    FileNotFound {
        /// Path at which the file was expected.
        path: PathBuf,
    },

    /// The file could not be parsed as valid JSON, or the top-level structure is wrong.
    #[error(
        "signal-gates.json at {path} could not be parsed: {reason} — \
         repair the JSON and retry"
    )]
    ParseFailed {
        /// Path of the malformed file.
        path: PathBuf,
        /// Human-readable description of the parse failure.
        reason: String,
    },

    /// `$schema_version` is present but holds an unrecognised value.
    #[error(
        "signal-gates.json: unsupported $schema_version {actual} (expected {expected}) — \
         update the config to schema version {expected}"
    )]
    SchemaVersionUnknown {
        /// The only currently supported schema version.
        expected: u32,
        /// The version found in the file.
        actual: u32,
    },

    /// A required key (gate object or chain×gate cell) is absent.
    #[error(
        "signal-gates.json: required key \"{key}\" is missing — \
         add the key with a valid strictness value (\"strict\" or \"interim\") and retry"
    )]
    MissingKey {
        /// Dot-separated JSON path of the missing key (e.g. `"commit_gate.impl_catalog"`).
        key: String,
    },

    /// A chain×gate cell holds an unrecognised strictness string.
    #[error(
        "signal-gates.json: invalid strictness value \"{value}\" for key \"{key}\" — \
         valid values are \"strict\" and \"interim\""
    )]
    InvalidValue {
        /// Dot-separated JSON path of the offending key.
        key: String,
        /// The unrecognised value found at that key.
        value: String,
    },

    /// The blob could not be fetched from the given git ref (not-found or command failure).
    #[error(
        "signal-gates.json: could not fetch blob from branch '{branch}': {reason} — \
         ensure the branch is fetched and `.harness/config/signal-gates.json` is committed"
    )]
    BlobFetchError {
        /// Branch ref from which the blob was requested.
        branch: String,
        /// Human-readable description of the fetch failure.
        reason: String,
    },
}

// ── Loader ────────────────────────────────────────────────────────────────────

/// The only currently supported `$schema_version`.
const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Load and validate `.harness/config/signal-gates.json` from `config_path`.
///
/// Performs a strict, fail-closed validation pipeline:
///
/// 1. Read the file — [`SignalGatesConfigError::FileNotFound`] on missing file.
/// 2. Parse as JSON — [`SignalGatesConfigError::ParseFailed`] on malformed JSON.
/// 3. Validate `$schema_version == 1` — [`SignalGatesConfigError::SchemaVersionUnknown`].
/// 4. Check all 8 required chain×gate cells — [`SignalGatesConfigError::MissingKey`].
/// 5. Convert strictness strings — [`SignalGatesConfigError::InvalidValue`] on unknown string.
///
/// On success returns a fully-populated [`SignalGateMatrix`]. No implicit fallback is applied.
///
/// # Errors
///
/// Returns [`SignalGatesConfigError`] on any failure at the above steps.
pub fn load_signal_gates_config(
    config_path: PathBuf,
) -> Result<SignalGateMatrix, SignalGatesConfigError> {
    // Step 1: read the raw JSON text.
    let raw = std::fs::read_to_string(&config_path)
        .map_err(|_| SignalGatesConfigError::FileNotFound { path: config_path.clone() })?;

    // Steps 2-5: delegate to the shared parse/validate pipeline.
    parse_signal_gates_config_str(&raw, &config_path)
}

/// Load and validate `.harness/config/signal-gates.json` from a git branch via
/// `git show origin/<branch>:.harness/config/signal-gates.json`.
///
/// Applies the same strict validation pipeline as [`load_signal_gates_config`] but
/// reads the config from the committed blob on the given branch rather than from the
/// local worktree. This ensures that `pr_wait_and_merge` evaluates the gate matrix
/// that is committed on the PR branch, preventing a locally relaxed config from
/// silently bypassing the merge gate.
///
/// Uses `fetch_blob_safe` which rejects symlinks and submodules (fail-closed, §D4.3).
///
/// # Errors
///
/// Returns [`SignalGatesConfigError::BlobFetchError`] when the blob cannot be fetched
/// (not found on the branch, or git command failure). Remaining variants follow the
/// same pipeline as [`load_signal_gates_config`].
pub fn load_signal_gates_config_from_branch(
    repo_root: &Path,
    branch: &str,
) -> Result<SignalGateMatrix, SignalGatesConfigError> {
    let blob_path = ".harness/config/signal-gates.json";
    let raw = match fetch_blob_safe(repo_root, branch, blob_path) {
        BlobResult::Found(bytes) => {
            String::from_utf8(bytes).map_err(|e| SignalGatesConfigError::BlobFetchError {
                branch: branch.to_owned(),
                reason: format!("non-UTF-8 bytes in blob: {e}"),
            })?
        }
        BlobResult::NotFound => {
            return Err(SignalGatesConfigError::BlobFetchError {
                branch: branch.to_owned(),
                reason: format!(
                    "path '{blob_path}' not found on branch — commit \
                     `.harness/config/signal-gates.json` before merging"
                ),
            });
        }
        BlobResult::CommandFailed(msg) => {
            return Err(SignalGatesConfigError::BlobFetchError {
                branch: branch.to_owned(),
                reason: msg,
            });
        }
    };

    // Re-use the same parse/validate pipeline as the file-based loader.
    // Use a descriptive display path that names the branch for diagnostics.
    let display_path = PathBuf::from(format!("origin/{branch}:.harness/config/signal-gates.json"));
    parse_signal_gates_config_str(&raw, &display_path)
}

/// Parse and validate a signal-gates JSON string (shared logic for both loaders).
///
/// `display_path` is embedded in any [`SignalGatesConfigError::ParseFailed`] messages
/// so diagnostics name the source (file path or a branch-blob descriptor).
fn parse_signal_gates_config_str(
    raw: &str,
    display_path: &Path,
) -> Result<SignalGateMatrix, SignalGatesConfigError> {
    // Step 2: parse into a serde_json::Value for fine-grained key-presence checks.
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| SignalGatesConfigError::ParseFailed {
            path: display_path.to_path_buf(),
            reason: e.to_string(),
        })?;
    validate_top_level_object(&value, display_path)?;

    // Step 3: validate $schema_version.
    let schema_version = extract_schema_version(&value, display_path)?;
    if schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(SignalGatesConfigError::SchemaVersionUnknown {
            expected: SUPPORTED_SCHEMA_VERSION,
            actual: schema_version,
        });
    }

    // Steps 4 & 5: validate all required keys and strictness values, then convert.
    let commit_gate = extract_gate_row(&value, "commit_gate", display_path)?;
    let merge_gate = extract_gate_row(&value, "merge_gate", display_path)?;

    Ok(SignalGateMatrix {
        adr_user: ChainGateEntry {
            commit_gate: commit_gate.adr_user.into(),
            merge_gate: merge_gate.adr_user.into(),
        },
        spec_adr: ChainGateEntry {
            commit_gate: commit_gate.spec_adr.into(),
            merge_gate: merge_gate.spec_adr.into(),
        },
        catalog_spec: ChainGateEntry {
            commit_gate: commit_gate.catalog_spec.into(),
            merge_gate: merge_gate.catalog_spec.into(),
        },
        impl_catalog: ChainGateEntry {
            commit_gate: commit_gate.impl_catalog.into(),
            merge_gate: merge_gate.impl_catalog.into(),
        },
    })
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Validate that the parsed document is a top-level JSON object.
fn validate_top_level_object(
    value: &serde_json::Value,
    config_path: &Path,
) -> Result<(), SignalGatesConfigError> {
    if value.is_object() {
        return Ok(());
    }

    Err(SignalGatesConfigError::ParseFailed {
        path: config_path.to_path_buf(),
        reason: format!("top-level JSON document must be an object, got {}", json_type_name(value)),
    })
}

/// Extract and validate `$schema_version` from the top-level JSON object.
///
/// Returns [`SignalGatesConfigError::MissingKey`] when the key is absent and
/// [`SignalGatesConfigError::ParseFailed`] when the value is not a valid integer.
fn extract_schema_version(
    value: &serde_json::Value,
    config_path: &Path,
) -> Result<u32, SignalGatesConfigError> {
    let version_value = value
        .get("$schema_version")
        .ok_or_else(|| SignalGatesConfigError::MissingKey { key: "$schema_version".to_owned() })?;

    version_value.as_u64().and_then(|v| u32::try_from(v).ok()).ok_or_else(|| {
        SignalGatesConfigError::ParseFailed {
            path: config_path.to_path_buf(),
            reason: format!("$schema_version must be a non-negative integer, got: {version_value}"),
        }
    })
}

/// Extract one gate row from the JSON value at `gate_key` (e.g. `"commit_gate"`).
///
/// Returns [`SignalGatesConfigError::MissingKey`] when the gate object itself or any of
/// its four chain cells is absent, and [`SignalGatesConfigError::InvalidValue`] when a
/// cell holds an unrecognised strictness string.
fn extract_gate_row(
    value: &serde_json::Value,
    gate_key: &str,
    config_path: &Path,
) -> Result<GateRowDto, SignalGatesConfigError> {
    let gate_obj = value
        .get(gate_key)
        .ok_or_else(|| SignalGatesConfigError::MissingKey { key: gate_key.to_owned() })?;

    validate_gate_shape_and_values(gate_obj, gate_key, config_path)?;

    serde_json::from_value::<GateRowDto>(gate_obj.clone()).map_err(|e| {
        SignalGatesConfigError::ParseFailed {
            path: config_path.to_path_buf(),
            reason: format!("{gate_key} could not be converted to the typed DTO: {e}"),
        }
    })
}

/// Validate one gate row before typed DTO conversion.
fn validate_gate_shape_and_values(
    gate_obj: &serde_json::Value,
    gate_key: &str,
    config_path: &Path,
) -> Result<(), SignalGatesConfigError> {
    const CHAIN_CELLS: &[&str] = &["adr_user", "spec_adr", "catalog_spec", "impl_catalog"];
    const VALID: &[&str] = &["strict", "interim"];

    let gate_map = gate_obj.as_object().ok_or_else(|| SignalGatesConfigError::ParseFailed {
        path: config_path.to_path_buf(),
        reason: format!("{gate_key} must be an object, got {}", json_type_name(gate_obj)),
    })?;

    for cell in CHAIN_CELLS {
        match gate_map.get(*cell) {
            None => {
                return Err(SignalGatesConfigError::MissingKey {
                    key: format!("{gate_key}.{cell}"),
                });
            }
            Some(v) => {
                let Some(s) = v.as_str() else {
                    return Err(SignalGatesConfigError::InvalidValue {
                        key: format!("{gate_key}.{cell}"),
                        value: v.to_string(),
                    });
                };
                if !VALID.contains(&s) {
                    return Err(SignalGatesConfigError::InvalidValue {
                        key: format!("{gate_key}.{cell}"),
                        value: s.to_owned(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Human-readable JSON value kind for structure diagnostics.
fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_config(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    fn recommended_default_json() -> &'static str {
        r#"{
  "$schema_version": 1,
  "commit_gate": {
    "adr_user":     "interim",
    "spec_adr":     "strict",
    "catalog_spec": "strict",
    "impl_catalog": "interim"
  },
  "merge_gate": {
    "adr_user":     "strict",
    "spec_adr":     "strict",
    "catalog_spec": "strict",
    "impl_catalog": "strict"
  }
}"#
    }

    // ── valid config round-trips ──────────────────────────────────────────────

    #[test]
    fn test_load_valid_recommended_default_config_returns_expected_matrix() {
        let f = write_config(recommended_default_json());
        let matrix = load_signal_gates_config(f.path().to_path_buf()).unwrap();

        // commit gate
        assert_eq!(matrix.adr_user.commit_gate, Strictness::Interim);
        assert_eq!(matrix.spec_adr.commit_gate, Strictness::Strict);
        assert_eq!(matrix.catalog_spec.commit_gate, Strictness::Strict);
        assert_eq!(matrix.impl_catalog.commit_gate, Strictness::Interim);

        // merge gate
        assert_eq!(matrix.adr_user.merge_gate, Strictness::Strict);
        assert_eq!(matrix.spec_adr.merge_gate, Strictness::Strict);
        assert_eq!(matrix.catalog_spec.merge_gate, Strictness::Strict);
        assert_eq!(matrix.impl_catalog.merge_gate, Strictness::Strict);
    }

    #[test]
    fn test_load_all_strict_config_returns_all_strict_matrix() {
        let json = r#"{
  "$schema_version": 1,
  "commit_gate": {
    "adr_user":     "strict",
    "spec_adr":     "strict",
    "catalog_spec": "strict",
    "impl_catalog": "strict"
  },
  "merge_gate": {
    "adr_user":     "strict",
    "spec_adr":     "strict",
    "catalog_spec": "strict",
    "impl_catalog": "strict"
  }
}"#;
        let f = write_config(json);
        let matrix = load_signal_gates_config(f.path().to_path_buf()).unwrap();
        for entry in
            [&matrix.adr_user, &matrix.spec_adr, &matrix.catalog_spec, &matrix.impl_catalog]
        {
            assert_eq!(entry.commit_gate, Strictness::Strict);
            assert_eq!(entry.merge_gate, Strictness::Strict);
        }
    }

    // ── file-not-found ────────────────────────────────────────────────────────

    #[test]
    fn test_load_nonexistent_file_returns_file_not_found_error() {
        let path = PathBuf::from("/tmp/nonexistent-signal-gates-xyzzy-12345.json");
        let result = load_signal_gates_config(path.clone());
        assert!(
            matches!(result, Err(SignalGatesConfigError::FileNotFound { .. })),
            "expected FileNotFound, got: {result:?}"
        );
    }

    // ── malformed JSON ────────────────────────────────────────────────────────

    #[test]
    fn test_load_malformed_json_returns_parse_failed_error() {
        let f = write_config("{ not valid json !!!");
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(result, Err(SignalGatesConfigError::ParseFailed { .. })),
            "expected ParseFailed, got: {result:?}"
        );
    }

    #[test]
    fn test_load_non_object_json_returns_parse_failed_error_with_path() {
        let f = write_config("[]");
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(
                result,
                Err(SignalGatesConfigError::ParseFailed { ref path, ref reason })
                if path == &f.path().to_path_buf() && reason.contains("top-level JSON document must be an object")
            ),
            "expected ParseFailed with config path for non-object document, got: {result:?}"
        );
    }

    // ── unknown schema_version ────────────────────────────────────────────────

    #[test]
    fn test_load_schema_version_zero_returns_schema_version_unknown_error() {
        let json = r#"{
  "$schema_version": 0,
  "commit_gate": { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" },
  "merge_gate":  { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" }
}"#;
        let f = write_config(json);
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(
                result,
                Err(SignalGatesConfigError::SchemaVersionUnknown { actual: 0, expected: 1 })
            ),
            "expected SchemaVersionUnknown(0), got: {result:?}"
        );
    }

    #[test]
    fn test_load_schema_version_99_returns_schema_version_unknown_error() {
        let json = r#"{
  "$schema_version": 99,
  "commit_gate": { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" },
  "merge_gate":  { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" }
}"#;
        let f = write_config(json);
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(
                result,
                Err(SignalGatesConfigError::SchemaVersionUnknown { actual: 99, expected: 1 })
            ),
            "expected SchemaVersionUnknown(99), got: {result:?}"
        );
    }

    // ── missing key ───────────────────────────────────────────────────────────

    #[test]
    fn test_load_missing_commit_gate_object_returns_missing_key_error() {
        let json = r#"{
  "$schema_version": 1,
  "merge_gate": { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" }
}"#;
        let f = write_config(json);
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(result, Err(SignalGatesConfigError::MissingKey { ref key }) if key == "commit_gate"),
            "expected MissingKey(commit_gate), got: {result:?}"
        );
    }

    #[test]
    fn test_load_missing_chain_cell_returns_missing_key_with_dotted_path() {
        let json = r#"{
  "$schema_version": 1,
  "commit_gate": { "adr_user": "interim", "spec_adr": "strict", "catalog_spec": "strict" },
  "merge_gate":  { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" }
}"#;
        let f = write_config(json);
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(result, Err(SignalGatesConfigError::MissingKey { ref key }) if key == "commit_gate.impl_catalog"),
            "expected MissingKey(commit_gate.impl_catalog), got: {result:?}"
        );
    }

    // ── invalid strictness value ──────────────────────────────────────────────

    #[test]
    fn test_load_unknown_strictness_string_returns_invalid_value_error() {
        let json = r#"{
  "$schema_version": 1,
  "commit_gate": { "adr_user": "permissive", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "interim" },
  "merge_gate":  { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" }
}"#;
        let f = write_config(json);
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(
                result,
                Err(SignalGatesConfigError::InvalidValue { ref key, ref value })
                if key == "commit_gate.adr_user" && value == "permissive"
            ),
            "expected InvalidValue(commit_gate.adr_user=permissive), got: {result:?}"
        );
    }

    #[test]
    fn test_load_non_string_strictness_cell_returns_invalid_value_error() {
        let json = r#"{
  "$schema_version": 1,
  "commit_gate": { "adr_user": 1, "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "interim" },
  "merge_gate":  { "adr_user": "strict", "spec_adr": "strict", "catalog_spec": "strict", "impl_catalog": "strict" }
}"#;
        let f = write_config(json);
        let result = load_signal_gates_config(f.path().to_path_buf());
        assert!(
            matches!(
                result,
                Err(SignalGatesConfigError::InvalidValue { ref key, ref value })
                if key == "commit_gate.adr_user" && value == "1"
            ),
            "expected InvalidValue(commit_gate.adr_user=1), got: {result:?}"
        );
    }

    // ── Display messages contain actionable text ──────────────────────────────

    #[test]
    fn test_file_not_found_display_contains_actionable_guidance() {
        let err = SignalGatesConfigError::FileNotFound { path: PathBuf::from("/foo/bar.json") };
        let msg = err.to_string();
        assert!(msg.contains("signal-gates.json"), "must mention signal-gates.json: {msg}");
        assert!(msg.contains("/foo/bar.json"), "must include path: {msg}");
    }

    #[test]
    fn test_schema_version_unknown_display_names_expected_and_actual() {
        let err = SignalGatesConfigError::SchemaVersionUnknown { expected: 1, actual: 42 };
        let msg = err.to_string();
        assert!(msg.contains('1'), "must name expected version 1: {msg}");
        assert!(msg.contains("42"), "must name actual version 42: {msg}");
    }

    #[test]
    fn test_missing_key_display_names_the_key() {
        let err = SignalGatesConfigError::MissingKey { key: "commit_gate.impl_catalog".to_owned() };
        let msg = err.to_string();
        assert!(msg.contains("commit_gate.impl_catalog"), "must name the key: {msg}");
    }

    #[test]
    fn test_invalid_value_display_names_key_and_value() {
        let err = SignalGatesConfigError::InvalidValue {
            key: "merge_gate.adr_user".to_owned(),
            value: "permissive".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("merge_gate.adr_user"), "must name the key: {msg}");
        assert!(msg.contains("permissive"), "must name the bad value: {msg}");
    }

    #[test]
    fn test_parse_failed_display_contains_path_and_reason() {
        let err = SignalGatesConfigError::ParseFailed {
            path: PathBuf::from("/harness/config/signal-gates.json"),
            reason: "unexpected end of JSON input".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("signal-gates.json"), "must mention signal-gates.json: {msg}");
        assert!(msg.contains("unexpected end of JSON input"), "must include the reason: {msg}");
    }

    // ── integration: committed .harness/config/signal-gates.json parses ──────

    #[test]
    fn test_committed_signal_gates_json_parses_successfully() {
        // CARGO_MANIFEST_DIR points at libs/infrastructure; go up two levels to the
        // workspace root where .harness/config/signal-gates.json lives.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let config_path = manifest_dir.join("../../.harness/config/signal-gates.json");
        assert!(config_path.exists(), "committed signal-gates.json not found at {:?}", config_path);
        let result = load_signal_gates_config(config_path);
        assert!(
            result.is_ok(),
            "committed signal-gates.json should parse without error, got: {result:?}"
        );
        // The committed default has interim commit-gate for adr_user and impl_catalog,
        // strict for everything else; all merge-gate cells are strict.
        let matrix = result.unwrap();
        assert_eq!(matrix.adr_user.commit_gate, Strictness::Interim);
        assert_eq!(matrix.spec_adr.commit_gate, Strictness::Strict);
        assert_eq!(matrix.catalog_spec.commit_gate, Strictness::Strict);
        assert_eq!(matrix.impl_catalog.commit_gate, Strictness::Interim);
        assert_eq!(matrix.adr_user.merge_gate, Strictness::Strict);
        assert_eq!(matrix.spec_adr.merge_gate, Strictness::Strict);
        assert_eq!(matrix.catalog_spec.merge_gate, Strictness::Strict);
        assert_eq!(matrix.impl_catalog.merge_gate, Strictness::Strict);
    }

    // ── load_signal_gates_config_from_branch ─────────────────────────────────

    /// Helper: create a minimal git repo with a committed `signal-gates.json`
    /// at `.harness/config/signal-gates.json` on the given branch, set up a
    /// self-referencing `origin` remote, and return the temp dir.
    fn setup_git_repo_with_signal_gates(branch: &str, content: &str) -> tempfile::TempDir {
        use super::super::test_support::git_with_identity;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::process::Command::new("git")
            .args(["init", "--quiet", &format!("--initial-branch={branch}")])
            .current_dir(root)
            .status()
            .expect("git init failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .status()
            .ok();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(root)
            .status()
            .ok();
        let config_dir = root.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("signal-gates.json"), content).unwrap();
        git_with_identity(root, &["add", ".harness"]);
        git_with_identity(root, &["commit", "--quiet", "-m", "initial"]);
        std::process::Command::new("git")
            .args(["remote", "add", "origin", root.to_str().unwrap()])
            .current_dir(root)
            .status()
            .ok();
        std::process::Command::new("git")
            .args(["fetch", "--quiet", "origin"])
            .current_dir(root)
            .status()
            .ok();
        dir
    }

    #[test]
    fn test_load_signal_gates_config_from_branch_valid_blob_returns_matrix() {
        let dir = setup_git_repo_with_signal_gates("main", recommended_default_json());
        let result = load_signal_gates_config_from_branch(dir.path(), "main");
        let matrix = result.expect("valid blob on main should parse successfully");
        assert_eq!(matrix.adr_user.commit_gate, Strictness::Interim);
        assert_eq!(matrix.spec_adr.commit_gate, Strictness::Strict);
        assert_eq!(matrix.impl_catalog.merge_gate, Strictness::Strict);
    }

    #[test]
    fn test_load_signal_gates_config_from_branch_not_found_returns_blob_fetch_error() {
        // Repo has NO signal-gates.json committed.
        use super::super::test_support::git_with_identity;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::process::Command::new("git")
            .args(["init", "--quiet", "--initial-branch=main"])
            .current_dir(root)
            .status()
            .expect("git init failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .status()
            .ok();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(root)
            .status()
            .ok();
        std::fs::write(root.join(".gitkeep"), b"").unwrap();
        git_with_identity(root, &["add", ".gitkeep"]);
        git_with_identity(root, &["commit", "--quiet", "-m", "initial"]);
        std::process::Command::new("git")
            .args(["remote", "add", "origin", root.to_str().unwrap()])
            .current_dir(root)
            .status()
            .ok();
        std::process::Command::new("git")
            .args(["fetch", "--quiet", "origin"])
            .current_dir(root)
            .status()
            .ok();

        let result = load_signal_gates_config_from_branch(root, "main");
        assert!(
            matches!(result, Err(SignalGatesConfigError::BlobFetchError { .. })),
            "missing blob must return BlobFetchError, got: {result:?}"
        );
    }

    #[test]
    fn test_load_signal_gates_config_from_branch_malformed_json_returns_parse_failed() {
        let dir = setup_git_repo_with_signal_gates("main", "{ not valid json !!!");
        let result = load_signal_gates_config_from_branch(dir.path(), "main");
        assert!(
            matches!(result, Err(SignalGatesConfigError::ParseFailed { .. })),
            "malformed blob must return ParseFailed, got: {result:?}"
        );
    }

    #[test]
    fn test_load_signal_gates_config_from_branch_bad_branch_returns_blob_fetch_error() {
        let dir = setup_git_repo_with_signal_gates("main", recommended_default_json());
        let result = load_signal_gates_config_from_branch(dir.path(), "does-not-exist");
        assert!(
            matches!(result, Err(SignalGatesConfigError::BlobFetchError { .. })),
            "nonexistent branch must return BlobFetchError, got: {result:?}"
        );
    }
}
