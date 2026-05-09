//! Loader for `rustdoc_types::Crate` JSON (TypeGraph B — Baseline, or C — Current).
//!
//! `BaselineRustdocCodec` deserializes a `rustdoc_types::Crate` from a JSON
//! file produced by `cargo +nightly rustdoc --output-format json`.
//!
//! ## Responsibilities
//!
//! * Read raw bytes from the filesystem.
//! * Deserialize via `serde_json` into `rustdoc_types::Crate`.
//! * Validate `format_version` against the compile-time constant
//!   `rustdoc_types::FORMAT_VERSION` to detect schema mismatches early.
//!
//! ## Non-responsibilities
//!
//! * The codec does not verify the semantic content of the crate graph.
//! * It does not convert to `ExtendedCrate`; that is the job of
//!   `CatalogueToExtendedCrateCodec` (T005).
//!
//! (ADR 2 D6 / CN-04 / AC-04)

use std::io;
use std::path::Path;

use rustdoc_types::{Crate, FORMAT_VERSION};
use serde::Deserialize;
use thiserror::Error;

/// Minimal helper used to extract just `format_version` without deserializing
/// the full `rustdoc_types::Crate` (which may fail for mismatched schemas).
#[derive(Deserialize)]
struct RustdocFormatVersion {
    format_version: u32,
}

// ---------------------------------------------------------------------------
// Error type — BaselineRustdocCodecError
// ---------------------------------------------------------------------------

/// Error returned by `BaselineRustdocCodec::load` and `BaselineRustdocCodec::from_json`.
#[derive(Debug, Error)]
pub enum BaselineRustdocCodecError {
    /// `serde_json` failed to deserialize the file content.
    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Filesystem I/O error (file not found, permission denied, …).
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    /// The JSON file's `format_version` does not match
    /// `rustdoc_types::FORMAT_VERSION`.
    ///
    /// This typically indicates a mismatch between the nightly toolchain that
    /// generated the JSON and the `rustdoc-types` crate version compiled into
    /// this binary.
    #[error(
        "unsupported rustdoc format_version: file has {actual}, binary expects {expected}. \
         Re-generate the rustdoc JSON with the matching nightly toolchain."
    )]
    UnsupportedFormatVersion {
        /// Version number found in the JSON file.
        actual: u32,
        /// Version number expected by the compiled `rustdoc-types` crate.
        expected: u32,
    },
}

// ---------------------------------------------------------------------------
// Codec struct
// ---------------------------------------------------------------------------

/// Loads a `rustdoc_types::Crate` from a rustdoc JSON file.
///
/// Typically used to load TypeGraph B (Baseline) or TypeGraph C (Current).
/// Both use the same wire format (`rustdoc_types::Crate` JSON) and the same
/// deserialization path.
///
/// This is an infrastructure-internal type; it is not part of the public
/// TDDD catalogue (infrastructure-types.json) and should not be used outside
/// the infrastructure layer.
///
/// T005 (`CatalogueToExtendedCrateCodec`) and downstream callers will use this
/// codec to load TypeGraph B (Baseline) and C (Current). The `#[allow(dead_code)]`
/// suppresses the "never constructed" lint while the codec has no caller yet.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct BaselineRustdocCodec;

// T005 callers will use these methods once `CatalogueToExtendedCrateCodec` is
// implemented. Suppress the dead-code lint until that point.
#[allow(dead_code)]
impl BaselineRustdocCodec {
    /// Loads and deserializes a `rustdoc_types::Crate` from the given JSON file.
    ///
    /// # Errors
    ///
    /// Returns `BaselineRustdocCodecError::IoError` if the file cannot be read.
    ///
    /// Returns `BaselineRustdocCodecError::Json` if `serde_json` fails to
    /// deserialize the file content.
    ///
    /// Returns `BaselineRustdocCodecError::UnsupportedFormatVersion` if the
    /// file's `format_version` differs from `rustdoc_types::FORMAT_VERSION`.
    pub(crate) fn load(path: &Path) -> Result<Crate, BaselineRustdocCodecError> {
        let content = std::fs::read_to_string(path)?;
        Self::check_and_deserialize(&content)
    }

    /// Deserializes a `rustdoc_types::Crate` from an in-memory JSON string.
    ///
    /// Intended for in-process pipeline use-cases where the caller already
    /// holds the JSON bytes in memory (e.g., tests, direct rustdoc output).
    ///
    /// # Errors
    ///
    /// Returns `BaselineRustdocCodecError::Json` if deserialization fails.
    ///
    /// Returns `BaselineRustdocCodecError::UnsupportedFormatVersion` if the
    /// `format_version` field does not match the expected constant.
    pub(crate) fn from_json(json: &str) -> Result<Crate, BaselineRustdocCodecError> {
        Self::check_and_deserialize(json)
    }

    /// Checks `format_version` first (without full `Crate` deserialization), then
    /// deserializes the full `rustdoc_types::Crate`.
    ///
    /// This two-phase approach ensures that a schema mismatch (e.g. an older
    /// rustdoc JSON that is missing required fields introduced in a later version)
    /// is reported as `UnsupportedFormatVersion` rather than as a `Json` error,
    /// which would obscure the real root cause.
    ///
    /// # Errors
    ///
    /// Returns `BaselineRustdocCodecError::Json` if the input is not valid JSON
    /// or if `format_version` cannot be extracted.
    ///
    /// Returns `BaselineRustdocCodecError::UnsupportedFormatVersion` if the
    /// `format_version` field does not match `FORMAT_VERSION`.
    ///
    /// Returns `BaselineRustdocCodecError::Json` if full `Crate` deserialization
    /// fails after the version check passes (e.g. malformed field values).
    fn check_and_deserialize(json: &str) -> Result<Crate, BaselineRustdocCodecError> {
        // Phase 1: extract format_version only.
        let version_probe: RustdocFormatVersion = serde_json::from_str(json)?;
        if version_probe.format_version != FORMAT_VERSION {
            return Err(BaselineRustdocCodecError::UnsupportedFormatVersion {
                actual: version_probe.format_version,
                expected: FORMAT_VERSION,
            });
        }
        // Phase 2: full deserialization now that the schema version is confirmed.
        let krate: Crate = serde_json::from_str(json)?;
        Ok(krate)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::io::Write as _;

    use rustdoc_types::{FORMAT_VERSION, Id};
    use tempfile::NamedTempFile;

    use super::*;

    /// Minimal JSON for a valid `rustdoc_types::Crate` with correct `format_version`.
    ///
    /// Note: rustdoc_types::Id(u32) is serde-transparent so it deserializes from a
    /// JSON number (not a quoted string). `"root": 0` → Id(0).
    fn minimal_crate_json(format_version: u32) -> String {
        format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {format_version},
                "target": {{"triple": "", "target_features": []}}
            }}"#
        )
    }

    #[test]
    fn test_load_valid_json_from_file_succeeds() {
        let json = minimal_crate_json(FORMAT_VERSION);
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(json.as_bytes()).unwrap();
        let krate = BaselineRustdocCodec::load(tmp.path()).unwrap();
        assert_eq!(krate.format_version, FORMAT_VERSION);
        // root Id(0) corresponds to JSON numeric 0 (serde-transparent Id(u32))
        assert_eq!(krate.root, Id(0));
    }

    #[test]
    fn test_load_nonexistent_file_returns_io_error() {
        let result = BaselineRustdocCodec::load(Path::new("/nonexistent/path/does-not-exist.json"));
        assert!(matches!(result, Err(BaselineRustdocCodecError::IoError(_))));
    }

    #[test]
    fn test_load_invalid_json_returns_json_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"{ not valid json }").unwrap();
        let result = BaselineRustdocCodec::load(tmp.path());
        assert!(matches!(result, Err(BaselineRustdocCodecError::Json(_))));
    }

    #[test]
    fn test_load_wrong_format_version_returns_unsupported_error() {
        let json = minimal_crate_json(FORMAT_VERSION + 1);
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(json.as_bytes()).unwrap();
        let result = BaselineRustdocCodec::load(tmp.path());
        assert!(matches!(
            result,
            Err(BaselineRustdocCodecError::UnsupportedFormatVersion { actual, expected })
                if actual == FORMAT_VERSION + 1 && expected == FORMAT_VERSION
        ));
    }

    #[test]
    fn test_from_json_valid_json_succeeds() {
        let json = minimal_crate_json(FORMAT_VERSION);
        let krate = BaselineRustdocCodec::from_json(&json).unwrap();
        assert_eq!(krate.format_version, FORMAT_VERSION);
    }

    #[test]
    fn test_from_json_invalid_json_returns_json_error() {
        let result = BaselineRustdocCodec::from_json("{ broken }");
        assert!(matches!(result, Err(BaselineRustdocCodecError::Json(_))));
    }

    #[test]
    fn test_from_json_wrong_format_version_returns_unsupported_error() {
        let json = minimal_crate_json(0);
        let result = BaselineRustdocCodec::from_json(&json);
        assert!(matches!(
            result,
            Err(BaselineRustdocCodecError::UnsupportedFormatVersion { actual: 0, .. })
        ));
    }

    #[test]
    fn test_baseline_rustdoc_codec_error_io_display() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let err = BaselineRustdocCodecError::IoError(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"), "unexpected message: {msg}");
    }

    #[test]
    fn test_baseline_rustdoc_codec_error_unsupported_version_display() {
        let err = BaselineRustdocCodecError::UnsupportedFormatVersion {
            actual: 99,
            expected: FORMAT_VERSION,
        };
        let msg = err.to_string();
        assert!(msg.contains("99"), "expected actual version in message: {msg}");
        assert!(
            msg.contains(&FORMAT_VERSION.to_string()),
            "expected expected version in message: {msg}"
        );
    }

    /// Verifies the two-phase deserialization contract: a JSON that carries a wrong
    /// `format_version` AND is missing required `Crate` fields (simulating an older
    /// schema that predates the current `rustdoc_types` version) must be reported as
    /// `UnsupportedFormatVersion`, not as a `Json` deserialization error.
    ///
    /// Without the two-phase approach the full `serde_json::from_str::<Crate>`
    /// call would fire first and return `Json` before the version check could run.
    #[test]
    fn test_wrong_version_with_missing_fields_returns_unsupported_format_version_not_json_error() {
        // JSON with a wrong `format_version` and intentionally omitted required fields
        // (e.g. `target` and `paths`) to simulate an older rustdoc schema.
        let json = r#"{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {},
                "external_crates": {},
                "format_version": 1
            }"#;
        // Must be UnsupportedFormatVersion — NOT Json — even though full Crate
        // deserialization would fail due to the missing fields.
        let result = BaselineRustdocCodec::from_json(json);
        assert!(
            matches!(
                result,
                Err(BaselineRustdocCodecError::UnsupportedFormatVersion { actual: 1, .. })
            ),
            "expected UnsupportedFormatVersion(1), got: {result:?}"
        );
    }
}
