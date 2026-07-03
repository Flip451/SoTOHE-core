//! Infrastructure adapters implementing the four
//! `usecase::dry_driver_shared` secondary ports.
//!
//! Each adapter performs a single narrow I/O concern (ADR 2026-06-21-1328 D7 /
//! R1: `SecondaryAdapter` is infrastructure-only) — the orchestration itself
//! lives in the T027/T029/T030 driver interactors. This module is purely
//! additive: nothing outside it references these adapters yet, so it
//! compiles standalone without touching cli_driver or cli_composition.
//!
//! Design: ADR 2026-06-21-1328 D7, IN-14, AC-20.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::TrackId;
use usecase::dry_driver_shared::{
    DryBaseBranchError, DryBaseBranchPort, DryCheckStorageFactoryPort, DryCheckStorageHandle,
    DryDiffBaseFactoryPort, DryRepoRootPort, DryRepoWorkspace, DryRepoWorkspaceError,
};

use crate::dry_check::diff_base_resolver::FsDiffBaseResolverAdapter;
use crate::dry_check::{FsDryCheckCoverageAdapter, FsDryCheckStore};
use crate::git_cli::{GitRepository as _, SystemGitRepo};
use crate::track::symlink_guard::reject_symlinks_below;

// ── Local helpers (reimplemented — see per-fn docs) ───────────────────────────

/// Resolve an input directory and require it to stay inside the repository root.
///
/// Reimplemented locally: `libs/infrastructure` cannot depend on the
/// `cli_composition`-private
/// `apps/cli-composition/src/dry/shared.rs::resolve_existing_dir_under_repo`
/// helper. Same validation semantics: existing directory, must resolve under
/// the canonicalized repo root.
fn resolve_existing_dir_under_repo(
    input_path: &Path,
    repo_root: &Path,
    canonical_root: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let absolute_path = if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        repo_root.join(input_path)
    };
    reject_symlinks_below(&absolute_path, canonical_root)
        .map_err(|e| format!("symlink guard {label} '{}': {e}", input_path.display()))?;
    let canonical_path = absolute_path.canonicalize().map_err(|_| {
        format!(
            "{label} '{}' must be an existing directory under the repository root",
            input_path.display()
        )
    })?;

    if !canonical_path.is_dir() || !canonical_path.starts_with(canonical_root) {
        return Err(format!(
            "{label} '{}' must be an existing directory under the repository root",
            input_path.display()
        ));
    }

    Ok(canonical_path)
}

fn resolve_metadata_path_under_repo(
    track_dir: &Path,
    canonical_root: &Path,
) -> Result<PathBuf, DryBaseBranchError> {
    let metadata_path = track_dir.join("metadata.json");
    let absolute_path = if metadata_path.is_absolute() {
        metadata_path
    } else {
        canonical_root.join(metadata_path)
    };

    if absolute_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
        || !absolute_path.starts_with(canonical_root)
    {
        return Err(DryBaseBranchError::Unavailable(format!(
            "metadata.json path '{}' resolves outside repo root '{}'",
            absolute_path.display(),
            canonical_root.display()
        )));
    }

    Ok(absolute_path)
}

// ── FsDryRepoRootAdapter ───────────────────────────────────────────────────────

/// Filesystem/git adapter implementing [`DryRepoRootPort`].
///
/// Reproduces the CWD-anchored repo discovery + `items_dir`
/// canonicalize/containment check previously duplicated inline at the top of
/// `DryCompositionRoot::dry_write` / `dry_results` / `dry_check_approved`.
pub struct FsDryRepoRootAdapter;

impl DryRepoRootPort for FsDryRepoRootAdapter {
    fn resolve(&self, items_dir: &Path) -> Result<DryRepoWorkspace, DryRepoWorkspaceError> {
        let git = SystemGitRepo::discover()
            .map_err(|e| DryRepoWorkspaceError::Unavailable(format!("git discover: {e}")))?;
        let repo_root = git.root().to_path_buf();
        let canonical_root = repo_root.canonicalize().map_err(|e| {
            DryRepoWorkspaceError::Unavailable(format!("failed to canonicalize repo root: {e}"))
        })?;
        let canonical_items_dir =
            resolve_existing_dir_under_repo(items_dir, &repo_root, &canonical_root, "items_dir")
                .map_err(DryRepoWorkspaceError::Unavailable)?;

        Ok(DryRepoWorkspace { repo_root, canonical_root, canonical_items_dir })
    }
}

// ── FsDryBaseBranchAdapter ─────────────────────────────────────────────────────

/// Filesystem adapter implementing [`DryBaseBranchPort`].
///
/// Reads `<track_dir>/metadata.json#branch_strategy_snapshot.base_branch`.
/// Fail-closed per IN-06/IN-07: a missing, unreadable, or malformed
/// `metadata.json` returns `DryBaseBranchError::Unavailable` — there is no
/// `.harness/config/branch-strategy.json` fallback and no hardcoded default.
pub struct FsDryBaseBranchAdapter;

impl DryBaseBranchPort for FsDryBaseBranchAdapter {
    fn resolve_base_branch(
        &self,
        canonical_root: &Path,
        track_dir: &Path,
        track_id: &TrackId,
    ) -> Result<String, DryBaseBranchError> {
        let metadata_path = resolve_metadata_path_under_repo(track_dir, canonical_root)?;
        match crate::track::symlink_guard::reject_symlinks_below(&metadata_path, canonical_root) {
            Ok(true) => {}
            Ok(false) => {
                // metadata.json is not present — fall through to the NotFound branch below.
            }
            Err(e) => {
                return Err(DryBaseBranchError::Unavailable(format!(
                    "symlink guard metadata.json for '{}': {e}",
                    track_id.as_ref()
                )));
            }
        }
        match std::fs::read_to_string(&metadata_path) {
            Ok(metadata_json) => {
                let (track_meta, _) = crate::track::codec::decode(&metadata_json).map_err(|e| {
                    DryBaseBranchError::Unavailable(format!(
                        "decode metadata.json for '{}': {e} (track metadata schema v6 required)",
                        track_id.as_ref()
                    ))
                })?;
                Ok(track_meta.branch_strategy_snapshot().base_branch().to_owned())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(DryBaseBranchError::Unavailable(format!(
                    "metadata.json for '{}' not found",
                    track_id.as_ref()
                )))
            }
            Err(e) => Err(DryBaseBranchError::Unavailable(format!(
                "read metadata.json for '{}': {e}",
                track_id.as_ref()
            ))),
        }
    }
}

// ── FsDryCheckStorageFactoryAdapter ───────────────────────────────────────────

/// Filesystem adapter implementing [`DryCheckStorageFactoryPort`].
///
/// Infallible pure construction of the reader/writer/coverage trio bound to
/// `<track_dir>/dry-check.json` / `dry-check-coverage.json`.
pub struct FsDryCheckStorageFactoryAdapter;

impl DryCheckStorageFactoryPort for FsDryCheckStorageFactoryAdapter {
    fn build(&self, track_dir: &Path, canonical_root: &Path) -> DryCheckStorageHandle {
        let dry_check_json_path = track_dir.join("dry-check.json");
        let dry_check_coverage_path = track_dir.join("dry-check-coverage.json");
        let store =
            Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root.to_path_buf()));
        let coverage = Arc::new(FsDryCheckCoverageAdapter::new(
            dry_check_coverage_path,
            canonical_root.to_path_buf(),
        ));
        DryCheckStorageHandle {
            reader: store.clone() as Arc<dyn domain::dry_check::DryCheckReader>,
            writer: store as Arc<dyn domain::dry_check::DryCheckWriter>,
            coverage: coverage as Arc<dyn usecase::dry_check::DryCheckCoveragePort>,
        }
    }
}

// ── FsDryDiffBaseFactoryAdapter ───────────────────────────────────────────────

/// Factory adapter implementing [`DryDiffBaseFactoryPort`].
///
/// Mirrors `FsFixpointDryGateFactoryAdapter`'s construction pattern for the
/// same underlying adapter. Infallible — no I/O at `build()` time.
pub struct FsDryDiffBaseFactoryAdapter;

impl DryDiffBaseFactoryPort for FsDryDiffBaseFactoryAdapter {
    fn build(&self, base_branch: &str) -> Arc<dyn usecase::fixpoint_resolve::DiffBaseResolverPort> {
        Arc::new(FsDiffBaseResolverAdapter::new(base_branch.to_owned()))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::process::Command;
    use std::sync::Mutex;

    use super::*;

    /// Serializes tests that rely on `SystemGitRepo::discover()`'s CWD-implicit
    /// behavior. Mirrors `crate::dry_check::diff_getter`'s local `CWD_LOCK` pattern.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn run_git(path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .status()
            .expect("git must run");
        assert!(status.success(), "git {args:?} failed with {status}");
    }

    /// Create a minimal git repo with an initial commit on `main`. Returns
    /// `(tempdir, items_dir)`.
    fn seed_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);
        let items_dir = root.join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        (dir, items_dir)
    }

    fn test_track_id() -> TrackId {
        TrackId::try_new("my-track-2026").unwrap()
    }

    // ── FsDryRepoRootAdapter ────────────────────────────────────────────────

    #[test]
    fn test_fs_dry_repo_root_adapter_resolves_items_dir_under_repo() {
        let _lock = CWD_LOCK.lock().unwrap();
        let (dir, items_dir) = seed_repo();
        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(dir.path()).expect("set_current_dir must succeed");

        let result = FsDryRepoRootAdapter.resolve(&PathBuf::from("track/items"));

        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");
        let workspace = result.unwrap();
        assert_eq!(workspace.canonical_items_dir, items_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_fs_dry_repo_root_adapter_items_dir_outside_repo_returns_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let (dir, _items_dir) = seed_repo();
        let outside = tempfile::tempdir().unwrap();
        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(dir.path()).expect("set_current_dir must succeed");

        let result = FsDryRepoRootAdapter.resolve(outside.path());

        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("items_dir"),
            "error must mention items_dir containment failure, got: {err}"
        );
    }

    #[test]
    fn test_fs_dry_repo_root_adapter_missing_items_dir_returns_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let (dir, items_dir) = seed_repo();
        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(dir.path()).expect("set_current_dir must succeed");

        let result = FsDryRepoRootAdapter.resolve(&items_dir.join("nonexistent"));

        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");
        assert!(result.is_err(), "a nonexistent items_dir must fail closed");
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_dry_repo_root_adapter_symlinked_items_dir_returns_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let (dir, _items_dir) = seed_repo();
        let link = dir.path().join("items-link");
        std::os::unix::fs::symlink(dir.path().join("track/items"), &link).unwrap();
        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(dir.path()).expect("set_current_dir must succeed");

        let result = FsDryRepoRootAdapter.resolve(&PathBuf::from("items-link"));

        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("symlink guard"),
            "symlinked items_dir must fail closed, got: {err}"
        );
    }

    // ── FsDryBaseBranchAdapter ──────────────────────────────────────────────

    #[test]
    fn test_fs_dry_base_branch_adapter_reads_metadata_json_snapshot() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":6,"id":"my-track-2026","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{"base_branch":"develop","merge_target":"develop","merge_method":"squash"}}"#,
        )
        .unwrap();

        let base_branch =
            FsDryBaseBranchAdapter.resolve_base_branch(root, &track_dir, &test_track_id()).unwrap();

        assert_eq!(base_branch, "develop");
    }

    #[test]
    fn test_fs_dry_base_branch_adapter_missing_metadata_returns_error() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();

        let err = FsDryBaseBranchAdapter
            .resolve_base_branch(root, &track_dir, &test_track_id())
            .unwrap_err();

        assert!(
            err.to_string().contains("metadata.json for 'my-track-2026' not found"),
            "missing metadata.json must fail closed with no global-config fallback, got: {err}"
        );
    }

    #[test]
    fn test_fs_dry_base_branch_adapter_malformed_metadata_returns_error() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), "not valid json").unwrap();

        let err = FsDryBaseBranchAdapter
            .resolve_base_branch(root, &track_dir, &test_track_id())
            .unwrap_err();

        assert!(
            err.to_string().contains("decode metadata.json"),
            "malformed metadata.json must fail closed, got: {err}"
        );
    }

    #[test]
    fn test_fs_dry_base_branch_adapter_old_schema_metadata_fails_closed() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":1,"id":"my-track-2026"}"#,
        )
        .unwrap();

        let err = FsDryBaseBranchAdapter
            .resolve_base_branch(root, &track_dir, &test_track_id())
            .unwrap_err();

        assert!(
            err.to_string().contains("decode metadata.json"),
            "pre-v6 metadata.json must fail closed (ADR D5: no backward compatibility), got: {err}"
        );
    }

    #[test]
    fn test_fs_dry_base_branch_adapter_out_of_repo_track_dir_returns_error() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = repo.path().canonicalize().unwrap();
        let outside_track_dir = outside.path().join("my-track-2026");
        std::fs::create_dir_all(&outside_track_dir).unwrap();
        std::fs::write(
            outside_track_dir.join("metadata.json"),
            r#"{"schema_version":6,"id":"my-track-2026","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{"base_branch":"evil","merge_target":"evil","merge_method":"squash"}}"#,
        )
        .unwrap();

        let err = FsDryBaseBranchAdapter
            .resolve_base_branch(&root, &outside_track_dir, &test_track_id())
            .unwrap_err();

        assert!(
            err.to_string().contains("outside repo root"),
            "out-of-repo metadata.json must fail closed before read, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_dry_base_branch_adapter_symlinked_metadata_returns_error() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();
        let target = root.join("metadata-target.json");
        std::fs::write(
            &target,
            r#"{"schema_version":6,"id":"my-track-2026","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{"base_branch":"main","merge_target":"main","merge_method":"squash"}}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, track_dir.join("metadata.json")).unwrap();

        let err = FsDryBaseBranchAdapter
            .resolve_base_branch(root, &track_dir, &test_track_id())
            .unwrap_err();

        assert!(
            err.to_string().contains("symlink guard"),
            "symlinked metadata.json must fail closed, got: {err}"
        );
    }

    // ── FsDryCheckStorageFactoryAdapter ─────────────────────────────────────

    #[test]
    fn test_fs_dry_check_storage_factory_adapter_builds_working_trio() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path().canonicalize().unwrap();
        let track_dir = root.join("track").join("items").join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();

        let handle = FsDryCheckStorageFactoryAdapter.build(&track_dir, &root);

        let records = handle.reader.read_records().unwrap();
        assert!(records.is_empty(), "a fresh store must have no records");
        let coverage = handle.coverage.read_coverage(&test_track_id()).unwrap();
        assert!(coverage.is_none(), "a fresh coverage manifest must be absent");
    }

    // ── FsDryDiffBaseFactoryAdapter ─────────────────────────────────────────

    #[test]
    fn test_fs_dry_diff_base_factory_adapter_build_is_infallible() {
        // Construction performs no I/O — building the port for an arbitrary
        // (non-existent-repo) base branch string must not panic.
        let _resolver = FsDryDiffBaseFactoryAdapter.build("main");
    }
}
