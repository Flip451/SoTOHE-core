//! Filesystem adapter for loading the catalogue lint configuration (ADR D19).
//!
//! [`FsLintConfigLoader`] reads a JSON file whose path is baked in at construction
//! time and returns a [`usecase::catalogue_lint_workflow::LintConfig`].  It
//! implements [`usecase::catalogue_lint_workflow::LintConfigLoader`].

use std::path::PathBuf;

use usecase::catalogue_lint_workflow::{
    LintConfig, LintConfigLoader, LintConfigLoaderError, LintRuleSpec,
};

/// Expected schema version for the lint config JSON file.
const EXPECTED_SCHEMA_VERSION: u32 = 1;

/// Minimal wire format used for the first-pass schema version probe.
///
/// Only `schema_version` is extracted so that an incompatible body (e.g. a
/// future schema with renamed or missing fields) cannot cause `ParseError`
/// before the version is validated.
#[derive(serde::Deserialize)]
struct LintConfigVersionProbe {
    schema_version: u32,
}

/// Wire format for the lint config JSON file (schema_version 1).
///
/// Shape:
/// ```json
/// {
///   "schema_version": 1,
///   "rules": [ { "target_roles": [...], "kind": { ... } }, ... ]
/// }
/// ```
///
/// `schema_version` is intentionally absent: it is validated by
/// [`LintConfigVersionProbe`] before this struct is decoded, so the field is
/// ignored (serde ignores unknown fields by default).
#[derive(serde::Deserialize)]
struct LintConfigFile {
    rules: Vec<LintRuleSpec>,
}

/// Filesystem-backed implementation of [`LintConfigLoader`] (D19).
///
/// Reads `.harness/catalogue-lint/config.json` (or any path supplied at
/// construction).  The path is baked in at construction time; [`load`] takes
/// no path argument.
///
/// [`load`]: FsLintConfigLoader::load
#[derive(Debug)]
pub struct FsLintConfigLoader {
    path: PathBuf,
}

impl FsLintConfigLoader {
    /// Creates a new loader that will read from `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl LintConfigLoader for FsLintConfigLoader {
    fn load(&self) -> Result<LintConfig, LintConfigLoaderError> {
        // 1. Read the file; missing → MissingFile.
        let content = std::fs::read_to_string(&self.path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LintConfigLoaderError::MissingFile { path: self.path.clone() }
            } else {
                LintConfigLoaderError::ParseError { path: self.path.clone(), reason: e.to_string() }
            }
        })?;

        // 2. Probe schema_version before full decode so that an incompatible
        //    body (e.g. future schema) yields SchemaVersionMismatch, not
        //    ParseError.
        let probe: LintConfigVersionProbe = serde_json::from_str(&content).map_err(|e| {
            LintConfigLoaderError::ParseError { path: self.path.clone(), reason: e.to_string() }
        })?;

        // 3. Validate schema_version.
        if probe.schema_version != EXPECTED_SCHEMA_VERSION {
            return Err(LintConfigLoaderError::SchemaVersionMismatch {
                expected: EXPECTED_SCHEMA_VERSION,
                actual: probe.schema_version,
            });
        }

        // 4. Full decode now that the version is confirmed.
        let file: LintConfigFile = serde_json::from_str(&content).map_err(|e| {
            LintConfigLoaderError::ParseError { path: self.path.clone(), reason: e.to_string() }
        })?;

        // 5. Build LintConfig.
        Ok(LintConfig::new(file.rules))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn write_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("config.json");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_load_valid_config_returns_lint_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            dir.path(),
            r#"{ "schema_version": 1, "rules": [
                { "target_roles": [], "kind": "NoPublicField" }
            ] }"#,
        );
        let loader = FsLintConfigLoader::new(path);
        let config = loader.load().unwrap();
        assert_eq!(config.rules().len(), 1);
    }

    #[test]
    fn test_load_missing_file_returns_missing_file_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let loader = FsLintConfigLoader::new(path.clone());
        let err = loader.load().unwrap_err();
        assert!(
            matches!(&err, LintConfigLoaderError::MissingFile { path: p } if p == &path),
            "expected MissingFile, got: {err:?}"
        );
    }

    #[test]
    fn test_load_invalid_json_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "not valid json {{");
        let loader = FsLintConfigLoader::new(path);
        let err = loader.load().unwrap_err();
        assert!(
            matches!(&err, LintConfigLoaderError::ParseError { .. }),
            "expected ParseError, got: {err:?}"
        );
    }

    #[test]
    fn test_load_wrong_schema_version_returns_mismatch_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), r#"{ "schema_version": 99, "rules": [] }"#);
        let loader = FsLintConfigLoader::new(path);
        let err = loader.load().unwrap_err();
        assert!(
            matches!(
                &err,
                LintConfigLoaderError::SchemaVersionMismatch { expected: 1, actual: 99 }
            ),
            "expected SchemaVersionMismatch, got: {err:?}"
        );
    }

    #[test]
    fn test_load_wrong_schema_version_without_rules_field_returns_mismatch_not_parse_error() {
        // Regression: when schema_version != 1 and the body is incompatible
        // (e.g. "rules" field absent, as would occur in a future schema),
        // the loader must return SchemaVersionMismatch, not ParseError.
        // This validates the two-pass decode: probe version first, then
        // decode the full body only after confirming version == 1.
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), r#"{ "schema_version": 2 }"#);
        let loader = FsLintConfigLoader::new(path);
        let err = loader.load().unwrap_err();
        assert!(
            matches!(&err, LintConfigLoaderError::SchemaVersionMismatch { expected: 1, actual: 2 }),
            "expected SchemaVersionMismatch, got: {err:?}"
        );
    }
}
