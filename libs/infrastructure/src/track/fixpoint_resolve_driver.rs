//! Infrastructure adapters implementing the four
//! `usecase::fixpoint_resolve_driver` secondary ports.
//!
//! Each adapter performs a single narrow I/O concern (ADR 2026-06-21-1328 D7 /
//! R1: `SecondaryAdapter` is infrastructure-only) — the orchestration itself
//! lives in [`usecase::fixpoint_resolve_driver::FixpointResolveDriverInteractor`].
//! This module is purely additive: nothing outside it references these
//! adapters yet.
//!
//! Design: ADR 2026-06-21-1328 D7, IN-12, AC-17, CN-03, CN-06.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::DryCheckConfigFingerprint;
use usecase::dry_check::{
    DryCheckApprovalService, DryCheckConfig, DryCheckParallelism, DryCheckPercent,
    DryFragmentPipelineInteractor,
};
use usecase::fixpoint_resolve::{FixpointDryGateInteractor, FixpointDryGateService};
use usecase::fixpoint_resolve_driver::{
    DryCheckConfigLoaderError, DryCheckConfigLoaderPort, FixpointDryGateFactoryPort,
    FixpointGateStateFactoryPort, FixpointWorkspaceContext, FixpointWorkspaceContextError,
    FixpointWorkspaceContextPort,
};

use crate::dry_check::approval_factory::FsDryApprovalFactoryAdapter;
use crate::dry_check::diff_base_resolver::FsDiffBaseResolverAdapter;
use crate::dry_check::noop_approval::NoOpDryApprovalService;
use crate::dry_check::{DryCheckConfig as InfraDryCheckConfig, FsDryCorpusMetaAdapter};
use crate::git_cli::{GitRepository as _, SystemGitRepo};
use crate::semantic_dup::CodeFragmentExtractorAdapter;
use crate::track::gate_state::{FsRefVerifyGateStateAdapter, FsReviewGateStateAdapter};
use crate::track::symlink_guard::reject_symlinks_below;

// ── Local helpers (reimplemented — see per-fn docs) ───────────────────────────

/// Resolves `<project-root>/track/items` → `<project-root>`.
///
/// Reimplemented locally: `libs/infrastructure` cannot depend on the
/// `cli_composition`-private `resolve_project_root` helper. Mirrors
/// `crate::track::gate_state`'s equivalent private helper.
fn resolve_project_root(items_dir: &Path) -> Result<PathBuf, String> {
    let items_name = items_dir.file_name().and_then(|n| n.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(Path::file_name).and_then(|n| n.to_str());
    let project_root = track_dir.and_then(Path::parent);
    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            if root.as_os_str().is_empty() {
                Ok(PathBuf::from("."))
            } else {
                Ok(root.to_path_buf())
            }
        }
        _ => Err(format!(
            "items_dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}

/// Read `base_branch` from `<canonical_items_dir>/<track_id>/metadata.json`.
///
/// Fail-closed: a missing `metadata.json` or any decode error maps to
/// [`FixpointWorkspaceContextError::Unavailable`] — there is no branch-name
/// fallback (IN-06/IN-07/CN-03/CN-06).
fn read_base_branch(
    canonical_items_dir: &Path,
    track_id: &TrackId,
) -> Result<String, FixpointWorkspaceContextError> {
    let metadata_path = canonical_items_dir.join(track_id.as_ref()).join("metadata.json");
    match reject_symlinks_below(&metadata_path, canonical_items_dir) {
        Ok(true) => {}
        Ok(false) => {
            return Err(FixpointWorkspaceContextError::Unavailable(format!(
                "read metadata.json for '{}': metadata.json is missing",
                track_id.as_ref()
            )));
        }
        Err(e) => {
            return Err(FixpointWorkspaceContextError::Unavailable(format!(
                "symlink guard metadata.json for '{}': {e}",
                track_id.as_ref()
            )));
        }
    }
    let metadata_json = std::fs::read_to_string(&metadata_path).map_err(|e| {
        FixpointWorkspaceContextError::Unavailable(format!(
            "read metadata.json for '{}': {e}",
            track_id.as_ref()
        ))
    })?;
    let (track_meta, _) = crate::track::codec::decode(&metadata_json).map_err(|e| {
        FixpointWorkspaceContextError::Unavailable(format!(
            "decode metadata.json for '{}': {e}",
            track_id.as_ref()
        ))
    })?;
    Ok(track_meta.branch_strategy_snapshot().base_branch().to_owned())
}

// ── FsFixpointWorkspaceContextAdapter ─────────────────────────────────────────

/// Filesystem/git adapter implementing [`FixpointWorkspaceContextPort`].
///
/// Reproduces the CWD-anchored repo discovery, `items_dir` canonicalization +
/// containment check, `canonical_root` derivation, and fail-closed
/// `base_branch` read previously inlined in the removed
/// `TrackCompositionRoot::fixpoint_resolve`.
pub struct FsFixpointWorkspaceContextAdapter;

impl FixpointWorkspaceContextPort for FsFixpointWorkspaceContextAdapter {
    fn resolve_context(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<FixpointWorkspaceContext, FixpointWorkspaceContextError> {
        let cwd_repo = SystemGitRepo::discover().map_err(|e| {
            FixpointWorkspaceContextError::Unavailable(format!("cannot discover git repo: {e}"))
        })?;
        let repo_root = cwd_repo.root().canonicalize().map_err(|e| {
            FixpointWorkspaceContextError::Unavailable(format!(
                "cannot canonicalize repo root: {e}"
            ))
        })?;

        let items_dir_abs = if items_dir.is_absolute() {
            items_dir.to_path_buf()
        } else {
            repo_root.join(items_dir)
        };
        reject_symlinks_below(&items_dir_abs, &repo_root).map_err(|e| {
            FixpointWorkspaceContextError::Unavailable(format!(
                "symlink guard: refusing to use --items-dir '{}': {e}",
                items_dir.display()
            ))
        })?;
        match items_dir_abs.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(FixpointWorkspaceContextError::Unavailable(format!(
                    "symlink guard: refusing to use symlinked --items-dir '{}'",
                    items_dir.display()
                )));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(FixpointWorkspaceContextError::Unavailable(format!(
                    "symlink guard: cannot stat --items-dir '{}': {e}",
                    items_dir.display()
                )));
            }
        }

        let canonical_items_dir = items_dir_abs.canonicalize().map_err(|_| {
            FixpointWorkspaceContextError::Unavailable(format!(
                "--items-dir '{}' must be an existing directory under the repository root",
                items_dir.display()
            ))
        })?;

        if !canonical_items_dir.starts_with(&repo_root) {
            return Err(FixpointWorkspaceContextError::Unavailable(format!(
                "--items-dir '{}' must be an existing directory under the repository root",
                items_dir.display()
            )));
        }

        let canonical_root = resolve_project_root(&canonical_items_dir)
            .and_then(|p| {
                p.canonicalize().map_err(|e| format!("cannot canonicalize project root: {e}"))
            })
            .map_err(|e| {
                FixpointWorkspaceContextError::Unavailable(format!(
                    "cannot derive project root from items_dir: {e}"
                ))
            })?;

        if !canonical_items_dir.is_dir() {
            return Err(FixpointWorkspaceContextError::Unavailable(format!(
                "--items-dir '{}' must be an existing directory under the repository root",
                items_dir.display()
            )));
        }

        let base_branch = read_base_branch(&canonical_items_dir, track_id)?;

        Ok(FixpointWorkspaceContext { repo_root, canonical_items_dir, canonical_root, base_branch })
    }
}

// ── FsDryCheckConfigLoaderAdapter ─────────────────────────────────────────────

/// Filesystem adapter implementing [`DryCheckConfigLoaderPort`].
///
/// Loads `.harness/config/dry-check.json` and lifts it into the usecase-level
/// [`DryCheckConfig`] newtypes, reproducing the field-by-field conversion
/// already implemented by `cli_composition::dry::dry_checker_config::build_usecase_dry_check_config`
/// (reimplemented locally since `libs/infrastructure` cannot depend on
/// `apps/cli-composition`).
pub struct FsDryCheckConfigLoaderAdapter;

impl DryCheckConfigLoaderPort for FsDryCheckConfigLoaderAdapter {
    fn load(
        &self,
        repo_root: &Path,
    ) -> Result<(DryCheckConfig, DryCheckConfigFingerprint), DryCheckConfigLoaderError> {
        let canonical_root = repo_root.canonicalize().map_err(|e| {
            DryCheckConfigLoaderError::Unavailable(format!(
                "failed to canonicalize repo root '{}': {e}",
                repo_root.display()
            ))
        })?;
        let dry_config_path = canonical_root.join(".harness/config/dry-check.json");
        let canonical_dry_config_path = dry_config_path.canonicalize().map_err(|e| {
            DryCheckConfigLoaderError::Unavailable(format!(
                "failed to canonicalize dry-check config '{}': {e}",
                dry_config_path.display()
            ))
        })?;
        if !canonical_dry_config_path.starts_with(&canonical_root) {
            return Err(DryCheckConfigLoaderError::Unavailable(format!(
                "dry-check config '{}' resolves outside repo root '{}'",
                dry_config_path.display(),
                canonical_root.display()
            )));
        }
        reject_symlinks_below(&dry_config_path, &canonical_root).map_err(|e| {
            DryCheckConfigLoaderError::Unavailable(format!(
                "symlink guard dry-check config '{}': {e}",
                dry_config_path.display()
            ))
        })?;
        let infra_config = InfraDryCheckConfig::load(&canonical_dry_config_path).map_err(|e| {
            DryCheckConfigLoaderError::Unavailable(format!("failed to load dry-check config: {e}"))
        })?;

        let percent = |v: u8| {
            DryCheckPercent::try_new(v).map_err(|e| {
                DryCheckConfigLoaderError::Unavailable(format!("invalid known-bad percent: {e}"))
            })
        };
        let usecase_config = DryCheckConfig::new(
            percent(infra_config.known_bad_injection_rate_percent())?,
            percent(infra_config.known_bad_detection_threshold_percent())?,
            DryCheckParallelism::try_new(infra_config.max_parallelism()).map_err(|e| {
                DryCheckConfigLoaderError::Unavailable(format!("invalid max_parallelism: {e}"))
            })?,
            infra_config.enabled(),
        );

        Ok((usecase_config, infra_config.fingerprint()))
    }
}

// ── FsFixpointDryGateFactoryAdapter ───────────────────────────────────────────

/// Factory adapter implementing [`FixpointDryGateFactoryPort`].
///
/// Reproduces unchanged the wiring previously done by the removed
/// `TrackCompositionRoot::make_dry_gate_interactor` helper.
pub struct FsFixpointDryGateFactoryAdapter;

impl FixpointDryGateFactoryPort for FsFixpointDryGateFactoryAdapter {
    fn build(&self, base_branch: &str) -> Arc<dyn FixpointDryGateService> {
        let diff_source = Arc::new(crate::dry_check::GitDryCheckDiffGetter);
        let extractor = Arc::new(CodeFragmentExtractorAdapter::new());
        let fragment_pipeline =
            Arc::new(DryFragmentPipelineInteractor::new(diff_source, extractor));
        Arc::new(FixpointDryGateInteractor::new(
            Arc::new(NoOpDryApprovalService) as Arc<dyn DryCheckApprovalService + Send + Sync>,
            Arc::new(FsDiffBaseResolverAdapter::new(base_branch.to_owned())),
            Arc::new(FsDryCorpusMetaAdapter),
            fragment_pipeline,
            Arc::new(FsDryApprovalFactoryAdapter),
        ))
    }
}

// ── FsFixpointGateStateFactoryAdapter ─────────────────────────────────────────

/// Factory adapter implementing [`FixpointGateStateFactoryPort`].
///
/// Both gate-state adapters were previously constructed inline in the removed
/// `TrackCompositionRoot::fixpoint_resolve`.
pub struct FsFixpointGateStateFactoryAdapter;

impl FixpointGateStateFactoryPort for FsFixpointGateStateFactoryAdapter {
    fn build_review_gate(
        &self,
        items_dir: &Path,
        base_branch: &str,
    ) -> Arc<dyn usecase::fixpoint_resolve::ReviewGateStatePort> {
        Arc::new(FsReviewGateStateAdapter::new(items_dir.to_path_buf(), base_branch.to_owned()))
    }

    fn build_ref_verify_gate(
        &self,
        items_dir: &Path,
    ) -> Arc<dyn usecase::fixpoint_resolve::RefVerifyGateStatePort> {
        Arc::new(FsRefVerifyGateStateAdapter::new(items_dir.to_path_buf()))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::process::Command;
    use std::sync::Mutex;

    use usecase::fixpoint_resolve_driver::{
        FixpointResolveDriverInput, FixpointResolveDriverInteractor, FixpointResolveDriverOutcome,
        FixpointResolveDriverService as _,
    };

    use super::*;

    /// Serializes tests that rely on `SystemGitRepo::discover()`'s CWD-implicit
    /// behavior (directly, or via a CWD change performed by the test itself).
    /// Mirrors `crate::dry_check::diff_getter`'s local `CWD_LOCK` pattern.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn make_real_interactor() -> FixpointResolveDriverInteractor {
        FixpointResolveDriverInteractor::new(
            Arc::new(FsFixpointWorkspaceContextAdapter),
            Arc::new(FsDryCheckConfigLoaderAdapter),
            Arc::new(FsFixpointDryGateFactoryAdapter),
            Arc::new(FsFixpointGateStateFactoryAdapter),
        )
    }

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

    /// Create a minimal git repo with an initial commit on `main` then switch
    /// to `track/<id>`. Returns `(tempdir, items_dir)`.
    fn seed_track_repo(track_id: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);
        run_git(root, &["checkout", "-b", &format!("track/{track_id}")]);
        let items_dir = root.join("track").join("items");
        std::fs::create_dir_all(items_dir.join(track_id)).unwrap();
        (dir, items_dir)
    }

    // ── Relocated from apps/cli-composition/src/track/fixpoint_resolve.rs ─────
    // (the three wiring-path tests that need real git repos / real filesystem /
    // real dry-gate wiring)

    /// `--items-dir` must be inside the discovered repository root.
    #[test]
    fn test_fixpoint_resolve_items_dir_outside_repo_returns_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let (dir, _items_dir) = seed_track_repo("my-track-2026");
        let outside = tempfile::tempdir().unwrap();
        let outside_items = outside.path().join("track").join("items");
        std::fs::create_dir_all(&outside_items).unwrap();

        let outcome = make_real_interactor().fixpoint_resolve(FixpointResolveDriverInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "track/my-track-2026".to_owned(),
            items_dir: outside_items,
        });
        drop(dir);

        match outcome {
            FixpointResolveDriverOutcome::Failure { message } => {
                assert!(
                    message.contains("items_dir")
                        || message.contains("items-dir")
                        || message.contains("cannot discover git repo")
                        || message.contains("cannot canonicalize"),
                    "error must mention items_dir containment failure, got: {message}"
                );
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    /// Passing a regular file (not a directory) as `--items-dir` must return a
    /// directory-constraint `Failure`.
    #[test]
    fn test_fixpoint_resolve_items_dir_is_file_returns_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);
        run_git(root, &["checkout", "-b", "track/my-track-2026"]);

        let track_dir = root.join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let file_path = track_dir.join("items");
        std::fs::write(&file_path, "not a directory").unwrap();

        let outcome = make_real_interactor().fixpoint_resolve(FixpointResolveDriverInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "track/my-track-2026".to_owned(),
            items_dir: file_path,
        });
        drop(dir);

        match outcome {
            FixpointResolveDriverOutcome::Failure { message } => {
                assert!(
                    message.contains("directory")
                        || message.contains("items_dir")
                        || message.contains("items-dir"),
                    "error must mention directory constraint, got: {message}"
                );
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    /// A symlinked `--items-dir` must be rejected before canonicalization can
    /// redirect the adapter into a different tree.
    #[cfg(unix)]
    #[test]
    fn test_fixpoint_resolve_symlinked_items_dir_returns_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);
        run_git(root, &["checkout", "-b", "track/my-track-2026"]);

        let real_items = root.join("redirect").join("track").join("items");
        std::fs::create_dir_all(&real_items).unwrap();
        let track_dir = root.join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::os::unix::fs::symlink(&real_items, track_dir.join("items")).unwrap();

        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(root).expect("set_current_dir to temp repo must succeed");
        let outcome = make_real_interactor().fixpoint_resolve(FixpointResolveDriverInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "track/my-track-2026".to_owned(),
            items_dir: PathBuf::from("track/items"),
        });
        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");
        drop(dir);

        match outcome {
            FixpointResolveDriverOutcome::Failure { message } => {
                assert!(
                    message.contains("symlink guard"),
                    "error must mention symlink guard, got: {message}"
                );
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    /// A symlinked `metadata.json` must not be followed when resolving the
    /// branch-strategy base branch.
    #[cfg(unix)]
    #[test]
    fn test_fixpoint_resolve_symlinked_metadata_json_returns_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let (dir, items_dir) = seed_track_repo("my-track-2026");
        let root = dir.path();
        let track_dir = items_dir.join("my-track-2026");
        let target = root.join("metadata-target.json");
        std::fs::write(
            &target,
            r#"{"schema_version":6,"id":"my-track-2026","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{"base_branch":"main","merge_target":"main","merge_method":"squash"}}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, track_dir.join("metadata.json")).unwrap();

        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(root).expect("set_current_dir to temp repo must succeed");
        let outcome = make_real_interactor().fixpoint_resolve(FixpointResolveDriverInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "track/my-track-2026".to_owned(),
            items_dir: items_dir.clone(),
        });
        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");
        drop(dir);

        match outcome {
            FixpointResolveDriverOutcome::Failure { message } => {
                assert!(
                    message.contains("symlink guard"),
                    "error must mention symlink guard, got: {message}"
                );
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    /// A symlinked dry-check config must be rejected before the config loader
    /// reads through it.
    #[cfg(unix)]
    #[test]
    fn test_dry_check_config_loader_symlinked_config_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        let harness_config_dir = root.join(".harness").join("config");
        std::fs::create_dir_all(&harness_config_dir).unwrap();

        let real_config = harness_config_dir.join("real-dry-check.json");
        std::fs::write(
            &real_config,
            r#"{
  "schema_version": 4,
  "enabled": false,
  "threshold": 0.85,
  "max_parallelism": 4,
  "known_bad_injection_rate_percent": 10,
  "known_bad_detection_threshold_percent": 90
}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(&real_config, harness_config_dir.join("dry-check.json"))
            .unwrap();

        let err = FsDryCheckConfigLoaderAdapter.load(root).unwrap_err();

        assert!(
            err.to_string().contains("symlink guard"),
            "error must reject symlinked dry-check config, got: {err}"
        );
    }

    /// When the dry gate is `enabled: true` and the coverage record is absent,
    /// the dry gate must be `Blocked` and the outcome must be `RunDfp`.
    #[test]
    fn test_fixpoint_resolve_missing_coverage_record_with_enabled_true_returns_run_dfp() {
        let _lock = CWD_LOCK.lock().unwrap();

        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);

        let track_id_str = "dfp-track-2026";
        run_git(root, &["checkout", "-b", &format!("track/{track_id_str}")]);

        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join(track_id_str);
        std::fs::create_dir_all(&track_dir).unwrap();

        let head_sha_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .expect("git rev-parse HEAD must succeed");
        let head_sha = String::from_utf8_lossy(&head_sha_output.stdout).trim().to_owned();
        std::fs::write(track_dir.join(".commit_hash"), &head_sha).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{"schema_version":6,"id":"{track_id_str}","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{{"base_branch":"main","merge_target":"main","merge_method":"squash"}}}}"#
            ),
        )
        .unwrap();

        let harness_config_dir = root.join(".harness").join("config");
        std::fs::create_dir_all(&harness_config_dir).unwrap();
        std::fs::write(
            harness_config_dir.join("dry-check.json"),
            r#"{
  "schema_version": 4,
  "enabled": true,
  "threshold": 0.85,
  "max_parallelism": 4,
  "known_bad_injection_rate_percent": 10,
  "known_bad_detection_threshold_percent": 90
}"#,
        )
        .unwrap();

        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(root).expect("set_current_dir to temp repo must succeed");

        let outcome = make_real_interactor().fixpoint_resolve(FixpointResolveDriverInput {
            track_id: track_id_str.to_owned(),
            current_branch: format!("track/{track_id_str}"),
            items_dir: items_dir.clone(),
        });

        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");
        drop(dir);

        assert!(
            matches!(outcome, FixpointResolveDriverOutcome::RunDfp),
            "enabled=true + missing coverage record must yield RunDfp, got {outcome:?}"
        );
    }
}
