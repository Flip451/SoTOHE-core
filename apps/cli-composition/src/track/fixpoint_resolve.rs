//! Fixpoint resolution composition: adapters + `CliApp::fixpoint_resolve`.
//!
//! Wires `FsReviewGateStateAdapter`, `FsRefVerifyGateStateAdapter` (secondary
//! ports for the fixpoint resolver), and the D4 dry-gate port adapters
//! (`FsDiffBaseResolverAdapter`, `FsDryCorpusMetaAdapter`,
//! `FsDryApprovalFactoryAdapter`) that implement the three secondary ports
//! required by `FixpointDryGateInteractor` (T008).
//!
//! Design: D2 / D4 / IN-02 / AC-03 / CN-02 / CN-07.

use std::path::PathBuf;
use std::sync::Arc;

use domain::CommitHash;
use domain::TrackId;
use domain::dry_check::{DryCheckConfigFingerprint, DryCheckCorpusFingerprint};
use infrastructure::dry_check::GitDryCheckDiffGetter;
use infrastructure::dry_check::{
    DryCheckConfig as InfraDryCheckConfig, FsDryCheckCoverageAdapter, FsDryCheckStore,
    FsDryCorpusMetaAdapter,
};
use infrastructure::semantic_dup::CodeFragmentExtractorAdapter;
use usecase::dry_check::{
    DryCheckApprovalInteractor, DryCheckApprovalService, DryFragmentPipelineInteractor,
};
use usecase::fixpoint_resolve::{
    DiffBaseResolverPort, DryApprovalFactoryPort, FixpointDryGateCommand,
    FixpointDryGateInteractor, FixpointDryGateService as _, FixpointResolveCommand,
    FixpointResolveInteractor, FixpointResolveService as _, RefVerifyGateStatePort,
    ReviewGateStatePort,
};
// Domain / usecase types used internally by this composition module and
// its in-crate unit tests. They are intentionally NOT re-exported through
// the `cli_composition` public surface — `apps/cli` consumes only DTOs and
// primitives across the CN-02 boundary, so the format-shape contract and
// the domain-validation contract are tested here, not in `apps/cli`.
use domain::track_phase::FixpointStep;
use usecase::dry_check::DryCheckConfig;
use usecase::fixpoint_resolve::{FixpointCurrentBranch, FixpointResolveError};

use crate::error::CompositionError;
use crate::{CliApp, CommandOutcome};

use super::resolve_project_root;

// ── NoOpDryApprovalService ────────────────────────────────────────────────────
// Re-export shim: implementation relocated to `libs/infrastructure` per ADR 1328 D7.
use infrastructure::dry_check::noop_approval::NoOpDryApprovalService;

// ── FsDiffBaseResolverAdapter ─────────────────────────────────────────────────

/// Infrastructure adapter implementing [`DiffBaseResolverPort`].
///
/// Wraps `crate::dry::resolve_dry_diff_base_from_store` which reads
/// `.commit_hash` from the track directory and falls back to
/// `git rev-parse main`. Stateless unit struct.
struct FsDiffBaseResolverAdapter;

impl DiffBaseResolverPort for FsDiffBaseResolverAdapter {
    fn resolve_diff_base(
        &self,
        track_dir: &std::path::Path,
        canonical_root: &std::path::Path,
        repo_root: &std::path::Path,
    ) -> Result<CommitHash, String> {
        let commit_hash_path = track_dir.join(".commit_hash");
        crate::dry::resolve_dry_diff_base_from_store(
            &commit_hash_path,
            canonical_root,
            Some(repo_root),
            "fixpoint-resolve",
        )
    }
}

// ── FsDryApprovalFactoryAdapter ───────────────────────────────────────────────

/// Infrastructure adapter implementing [`DryApprovalFactoryPort`].
///
/// Constructs a [`DryCheckApprovalInteractor`] from the injected
/// infrastructure-layer config (used for `current_config_fingerprint`) and the
/// resolved corpus metadata.
struct FsDryApprovalFactoryAdapter;

impl DryApprovalFactoryPort for FsDryApprovalFactoryAdapter {
    fn build_approval(
        &self,
        track_dir: &std::path::Path,
        canonical_root: &std::path::Path,
        dry_config: DryCheckConfig,
        config_fingerprint: DryCheckConfigFingerprint,
        corpus_fingerprint: DryCheckCorpusFingerprint,
    ) -> Arc<dyn DryCheckApprovalService + Send + Sync> {
        let dry_check_json_path = track_dir.join("dry-check.json");
        let dry_check_coverage_path = track_dir.join("dry-check-coverage.json");
        let store =
            Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root.to_path_buf()));
        let coverage = Arc::new(FsDryCheckCoverageAdapter::new(
            dry_check_coverage_path,
            canonical_root.to_path_buf(),
        ));
        Arc::new(DryCheckApprovalInteractor::new(
            dry_config,
            store,
            coverage,
            config_fingerprint,
            corpus_fingerprint,
        )) as Arc<dyn DryCheckApprovalService + Send + Sync>
    }
}

// ── FixpointResolveInput ──────────────────────────────────────────────────────

/// Input DTO for `sotp track fixpoint-resolve`.
///
/// Carries the three string-typed values that the CLI layer passes across the
/// composition boundary (CN-02 / AC-03).
#[derive(Debug, Clone)]
pub struct FixpointResolveInput {
    /// Active track ID (directory name under `items_dir/<id>`).
    pub track_id: String,
    /// Current git branch label (e.g. `"track/my-feature-2026"`).
    pub current_branch: String,
    /// Path to `track/items` directory.
    pub items_dir: PathBuf,
}

// ── Gate-state adapters (re-exported from infrastructure) ────────────────────
//
// Both adapter structs were relocated to `libs/infrastructure/src/track/gate_state.rs`
// per ADR 1328 D7 (port-impl adapters belong in libs/infrastructure).
// This re-export shim keeps existing wiring call sites unchanged.
pub(crate) use infrastructure::track::gate_state::{
    FsRefVerifyGateStateAdapter, FsReviewGateStateAdapter,
};

// ── Dry-gate helper ───────────────────────────────────────────────────────────

/// Build the [`FixpointDryGateInteractor`] wired with all infrastructure adapters.
fn make_dry_gate_interactor() -> FixpointDryGateInteractor {
    let diff_source = Arc::new(GitDryCheckDiffGetter);
    let extractor = Arc::new(CodeFragmentExtractorAdapter::new());
    let fragment_pipeline = Arc::new(DryFragmentPipelineInteractor::new(diff_source, extractor));
    FixpointDryGateInteractor::new(
        Arc::new(NoOpDryApprovalService) as Arc<dyn DryCheckApprovalService + Send + Sync>,
        Arc::new(FsDiffBaseResolverAdapter),
        Arc::new(FsDryCorpusMetaAdapter),
        fragment_pipeline,
        Arc::new(FsDryApprovalFactoryAdapter),
    )
}

// ── Format helpers ────────────────────────────────────────────────────────────

/// Format a [`FixpointStep`] as the canonical output string.
///
/// Output contracts:
/// - `RunDfp` → `"run-dfp"`
/// - `RunRfp { scopes }` → `"run-rfp scopes=<s1>,<s2>..."` (BTreeSet iteration order)
/// - `RunRefVerify` → `"run-ref-verify"`
/// - `Commit` → `"commit"`
pub(crate) fn format_fixpoint_step(step: FixpointStep) -> String {
    match step {
        FixpointStep::RunDfp => "run-dfp".to_owned(),
        FixpointStep::RunRfp { scopes } => {
            let joined = scopes.as_set().iter().map(String::as_str).collect::<Vec<_>>().join(",");
            format!("run-rfp scopes={joined}")
        }
        FixpointStep::RunRefVerify => "run-ref-verify".to_owned(),
        FixpointStep::Commit => "commit".to_owned(),
    }
}

// ── CliApp::fixpoint_resolve ──────────────────────────────────────────────────

impl CliApp {
    /// Resolve the next fixpoint step for the active track.
    ///
    /// Returns a [`CommandOutcome`] whose `stdout` contains one of:
    /// - `"run-dfp"` — DRY gate is open.
    /// - `"run-rfp scopes=<s1>,<s2>..."` — one or more review scopes are stale.
    /// - `"run-ref-verify"` — ref-verify gate is blocked.
    /// - `"commit"` — all gates are green.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - `track_id` is invalid.
    /// - `current_branch` is empty or does not match `"track/<track_id>"`.
    /// - git repo discovery or diff base resolution fails.
    /// - Any gate adapter returns an error.
    pub fn fixpoint_resolve(
        &self,
        input: FixpointResolveInput,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::SystemGitRepo;

        // ── Validate inputs ───────────────────────────────────────────────────
        super::validate_track_id_str(&input.track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))?;

        let track_id = TrackId::try_new(input.track_id.clone())
            .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;

        let current_branch =
            FixpointCurrentBranch::try_new(input.current_branch.clone()).map_err(|e| {
                CompositionError::WiringFailed(format!("invalid --current-branch: {e}"))
            })?;

        // ── Track-not-active guard ────────────────────────────────────────────
        let expected_branch = format!("track/{}", track_id.as_ref());
        if current_branch.as_str() != expected_branch {
            return Err(CompositionError::WiringFailed(
                FixpointResolveError::TrackNotActive { branch: expected_branch }.to_string(),
            ));
        }

        // ── Resolve items_dir and discover git repo ───────────────────────────
        // Always discover the caller's git repo from CWD first so that `repo_root`
        // is anchored to the caller's checkout, not to whatever repo happens to
        // contain the (possibly absolute) --items-dir path.  This is the same
        // policy used by all other track commands (`resolve_existing_dir_under_repo`
        // in `dry/shared.rs`): CWD-discovery is the trust anchor, not path-based
        // discovery from an untrusted input argument.
        let cwd_repo = SystemGitRepo::discover()
            .map_err(|e| CompositionError::AdapterInit(format!("cannot discover git repo: {e}")))?;
        let repo_root = {
            use infrastructure::git_cli::GitRepository as _;
            cwd_repo.root().canonicalize().map_err(|e| {
                CompositionError::Infrastructure(format!("cannot canonicalize repo root: {e}"))
            })?
        };

        // Resolve the canonical items_dir path:
        // - Absolute path: canonicalize directly.
        // - Relative path: anchor to `repo_root` first (consistent with
        //   `resolve_existing_dir_under_repo`) then canonicalize.
        let canonical_items_dir = if input.items_dir.is_absolute() {
            input.items_dir.canonicalize().map_err(|_| {
                CompositionError::WiringFailed(format!(
                    "--items-dir '{}' must be an existing directory under the repository root",
                    input.items_dir.display()
                ))
            })?
        } else {
            repo_root.join(&input.items_dir).canonicalize().map_err(|_| {
                CompositionError::WiringFailed(format!(
                    "--items-dir '{}' must be an existing directory under the repository root",
                    input.items_dir.display()
                ))
            })?
        };

        // Validate that canonical_items_dir lies within the caller's repo root.
        // This must be checked against the CWD-discovered `repo_root`, not against
        // a repo discovered from `canonical_root` (which could be a different checkout).
        if !canonical_items_dir.starts_with(&repo_root) {
            return Err(CompositionError::WiringFailed(format!(
                "--items-dir '{}' must be an existing directory under the repository root",
                input.items_dir.display()
            )));
        }

        // Derive canonical_root (project root = parent of track/) from the
        // canonical items_dir.  This is correct regardless of whether --items-dir
        // was absolute or relative.
        let canonical_root = resolve_project_root(&canonical_items_dir)
            .and_then(|p| {
                p.canonicalize().map_err(|e| format!("cannot canonicalize project root: {e}"))
            })
            .map_err(|e| {
                CompositionError::WiringFailed(format!(
                    "cannot derive project root from items_dir: {e}"
                ))
            })?;

        // Verify items_dir is actually a directory (not a file or missing).
        if !canonical_items_dir.is_dir() {
            return Err(CompositionError::WiringFailed(format!(
                "--items-dir '{}' must be an existing directory under the repository root",
                input.items_dir.display()
            )));
        }

        // ── Load DRY gate config early (T008) ────────────────────────────────
        // Load the usecase dry config before dry prep so `enabled=false` can skip all
        // dry diff-base resolution, corpus fingerprinting, and fragment-ref construction.
        let dry_config_path = repo_root.join(".harness/config/dry-check.json");
        let dry_infra_config = InfraDryCheckConfig::load(&dry_config_path).map_err(|e| {
            CompositionError::ConfigLoad(format!("failed to load dry-check config: {e}"))
        })?;
        let usecase_dry_config = crate::dry::build_usecase_dry_check_config_pub(&dry_infra_config)
            .map_err(CompositionError::ConfigLoad)?;

        let track_dir = canonical_items_dir.join(track_id.as_ref());

        // ── Build dry-gate dependencies via D4 FixpointDryGateInteractor ─────
        //
        // CWD guard: set CWD = repo_root before the dry-gate call so that
        // GitDryCheckDiffGetter (which uses SystemGitRepo::discover()) anchors
        // to the caller's repository. Restored after the call regardless of
        // success or failure (CN-07 / usecase purity: CWD guard stays here,
        // not inside the usecase interactor).
        let dry_gate = make_dry_gate_interactor();
        let current_config_fingerprint = dry_infra_config.fingerprint();

        let original_cwd = std::env::current_dir().map_err(|e| {
            CompositionError::Infrastructure(format!("cannot read current directory: {e}"))
        })?;
        std::env::set_current_dir(&repo_root).map_err(|e| {
            CompositionError::Infrastructure(format!(
                "cannot enter repo root '{}': {e}",
                repo_root.display()
            ))
        })?;

        let dry_gate_result = dry_gate.resolve_dry_gate(FixpointDryGateCommand {
            track_dir: track_dir.clone(),
            canonical_root: canonical_root.clone(),
            repo_root: repo_root.clone(),
            dry_config: usecase_dry_config.clone(),
            current_config_fingerprint,
        });

        if let Err(e) = std::env::set_current_dir(&original_cwd) {
            eprintln!("[warn] fixpoint-resolve: failed to restore CWD after dry-gate: {e}");
        }

        let dry_gate_output = dry_gate_result.map_err(|e| {
            CompositionError::Usecase(format!("fixpoint-resolve dry-gate failed: {e}"))
        })?;

        let current_fragment_refs = dry_gate_output.current_fragment_refs;
        let dry_approval = dry_gate_output.dry_approval;

        // ── Build review and ref-verify gate adapters ─────────────────────────
        // Use the canonical (absolute, validated) items_dir so that these adapters
        // resolve track artifacts correctly regardless of the caller's CWD.
        let review_state = Arc::new(FsReviewGateStateAdapter::new(canonical_items_dir.clone()))
            as Arc<dyn ReviewGateStatePort>;
        let ref_verify_results =
            Arc::new(FsRefVerifyGateStateAdapter::new(canonical_items_dir.clone()))
                as Arc<dyn RefVerifyGateStatePort>;

        // ── Construct and run the interactor ──────────────────────────────────
        let interactor = FixpointResolveInteractor::new(
            usecase_dry_config,
            dry_approval,
            review_state,
            ref_verify_results,
        );

        let cmd = FixpointResolveCommand { track_id, current_branch, current_fragment_refs };

        let step = interactor
            .resolve(&cmd)
            .map_err(|e| CompositionError::Usecase(format!("fixpoint-resolve failed: {e}")))?;

        Ok(CommandOutcome::success(Some(format_fixpoint_step(step))))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;

    // Status types used in test assertions — no longer re-exported from the
    // parent module's use list after the adapter relocation, so imported directly.
    use domain::track_phase::ReviewScopeSet;

    // ── format_fixpoint_step tests ────────────────────────────────────────────
    //
    // These live in `cli-composition` (not in `apps/cli`) so that
    // `format_fixpoint_step` callers do not need to import `domain` /
    // `usecase` types across the CN-02 boundary. `apps/cli` tests only
    // exercise the clap argument-parsing surface and the dispatch shim;
    // the output-shape contract is owned here.

    #[test]
    fn test_format_fixpoint_step_run_dfp() {
        assert_eq!(format_fixpoint_step(FixpointStep::RunDfp), "run-dfp");
    }

    #[test]
    fn test_format_fixpoint_step_run_rfp_single_scope() {
        let mut set = BTreeSet::new();
        set.insert("plan-artifacts".to_owned());
        let scopes = ReviewScopeSet::try_new(set).unwrap();
        assert_eq!(
            format_fixpoint_step(FixpointStep::RunRfp { scopes }),
            "run-rfp scopes=plan-artifacts"
        );
    }

    #[test]
    fn test_format_fixpoint_step_run_rfp_multiple_scopes_in_btreeset_order() {
        let mut set = BTreeSet::new();
        set.insert("code".to_owned());
        set.insert("plan-artifacts".to_owned());
        let scopes = ReviewScopeSet::try_new(set).unwrap();
        // "code" < "plan-artifacts" in BTreeSet order.
        assert_eq!(
            format_fixpoint_step(FixpointStep::RunRfp { scopes }),
            "run-rfp scopes=code,plan-artifacts"
        );
    }

    #[test]
    fn test_format_fixpoint_step_run_ref_verify() {
        assert_eq!(format_fixpoint_step(FixpointStep::RunRefVerify), "run-ref-verify");
    }

    #[test]
    fn test_format_fixpoint_step_commit() {
        assert_eq!(format_fixpoint_step(FixpointStep::Commit), "commit");
    }

    // FixpointCurrentBranch validation (empty-string rejection, etc.) is
    // tested in the usecase layer where the type is defined
    // (`libs/usecase/src/fixpoint_resolve.rs`). Re-asserting it here would
    // duplicate that contract; the composition layer only owns the
    // formatting-shape contract above.

    // ── Test helpers ──────────────────────────────────────────────────────────

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
        assert!(status.success(), "git {:?} failed with {status}", args);
    }

    /// Create a minimal git repo with an initial commit on `main` then switch
    /// to `track/<id>`.  Seeding `main` first ensures `git rev-parse main`
    /// succeeds inside the diff-base fallback path.  Returns `(tempdir, items_dir)`.
    fn seed_track_repo(track_id: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);
        // Create the track branch from main.
        run_git(root, &["checkout", "-b", &format!("track/{track_id}")]);
        let items_dir = root.join("track").join("items");
        std::fs::create_dir_all(items_dir.join(track_id)).unwrap();
        (dir, items_dir)
    }

    // ── items_dir containment tests ───────────────────────────────────────────

    /// `--items-dir` must be inside the discovered repository root.
    /// When an absolute path ending in `track/items` is given but points
    /// outside the repo tree, the method must return `Err`.
    #[test]
    fn test_fixpoint_resolve_items_dir_outside_repo_returns_error() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let (dir, _items_dir) = seed_track_repo("my-track-2026");
        // Use a completely unrelated temp dir (not inside the repo).
        let outside = tempfile::tempdir().unwrap();
        let outside_items = outside.path().join("track").join("items");
        std::fs::create_dir_all(&outside_items).unwrap();

        let result = CliApp::new().fixpoint_resolve(FixpointResolveInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "track/my-track-2026".to_owned(),
            items_dir: outside_items,
        });
        drop(dir);

        assert!(result.is_err(), "expected Err when items_dir is outside the repo");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("items_dir")
                || msg.contains("items-dir")
                || msg.contains("cannot discover git repo")
                || msg.contains("cannot canonicalize"),
            "error must mention items_dir containment failure, got: {msg}"
        );
    }

    // ── TrackNotActive guard tests ─────────────────────────────────────────────

    /// When `current_branch` does not match `"track/<track_id>"`, the method
    /// must return an error without touching the gate adapters.
    #[test]
    fn test_fixpoint_resolve_wrong_branch_returns_track_not_active_error() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let (dir, items_dir) = seed_track_repo("my-track-2026");

        let result = CliApp::new().fixpoint_resolve(FixpointResolveInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "main".to_owned(),
            items_dir,
        });
        drop(dir);

        assert!(result.is_err(), "expected Err when branch does not match track");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not active")
                || msg.contains("TrackNotActive")
                || msg.contains("track/my-track-2026"),
            "error must mention the expected branch, got: {msg}"
        );
    }

    /// An empty `--current-branch` must be rejected before reaching the gate adapters.
    #[test]
    fn test_fixpoint_resolve_empty_current_branch_returns_invalid_current_branch_error() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let (dir, items_dir) = seed_track_repo("my-track-2026");

        let result = CliApp::new().fixpoint_resolve(FixpointResolveInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "".to_owned(),
            items_dir,
        });
        drop(dir);

        assert!(result.is_err(), "expected Err for empty current_branch");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("current-branch") || msg.contains("InvalidCurrentBranch"),
            "error must mention the invalid branch, got: {msg}"
        );
    }

    /// An invalid `--track-id` (empty string) must be rejected immediately.
    #[test]
    fn test_fixpoint_resolve_invalid_track_id_returns_error() {
        let result = CliApp::new().fixpoint_resolve(FixpointResolveInput {
            track_id: "".to_owned(),
            current_branch: "track/x".to_owned(),
            items_dir: PathBuf::from("track/items"),
        });

        assert!(result.is_err(), "expected Err for empty track_id");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("track-id") || msg.contains("track id"),
            "error must mention the invalid track-id, got: {msg}"
        );
    }

    // ── Dry gate blocked → "run-dfp" ─────────────────────────────────────────

    /// When the dry gate is `enabled: true` and the coverage record is absent,
    /// the dry gate must be `Blocked` and the step must be `RunDfp`.
    ///
    /// Uses an isolated temp git repository (not the workspace) so that the
    /// `.harness/config/dry-check.json` read by `fixpoint_resolve` (`repo_root`-
    /// anchored) is under our control and can be set to `enabled: true`.
    ///
    /// CWD is temporarily changed to the temp repo root so that `SystemGitRepo::discover()`
    /// picks up the correct repo and its config.
    #[test]
    fn test_fixpoint_resolve_missing_coverage_record_with_enabled_true_returns_run_dfp() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();

        // Create a self-contained git repo with the full structure needed:
        //   <root>/
        //     .git/
        //     .harness/config/dry-check.json  (enabled: true)
        //     track/items/<track_id>/
        //       .commit_hash
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

        // Write the HEAD SHA to .commit_hash so diff-base resolution succeeds.
        let head_sha_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .expect("git rev-parse HEAD must succeed");
        let head_sha = String::from_utf8_lossy(&head_sha_output.stdout).trim().to_owned();
        std::fs::write(track_dir.join(".commit_hash"), &head_sha).unwrap();

        // Write `.harness/config/dry-check.json` with `enabled: true` so the dry
        // gate runs (rather than bypassing via the enabled=false short-circuit).
        // `fixpoint_resolve` reads config from `repo_root.join(".harness/config/dry-check.json")`,
        // where `repo_root` is CWD-discovered; we switch CWD to `root` below.
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

        // Temporarily change CWD to the temp repo root so SystemGitRepo::discover() finds
        // this repo (not the workspace) and loads config from the fixture harness dir.
        let original_cwd = std::env::current_dir().expect("current_dir must succeed");
        std::env::set_current_dir(root).expect("set_current_dir to temp repo must succeed");

        // No dry-check-coverage.json → dry gate is Blocked → step = "run-dfp".
        let outcome = CliApp::new().fixpoint_resolve(FixpointResolveInput {
            track_id: track_id_str.to_owned(),
            current_branch: format!("track/{track_id_str}"),
            items_dir: items_dir.clone(),
        });

        // Restore CWD before any assertions that might fail.
        std::env::set_current_dir(&original_cwd).expect("restore CWD must succeed");

        let outcome = outcome
            .expect("fixpoint-resolve with enabled=true + missing coverage must succeed (not Err)");
        drop(dir);

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("run-dfp"),
            "enabled=true + missing coverage record must yield run-dfp"
        );
    }

    // ── items_dir is-dir guard ────────────────────────────────────────────────

    /// Passing a regular file (not a directory) as `--items-dir` that lies inside the
    /// repo must return `Err` after the containment check passes but the `is_dir` check
    /// fires.
    ///
    /// Uses a fresh git repo (separate from `seed_track_repo`) so that `track/items`
    /// does not yet exist as a directory — we can create a regular file there instead.
    #[test]
    fn test_fixpoint_resolve_items_dir_is_file_returns_error() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        // Create a new, minimal git repo without the standard items directory.
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);
        run_git(root, &["checkout", "-b", "track/my-track-2026"]);

        // Create `track/` as a directory but write `track/items` as a regular file
        // (not a directory), so `is_dir()` fails on it.
        let track_dir = root.join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let file_path = track_dir.join("items");
        std::fs::write(&file_path, "not a directory").unwrap();

        let result = CliApp::new().fixpoint_resolve(FixpointResolveInput {
            track_id: "my-track-2026".to_owned(),
            current_branch: "track/my-track-2026".to_owned(),
            items_dir: file_path,
        });
        drop(dir);

        assert!(result.is_err(), "expected Err when items_dir is a file");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("directory") || msg.contains("items_dir") || msg.contains("items-dir"),
            "error must mention directory constraint, got: {msg}"
        );
    }

    // ── FsReviewGateStateAdapter / FsRefVerifyGateStateAdapter unit tests ─────
    // The adapters were relocated to `libs/infrastructure/src/track/gate_state.rs`
    // in T002 (D7 adapter relocation). Their unit tests now live alongside the
    // struct definitions in that infrastructure module. This module retains only
    // the wiring path tests (see above) to verify the re-export shim resolves
    // correctly and the adapters compose into FixpointResolveInteractor.
}
