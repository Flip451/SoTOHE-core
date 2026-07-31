//! Filesystem adapter for the declarative pre-review-gate applicability matrix.
//!
//! The review-scope configuration supplies the set of scopes the matrix must
//! cover. This module only decodes the separate gate declaration and passes
//! both sides to the usecase-owned matrix constructor for validation.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::Path;

use domain::review_v2::{MainScopeName, ScopeName};
use domain::{FreeText, TrackId};
use serde::Deserialize;
use usecase::pre_review_gate_dispatch::{
    PreReviewGateConfigLoadError, PreReviewGateConfigLoaderPort, PreReviewGateKind,
    PreReviewGateMatrix,
};

use crate::review_v2::load_v2_scope_config;
use crate::sanitized_failure::{io_classification, scope_config_classification};
use crate::track::symlink_guard::reject_symlinks_below;

/// Repository-relative location of the pre-review gate declaration.
pub(crate) const PRE_REVIEW_GATES_CONFIG: &str = ".harness/config/pre-review-gates.json";

/// Repository-relative location of the review-scope declaration.
const REVIEW_SCOPE_CONFIG: &str = ".harness/config/review-scope.json";
const SUPPORTED_SCHEMA_VERSION: u32 = 1;
/// The gate declaration is a small, hand-authored repository configuration.
const MAX_PRE_REVIEW_GATES_CONFIG_BYTES: u64 = 1024 * 1024;

/// Synchronous filesystem adapter for the scope-to-gate configuration matrix.
#[derive(Debug, Default)]
pub struct FsPreReviewGateConfigLoader;

impl FsPreReviewGateConfigLoader {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PreReviewGateConfigLoaderPort for FsPreReviewGateConfigLoader {
    fn load(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<PreReviewGateMatrix, PreReviewGateConfigLoadError> {
        let (repo, anchor) =
            crate::discover_isolated_repo_for_items_dir(items_dir).map_err(|error| {
                read_failed(format!(
                    "git repository could not be discovered: {}",
                    io_classification(&error)
                ))
            })?;
        ensure_encloses(repo.root(), &anchor)?;
        let root = repo.root();

        let known_scopes = load_v2_scope_config(&root.join(REVIEW_SCOPE_CONFIG), track_id, root)
            .map_err(|error| {
                read_failed(format!(
                    "load {REVIEW_SCOPE_CONFIG}: {}",
                    scope_config_classification(&error)
                ))
            })?
            .all_scope_names();

        let config_path = root.join(PRE_REVIEW_GATES_CONFIG);
        let exists = reject_symlinks_below(&config_path, root).map_err(|error| {
            read_failed(format!("load {PRE_REVIEW_GATES_CONFIG}: {}", io_classification(&error)))
        })?;
        if !exists {
            return Err(read_failed(format!("load {PRE_REVIEW_GATES_CONFIG}: not found")));
        }

        let source = read_bounded_config(&config_path).map_err(|error| {
            read_failed(format!("load {PRE_REVIEW_GATES_CONFIG}: {}", io_classification(&error)))
        })?;
        decode_matrix(&source, known_scopes)
    }
}

fn read_failed(message: impl Into<String>) -> PreReviewGateConfigLoadError {
    PreReviewGateConfigLoadError::ReadFailed { message: FreeText::new(message.into()) }
}

/// Refuses a repository that does not enclose the requested items directory.
///
/// Git discovery begins at the canonical items-directory anchor, but a
/// repository's configuration can cause Git to report a worktree root
/// elsewhere. Loading gate declarations from that other tree would let it
/// govern a track it does not contain, so containment is asserted explicitly.
fn ensure_encloses(root: &Path, anchor: &Path) -> Result<(), PreReviewGateConfigLoadError> {
    let canonical_root = root.canonicalize().map_err(|error| {
        read_failed(format!("repository root could not be resolved: {}", io_classification(&error)))
    })?;
    if anchor.starts_with(&canonical_root) {
        return Ok(());
    }
    Err(read_failed("the discovered repository does not enclose the items directory"))
}

/// Reads the configuration without allowing a growing file to exhaust memory.
fn read_bounded_config(path: &Path) -> Result<String, std::io::Error> {
    let oversized = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "larger than a pre-review gate configuration may be",
        )
    };

    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a regular file"));
    }
    if metadata.len() > MAX_PRE_REVIEW_GATES_CONFIG_BYTES {
        return Err(oversized());
    }

    let file = std::fs::File::open(path)?;
    let mut source = String::new();
    file.take(MAX_PRE_REVIEW_GATES_CONFIG_BYTES.saturating_add(1)).read_to_string(&mut source)?;
    if source.len() as u64 > MAX_PRE_REVIEW_GATES_CONFIG_BYTES {
        return Err(oversized());
    }
    Ok(source)
}

/// Serde-only representation of the configuration file.
///
/// `entries` is a vector rather than a map so duplicate scope declarations
/// reach the usecase constructor and are rejected as matrix violations.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreReviewGatesConfigDto {
    #[serde(rename = "$schema_version")]
    schema_version: u32,
    entries: Vec<ScopeGateEntryDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeGateEntryDto {
    scope: String,
    gates: Vec<PreReviewGateKindDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PreReviewGateKindDto {
    TaskContractLiveness,
}

impl From<PreReviewGateKindDto> for PreReviewGateKind {
    fn from(value: PreReviewGateKindDto) -> Self {
        match value {
            PreReviewGateKindDto::TaskContractLiveness => Self::TaskContractLiveness,
        }
    }
}

fn decode_matrix(
    source: &str,
    known_scopes: HashSet<ScopeName>,
) -> Result<PreReviewGateMatrix, PreReviewGateConfigLoadError> {
    let config: PreReviewGatesConfigDto = serde_json::from_str(source)
        .map_err(|_| read_failed("decode pre-review gate configuration: not valid JSON"))?;
    if config.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(read_failed(format!(
            "decode pre-review gate configuration: unsupported $schema_version {}",
            config.schema_version
        )));
    }

    let entries = config
        .entries
        .into_iter()
        .map(|entry| {
            Ok((decode_scope(&entry.scope)?, entry.gates.into_iter().map(Into::into).collect()))
        })
        .collect::<Result<Vec<_>, PreReviewGateConfigLoadError>>()?;

    PreReviewGateMatrix::try_new(known_scopes, entries)
        .map_err(PreReviewGateConfigLoadError::InvalidMatrix)
}

fn decode_scope(value: &str) -> Result<ScopeName, PreReviewGateConfigLoadError> {
    if value == "other" {
        return Ok(ScopeName::Other);
    }
    MainScopeName::new(value)
        .map(ScopeName::Main)
        .map_err(|_| read_failed("decode pre-review gate configuration: invalid scope name"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use super::*;

    const REVIEW_SCOPE_JSON: &str = r#"{
        "version": 2,
        "groups": {
            "spec": { "patterns": ["track/items/<track-id>/spec.json"] },
            "planning": { "patterns": ["track/items/<track-id>/plan.md"] },
            "implementation": { "patterns": ["libs/**"] }
        }
    }"#;

    const COMPLETE_MATRIX_JSON: &str = r#"{
        "$schema_version": 1,
        "entries": [
            { "scope": "spec", "gates": [] },
            { "scope": "planning", "gates": [] },
            { "scope": "implementation", "gates": ["task_contract_liveness"] },
            { "scope": "other", "gates": [] }
        ]
    }"#;

    fn scope(name: &str) -> ScopeName {
        ScopeName::Main(MainScopeName::new(name).unwrap())
    }

    fn track_id() -> TrackId {
        TrackId::try_new("scope-policy-test").unwrap()
    }

    fn fixture_repo() -> tempfile::TempDir {
        let repo = tempfile::Builder::new()
            .prefix("pre-review-gate-config-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        fs::create_dir_all(repo.path().join(".harness/config")).unwrap();
        fs::create_dir_all(repo.path().join("track/items")).unwrap();
        let status =
            Command::new("git").args(["init", "-q"]).current_dir(repo.path()).status().unwrap();
        assert!(status.success(), "the fixture needs a repository of its own");
        fs::write(repo.path().join(REVIEW_SCOPE_CONFIG), REVIEW_SCOPE_JSON).unwrap();
        repo
    }

    fn write_gate_config(repo: &Path, content: &str) {
        fs::write(repo.join(PRE_REVIEW_GATES_CONFIG), content).unwrap();
    }

    #[test]
    fn test_pre_review_gate_config_loader_port_complete_matrix_returns_declared_gates() {
        let repo = fixture_repo();
        write_gate_config(repo.path(), COMPLETE_MATRIX_JSON);

        let loader: &dyn PreReviewGateConfigLoaderPort = &FsPreReviewGateConfigLoader::new();
        let matrix = loader.load(&repo.path().join("track/items"), &track_id()).unwrap();

        assert_eq!(matrix.gates_for(&scope("spec")).unwrap(), []);
        assert_eq!(matrix.gates_for(&scope("planning")).unwrap(), []);
        assert_eq!(
            matrix.gates_for(&scope("implementation")).unwrap(),
            [PreReviewGateKind::TaskContractLiveness]
        );
        assert_eq!(matrix.gates_for(&ScopeName::Other).unwrap(), []);
    }

    #[test]
    fn test_pre_review_gate_config_loader_missing_file_returns_read_failure() {
        let repo = fixture_repo();

        let error = FsPreReviewGateConfigLoader::new()
            .load(&repo.path().join("track/items"), &track_id())
            .unwrap_err();

        assert!(matches!(error, PreReviewGateConfigLoadError::ReadFailed { .. }));
    }

    #[test]
    fn test_pre_review_gate_config_loader_refuses_repository_outside_items_directory() {
        let outer = fixture_repo();
        let inner = tempfile::Builder::new()
            .prefix("pre-review-gate-config-unrelated-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        fs::create_dir_all(inner.path().join("track/items")).unwrap();

        let init =
            Command::new("git").args(["init", "-q"]).current_dir(inner.path()).status().unwrap();
        assert!(init.success(), "the nested fixture needs a repository of its own");
        let configure = Command::new("git")
            .args([
                "config",
                "core.worktree",
                outer.path().to_str().expect("temporary path must be valid UTF-8"),
            ])
            .current_dir(inner.path())
            .status()
            .unwrap();
        assert!(configure.success(), "the nested fixture must redirect its reported worktree");

        let error = FsPreReviewGateConfigLoader::new()
            .load(&inner.path().join("track/items"), &track_id())
            .expect_err(
                "a repository outside the items directory must not supply gate declarations",
            );

        let message = match error {
            PreReviewGateConfigLoadError::ReadFailed { message } => message,
            PreReviewGateConfigLoadError::InvalidMatrix(error) => {
                panic!("the trusted-root refusal must be a read failure, got: {error}")
            }
        };
        assert!(message.as_str().contains("does not enclose"), "unexpected: {message}");
        assert!(
            !message.as_str().contains(&inner.path().display().to_string())
                && !message.as_str().contains(&outer.path().display().to_string()),
            "no absolute path may reach the operator: {message}"
        );
    }

    #[test]
    fn test_pre_review_gate_config_loader_oversized_file_returns_read_failure() {
        let repo = fixture_repo();
        std::fs::File::create(repo.path().join(PRE_REVIEW_GATES_CONFIG))
            .unwrap()
            .set_len(MAX_PRE_REVIEW_GATES_CONFIG_BYTES.saturating_add(1))
            .unwrap();

        let error = FsPreReviewGateConfigLoader::new()
            .load(&repo.path().join("track/items"), &track_id())
            .unwrap_err();

        assert!(matches!(error, PreReviewGateConfigLoadError::ReadFailed { .. }));
    }

    #[test]
    fn test_pre_review_gate_config_loader_invalid_json_returns_read_failure() {
        let repo = fixture_repo();
        write_gate_config(repo.path(), "{ not json");

        let error = FsPreReviewGateConfigLoader::new()
            .load(&repo.path().join("track/items"), &track_id())
            .unwrap_err();

        assert!(matches!(error, PreReviewGateConfigLoadError::ReadFailed { .. }));
    }

    #[test]
    fn test_pre_review_gate_config_loader_incomplete_matrix_returns_validation_error() {
        let repo = fixture_repo();
        write_gate_config(
            repo.path(),
            r#"{
                "$schema_version": 1,
                "entries": [
                    { "scope": "spec", "gates": [] },
                    { "scope": "implementation", "gates": [] }
                ]
            }"#,
        );

        let error = FsPreReviewGateConfigLoader::new()
            .load(&repo.path().join("track/items"), &track_id())
            .unwrap_err();

        assert!(matches!(error, PreReviewGateConfigLoadError::InvalidMatrix(_)));
    }

    #[test]
    fn test_pre_review_gate_config_loader_unknown_gate_returns_read_failure() {
        let repo = fixture_repo();
        write_gate_config(
            repo.path(),
            r#"{
                "$schema_version": 1,
                "entries": [
                    { "scope": "spec", "gates": ["unknown"] }
                ]
            }"#,
        );

        let error = FsPreReviewGateConfigLoader::new()
            .load(&repo.path().join("track/items"), &track_id())
            .unwrap_err();

        assert!(matches!(error, PreReviewGateConfigLoadError::ReadFailed { .. }));
    }

    #[test]
    fn test_pre_review_gate_config_loader_new_returns_default_adapter() {
        assert_eq!(
            format!("{:?}", FsPreReviewGateConfigLoader::new()),
            format!("{:?}", FsPreReviewGateConfigLoader)
        );
    }
}
