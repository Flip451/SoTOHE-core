//! Filesystem adapter for loading the catalogue lint configuration (ADR D19).
//!
//! [`FsLintConfigLoader`] reads a JSON file whose path is baked in at construction
//! time and returns a [`usecase::catalogue_lint_workflow::LintConfig`].  It
//! implements [`usecase::catalogue_lint_workflow::LintConfigLoader`].

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

use usecase::catalogue_lint_workflow::{
    LintConfig, LintConfigLoader, LintConfigLoaderError, LintRuleSpec,
};

/// Expected schema version for the lint config JSON file.
const EXPECTED_SCHEMA_VERSION: u32 = 1;
const MAX_LINT_CONFIG_BYTES: u64 = 1024 * 1024;

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
#[derive(serde::Deserialize)]
struct LintConfigFile {
    #[serde(rename = "schema_version")]
    _schema_version: u32,
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

    fn parse_error(path: &Path, reason: impl ToString) -> LintConfigLoaderError {
        LintConfigLoaderError::ParseError { path: path.to_path_buf(), reason: reason.to_string() }
    }

    fn read_error(path: &Path, error: std::io::Error) -> LintConfigLoaderError {
        if error.kind() == std::io::ErrorKind::NotFound {
            LintConfigLoaderError::MissingFile { path: path.to_path_buf() }
        } else {
            Self::parse_error(path, error)
        }
    }

    fn open_config_file(path: &Path) -> Result<File, LintConfigLoaderError> {
        crate::track::symlink_guard::reject_symlinks_up_to_root(path)
            .map_err(|error| Self::parse_error(path, error))?;
        let metadata = path.symlink_metadata().map_err(|error| Self::read_error(path, error))?;
        if metadata.file_type().is_symlink() {
            return Err(Self::parse_error(path, "refusing to load a symlinked lint config"));
        }
        if !metadata.is_file() {
            return Err(Self::parse_error(path, "lint config is not a regular file"));
        }

        #[cfg(unix)]
        let file = {
            use std::os::unix::fs::OpenOptionsExt;

            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)
                .map_err(|error| Self::read_error(path, error))?
        };
        #[cfg(not(unix))]
        let file = File::open(path).map_err(|error| Self::read_error(path, error))?;

        let opened_metadata = file.metadata().map_err(|error| Self::parse_error(path, error))?;
        if !opened_metadata.is_file() {
            return Err(Self::parse_error(path, "lint config is not a regular file"));
        }
        Ok(file)
    }

    fn read_config_file(path: &Path) -> Result<String, LintConfigLoaderError> {
        let file = Self::open_config_file(path)?;
        let metadata = file.metadata().map_err(|error| Self::parse_error(path, error))?;
        if !metadata.is_file() {
            return Err(Self::parse_error(path, "lint config is not a regular file"));
        }
        if metadata.len() > MAX_LINT_CONFIG_BYTES {
            return Err(Self::parse_error(
                path,
                format!("file exceeds maximum size of {MAX_LINT_CONFIG_BYTES} bytes"),
            ));
        }

        let mut content = String::new();
        let mut reader = file.take(MAX_LINT_CONFIG_BYTES.saturating_add(1));
        reader.read_to_string(&mut content).map_err(|error| Self::parse_error(path, error))?;
        if content.len() > MAX_LINT_CONFIG_BYTES as usize {
            return Err(Self::parse_error(
                path,
                format!("file exceeds maximum size of {MAX_LINT_CONFIG_BYTES} bytes"),
            ));
        }
        Ok(content)
    }
}

impl LintConfigLoader for FsLintConfigLoader {
    fn load(&self) -> Result<LintConfig, LintConfigLoaderError> {
        // 1. Read the file; missing → MissingFile.
        let content = Self::read_config_file(&self.path)?;

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
        let LintConfigFile { _schema_version: _, rules } = file;
        Ok(LintConfig::new(rules))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use domain::tddd::catalogue_linter::RoleKind;
    use usecase::catalogue_lint_workflow::LintRuleKind;

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
    fn test_load_oversized_config_returns_parse_error() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.json");
        File::create(&path)?.set_len(MAX_LINT_CONFIG_BYTES.saturating_add(1))?;

        let err = FsLintConfigLoader::new(path).load().unwrap_err();

        assert!(
            matches!(err, LintConfigLoaderError::ParseError { ref reason, .. }
                if reason.contains("maximum size")),
            "expected oversized config ParseError, got: {err:?}"
        );
        Ok(())
    }

    #[test]
    fn test_load_non_regular_file_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();

        let err = FsLintConfigLoader::new(dir.path().to_path_buf()).load().unwrap_err();

        assert!(
            matches!(err, LintConfigLoaderError::ParseError { ref reason, .. }
                if reason.contains("not a regular file")),
            "expected non-regular config ParseError, got: {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_symlinked_file_returns_parse_error() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let target = write_config(dir.path(), r#"{ "schema_version": 1, "rules": [] }"#);
        let link = dir.path().join("config-link.json");
        std::os::unix::fs::symlink(target, &link)?;

        let err = FsLintConfigLoader::new(link).load().unwrap_err();

        assert!(
            matches!(err, LintConfigLoaderError::ParseError { ref reason, .. }
                if reason.contains("symlink")),
            "expected symlinked config ParseError, got: {err:?}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_load_file_below_symlinked_parent_returns_parse_error() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let real_parent = dir.path().join("real-config");
        std::fs::create_dir(&real_parent)?;
        write_config(&real_parent, r#"{ "schema_version": 1, "rules": [] }"#);
        let linked_parent = dir.path().join("linked-config");
        std::os::unix::fs::symlink(&real_parent, &linked_parent)?;

        let err = FsLintConfigLoader::new(linked_parent.join("config.json")).load().unwrap_err();

        assert!(
            matches!(err, LintConfigLoaderError::ParseError { ref reason, .. }
                if reason.contains("refusing to follow symlink")),
            "expected symlinked-parent config ParseError, got: {err:?}"
        );
        Ok(())
    }

    #[test]
    fn test_load_unknown_top_level_field_preserves_forward_compatibility() {
        let dir = tempfile::tempdir().unwrap();
        let path =
            write_config(dir.path(), r#"{ "schema_version": 1, "rules": [], "unexpected": true }"#);
        let loader = FsLintConfigLoader::new(path);
        let config = loader.load().expect("unknown optional top-level fields must be ignored");
        assert!(config.rules().is_empty());
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

    #[test]
    fn test_shipped_config_and_preset_decode_to_identical_narrowed_primary_adapter_rules() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config_path = repo_root.join(".harness/catalogue-lint/config.json");
        let preset_path = repo_root.join(".harness/catalogue-lint/presets/ddd-strict.json");

        let config = FsLintConfigLoader::new(config_path)
            .load()
            .expect("shipped config.json must decode through the production loader");
        let preset = FsLintConfigLoader::new(preset_path)
            .load()
            .expect("shipped ddd-strict.json must decode through the production loader");

        let config_rules = serde_json::to_value(config.rules())
            .expect("decoded config rules must serialize for structural comparison");
        let preset_rules = serde_json::to_value(preset.rules())
            .expect("decoded preset rules must serialize for structural comparison");
        assert_eq!(
            config_rules, preset_rules,
            "shipped config and preset must have identical rules"
        );

        for rule in config.rules() {
            let rule_kind = serde_json::to_value(&rule.kind)
                .expect("decoded rule kind must serialize for obsolete-variant check");
            assert!(
                rule_kind.get("DomainValueObjectInboundReferenceRequired").is_none(),
                "the removed inbound-reference rule must not be present in shipped configuration"
            );
        }

        let forbidden_roles = config
            .rules()
            .iter()
            .find_map(|rule| match (&rule.target_roles[..], &rule.kind) {
                (_, LintRuleKind::NoRoleInMethodSignature { forbidden_roles })
                    if rule.target_roles == ["PrimaryAdapter"] =>
                {
                    Some(forbidden_roles)
                }
                _ => None,
            })
            .expect("shipped config must retain the PrimaryAdapter signature boundary rule");
        assert_eq!(
            forbidden_roles.as_slice(),
            &[
                RoleKind::Entity,
                RoleKind::AggregateRoot,
                RoleKind::Repository,
                RoleKind::SecondaryPort,
                RoleKind::SecondaryAdapter,
            ],
            "PrimaryAdapter must forbid only the narrowed structural boundary roles"
        );
    }
}
