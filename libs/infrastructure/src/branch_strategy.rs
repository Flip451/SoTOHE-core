//! Branch strategy adapters implementing [`usecase::branch_strategy::BranchStrategyPort`].
//!
//! Two adapters:
//! - [`JsonConfigBranchStrategyAdapter`]: reads `.harness/config/branch-strategy.json` at
//!   construction time; used at track init before a snapshot exists.
//! - [`SnapshotBranchStrategyAdapter`]: wraps a [`domain::branch_strategy::BranchStrategySnapshot`]
//!   captured at track init time; used for all post-init operations (CN-02).

use std::path::{Component, Path, PathBuf};

use domain::branch_strategy::{BranchStrategySnapshot, MergeMethod};
use usecase::branch_strategy::BranchStrategyPort;

use crate::track::codec::MergeMethodDocument;
use crate::track::symlink_guard::reject_symlinks_below;

// ── Config file DTO ───────────────────────────────────────────────────────────

/// Internal DTO for `.harness/config/branch-strategy.json`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BranchStrategyConfigDocument {
    base_branch: String,
    merge_target: String,
    merge_method: MergeMethodDocument,
}

// ── Error ─────────────────────────────────────────────────────────────────────

/// Error returned by [`JsonConfigBranchStrategyAdapter::new`] when the branch-strategy
/// config file cannot be read (Io) or cannot be parsed as valid JSON (Parse).
/// Fail-closed per CN-03: no fallback, no grace period.
#[derive(Debug, thiserror::Error)]
pub enum BranchStrategyConfigError {
    #[error("I/O error reading branch strategy config: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error in branch strategy config: {0}")]
    Parse(#[from] serde_json::Error),
}

// ── JsonConfigBranchStrategyAdapter ──────────────────────────────────────────

/// Reads branch strategy from `.harness/config/branch-strategy.json`.
/// Implements [`BranchStrategyPort`] for the global-config resolution path
/// (used at track init time before a snapshot exists).
pub struct JsonConfigBranchStrategyAdapter {
    base_branch: String,
    merge_target: String,
    merge_method: MergeMethod,
}

impl JsonConfigBranchStrategyAdapter {
    /// Create an adapter that reads and parses the branch strategy from the
    /// given JSON config file path. Returns Err if the file cannot be read or parsed.
    pub fn new(config_path: PathBuf) -> Result<Self, BranchStrategyConfigError> {
        guard_config_path(&config_path)?;
        let content = std::fs::read_to_string(&config_path)?;
        let doc: BranchStrategyConfigDocument = serde_json::from_str(&content)?;
        let merge_method = match doc.merge_method {
            MergeMethodDocument::Squash => MergeMethod::Squash,
            MergeMethodDocument::Merge => MergeMethod::Merge,
            MergeMethodDocument::Rebase => MergeMethod::Rebase,
        };
        Ok(Self { base_branch: doc.base_branch, merge_target: doc.merge_target, merge_method })
    }
}

fn guard_config_path(config_path: &Path) -> Result<(), std::io::Error> {
    if config_path.components().any(|component| component == Component::ParentDir) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "refusing branch strategy config path with parent traversal: {}",
                config_path.display()
            ),
        ));
    }

    let trusted_root = infer_config_trusted_root(config_path);
    match trusted_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "refusing to use symlinked branch strategy trusted root: {}",
                    trusted_root.display()
                ),
            ));
        }
        Ok(_) => {}
        Err(source) => {
            return Err(std::io::Error::new(
                source.kind(),
                format!(
                    "failed to stat branch strategy trusted root {}: {source}",
                    trusted_root.display()
                ),
            ));
        }
    }

    match reject_symlinks_below(config_path, &trusted_root)? {
        true => Ok(()),
        false => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("branch strategy config not found: {}", config_path.display()),
        )),
    }
}

fn infer_config_trusted_root(config_path: &Path) -> PathBuf {
    for ancestor in config_path.ancestors() {
        if ancestor.file_name().and_then(|name| name.to_str()) == Some(".harness") {
            return non_empty_path(ancestor.parent().unwrap_or_else(|| Path::new(".")));
        }
    }

    non_empty_path(config_path.parent().unwrap_or_else(|| Path::new(".")))
}

fn non_empty_path(path: &Path) -> PathBuf {
    if path.as_os_str().is_empty() { PathBuf::from(".") } else { path.to_path_buf() }
}

impl BranchStrategyPort for JsonConfigBranchStrategyAdapter {
    fn base_branch(&self) -> &str {
        &self.base_branch
    }

    fn merge_target(&self) -> &str {
        &self.merge_target
    }

    fn merge_method(&self) -> MergeMethod {
        self.merge_method
    }

    /// Always returns `"track/"` per CN-04 (prefix is fixed).
    fn track_prefix(&self) -> &str {
        "track/"
    }
}

// ── SnapshotBranchStrategyAdapter ─────────────────────────────────────────────

/// Reads branch strategy from a captured [`BranchStrategySnapshot`] stored in
/// `metadata.json`. Implements [`BranchStrategyPort`] for all post-init operations
/// (CN-02: no re-read of global config).
pub struct SnapshotBranchStrategyAdapter {
    snapshot: BranchStrategySnapshot,
}

impl SnapshotBranchStrategyAdapter {
    /// Create an adapter backed by a captured [`BranchStrategySnapshot`].
    pub fn new(snapshot: BranchStrategySnapshot) -> Self {
        Self { snapshot }
    }
}

impl BranchStrategyPort for SnapshotBranchStrategyAdapter {
    fn base_branch(&self) -> &str {
        self.snapshot.base_branch()
    }

    fn merge_target(&self) -> &str {
        self.snapshot.merge_target()
    }

    fn merge_method(&self) -> MergeMethod {
        self.snapshot.merge_method()
    }

    /// Always returns `"track/"` per CN-04 (prefix is fixed).
    fn track_prefix(&self) -> &str {
        "track/"
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use domain::NonEmptyString;

    fn make_snapshot(base: &str, target: &str, method: MergeMethod) -> BranchStrategySnapshot {
        BranchStrategySnapshot::new(
            NonEmptyString::try_new(base).unwrap(),
            NonEmptyString::try_new(target).unwrap(),
            method,
        )
    }

    #[test]
    fn snapshot_adapter_returns_snapshot_values() {
        let snap = make_snapshot("develop", "develop", MergeMethod::Squash);
        let adapter = SnapshotBranchStrategyAdapter::new(snap);
        assert_eq!(adapter.base_branch(), "develop");
        assert_eq!(adapter.merge_target(), "develop");
        assert_eq!(adapter.merge_method(), MergeMethod::Squash);
        assert_eq!(adapter.track_prefix(), "track/");
    }

    #[test]
    fn snapshot_adapter_merge_method_rebase() {
        let snap = make_snapshot("main", "main", MergeMethod::Rebase);
        let adapter = SnapshotBranchStrategyAdapter::new(snap);
        assert_eq!(adapter.merge_method(), MergeMethod::Rebase);
    }

    #[test]
    fn json_config_adapter_missing_file_returns_io_error() {
        let result = JsonConfigBranchStrategyAdapter::new(PathBuf::from(
            "/nonexistent/path/branch-strategy.json",
        ));
        assert!(
            matches!(result, Err(BranchStrategyConfigError::Io(_))),
            "missing file must return Io error"
        );
    }

    #[test]
    fn json_config_adapter_invalid_json_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("branch-strategy.json");
        std::fs::write(&path, "not valid json").unwrap();
        let result = JsonConfigBranchStrategyAdapter::new(path);
        assert!(
            matches!(result, Err(BranchStrategyConfigError::Parse(_))),
            "invalid JSON must return Parse error"
        );
    }

    #[test]
    fn json_config_adapter_valid_file_returns_correct_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("branch-strategy.json");
        std::fs::write(
            &path,
            r#"{"base_branch": "develop", "merge_target": "develop", "merge_method": "squash"}"#,
        )
        .unwrap();
        let adapter = JsonConfigBranchStrategyAdapter::new(path).unwrap();
        assert_eq!(adapter.base_branch(), "develop");
        assert_eq!(adapter.merge_target(), "develop");
        assert_eq!(adapter.merge_method(), MergeMethod::Squash);
        assert_eq!(adapter.track_prefix(), "track/");
    }

    #[test]
    fn json_config_adapter_rebase_method() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("branch-strategy.json");
        std::fs::write(
            &path,
            r#"{"base_branch": "main", "merge_target": "main", "merge_method": "rebase"}"#,
        )
        .unwrap();
        let adapter = JsonConfigBranchStrategyAdapter::new(path).unwrap();
        assert_eq!(adapter.merge_method(), MergeMethod::Rebase);
    }

    #[cfg(unix)]
    #[test]
    fn json_config_adapter_rejects_symlinked_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let harness_config = dir.path().join(".harness/config");
        std::fs::create_dir_all(&harness_config).unwrap();
        let real = dir.path().join("real-branch-strategy.json");
        std::fs::write(
            &real,
            r#"{"base_branch": "main", "merge_target": "main", "merge_method": "squash"}"#,
        )
        .unwrap();
        let link = harness_config.join("branch-strategy.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let result = JsonConfigBranchStrategyAdapter::new(link);
        assert!(
            matches!(result, Err(BranchStrategyConfigError::Io(_))),
            "symlinked config file must fail closed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn json_config_adapter_rejects_symlinked_harness_parent() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("config")).unwrap();
        std::fs::write(
            outside.path().join("config/branch-strategy.json"),
            r#"{"base_branch": "main", "merge_target": "main", "merge_method": "squash"}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".harness")).unwrap();

        let result = JsonConfigBranchStrategyAdapter::new(
            dir.path().join(".harness/config/branch-strategy.json"),
        );
        assert!(
            matches!(result, Err(BranchStrategyConfigError::Io(_))),
            "symlinked .harness parent must fail closed"
        );
    }
}
