//! Fixpoint resolution composition: adapters + `CliApp::fixpoint_resolve`.
//!
//! Wires `FsReviewGateStateAdapter` and `FsRefVerifyGateStateAdapter` (secondary
//! ports for the fixpoint resolver) plus a thin `CliApp` entry point that
//! constructs `FixpointResolveInteractor` from injected adapters and returns the
//! next required step as a plain string.
//!
//! Design: D2 / IN-02 / AC-03 / CN-02.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::FragmentRef;
use infrastructure::dry_check::{FsDryCheckCoverageAdapter, FsDryCheckStore};
use usecase::dry_check::{DryCheckApprovalInteractor, fragment_ref_of};
use usecase::fixpoint_resolve::{
    FixpointResolveCommand, FixpointResolveInteractor, FixpointResolveService as _,
    RefVerifyGateStatePort, RefVerifyGateStatus, ReviewGateStatePort, ReviewGateStatus,
};
use usecase::review_v2::ReviewApprovalDecision;

// ── Public re-exports for CLI test layer ──────────────────────────────────────
// These allow `apps/cli` tests to access types needed for format_fixpoint_step
// tests without importing `domain` or `usecase` directly (CN-02).
pub use domain::track_phase::{FixpointStep, ReviewScopeSet};
pub use usecase::fixpoint_resolve::{FixpointCurrentBranch, FixpointResolveError};

use crate::{CliApp, CommandOutcome};

use super::resolve_project_root;

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

// ── FsReviewGateStateAdapter ──────────────────────────────────────────────────

/// Implements [`ReviewGateStatePort`] by delegating to the existing
/// `check_approved_str` helper (same logic as `sotp review check-approved`).
///
/// Uses only the public API of the review subsystem — no internal `review.json`
/// parsing (CN-02).
struct FsReviewGateStateAdapter {
    items_dir: PathBuf,
}

impl ReviewGateStatePort for FsReviewGateStateAdapter {
    /// Query the current review gate status for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] when the review store
    /// cannot be read or the evaluation fails.
    fn review_status(&self, track_id: &TrackId) -> Result<ReviewGateStatus, FixpointResolveError> {
        let output =
            crate::review_v2::approved::check_approved_str(track_id.as_ref(), &self.items_dir)
                .map_err(|e| FixpointResolveError::GateQueryFailed {
                    gate: "review".to_owned(),
                    message: e.to_string(),
                })?;

        match output.decision {
            ReviewApprovalDecision::Approved | ReviewApprovalDecision::ApprovedWithBypass => {
                Ok(ReviewGateStatus::Approved)
            }
            ReviewApprovalDecision::Blocked => {
                let mut set = BTreeSet::new();
                for s in &output.blocked_scopes {
                    set.insert(s.clone());
                }
                let scopes = ReviewScopeSet::try_new(set).map_err(|e| {
                    FixpointResolveError::GateQueryFailed {
                        gate: "review".to_owned(),
                        message: format!("review scope set construction failed: {e}"),
                    }
                })?;
                Ok(ReviewGateStatus::NeedsReview { scopes })
            }
        }
    }
}

// ── FsRefVerifyGateStateAdapter ───────────────────────────────────────────────

/// Implements [`RefVerifyGateStatePort`] by delegating to the existing
/// `ref_verify_check_approved` composition logic (same logic as `sotp ref-verify
/// check-approved`).
///
/// Uses only the public gate API — no cache-file internals (CN-02).
struct FsRefVerifyGateStateAdapter {
    items_dir: PathBuf,
}

impl RefVerifyGateStatePort for FsRefVerifyGateStateAdapter {
    /// Query the current ref-verify gate status for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] when the cache read
    /// fails.  A wrong-branch error from ref-verify is mapped to
    /// `GateQueryFailed` as well — the CLI composition layer is responsible for
    /// ensuring the branch matches before calling this adapter.
    fn ref_verify_status(
        &self,
        track_id: &TrackId,
    ) -> Result<RefVerifyGateStatus, FixpointResolveError> {
        use crate::RefVerifyCheckApprovedInput;

        let outcome = CliApp::new()
            .ref_verify_check_approved(RefVerifyCheckApprovedInput {
                track_id: track_id.as_ref().to_owned(),
                items_dir: self.items_dir.clone(),
            })
            .map_err(|e| FixpointResolveError::GateQueryFailed {
                gate: "ref_verify".to_owned(),
                message: e,
            })?;

        if outcome.exit_code == 0 {
            Ok(RefVerifyGateStatus::Approved)
        } else {
            Ok(RefVerifyGateStatus::Blocked)
        }
    }
}

// ── Dry-gate fragment helpers ─────────────────────────────────────────────────

/// Resolve the dry-check diff-base commit for the given track directory.
///
/// Follows the same three-branch fail-closed policy as `dry check-approved` via
/// the shared [`crate::dry::resolve_dry_diff_base_from_store`] helper:
/// 1. Read `.commit_hash` → `Ok(Some(hash))` → use it.
/// 2. `Ok(None)` → fall back to `git rev-parse main`.
/// 3. `Err(Format)` / other → warn and fall back to `git rev-parse main`.
///
/// Git is discovered from `canonical_root` to anchor diff operations to the
/// correct repository (not the process CWD).
///
/// # Errors
///
/// Returns `Err` only when `git rev-parse main` fails.
fn resolve_dry_diff_base_for_track(
    track_dir: &std::path::Path,
    canonical_root: &std::path::Path,
) -> Result<domain::CommitHash, String> {
    let commit_hash_path = track_dir.join(".commit_hash");
    crate::dry::resolve_dry_diff_base_from_store(
        &commit_hash_path,
        canonical_root,
        Some(canonical_root),
        "fixpoint-resolve",
    )
}

/// Build the current diff fragment ref set for the dry gate (D5 / IN-05).
///
/// `canonical_root` is the project root (parent of `track/items`); it is passed
/// to [`extract_code_fragments`] so that all Rust source files in the project are
/// scanned.  `repo_root` is the git repository root (returned by
/// `SystemGitRepo::root()`); it is used for path normalization so that fragment
/// source paths match the repo-relative paths emitted by
/// [`GitDryCheckDiffGetter`].  In the common case `canonical_root == repo_root`;
/// they differ only when the project lives in a subdirectory of a monorepo.
///
/// # Errors
///
/// Returns `Err` when diff listing or fragment extraction / normalization fails.
fn build_current_fragment_refs(
    canonical_root: &std::path::Path,
    repo_root: &std::path::Path,
    base: &domain::CommitHash,
) -> Result<BTreeSet<FragmentRef>, String> {
    use domain::dry_check::fragments_overlapping_hunks;
    use domain::semantic_dup::CodeFragment;
    use infrastructure::dry_check::GitDryCheckDiffGetter;
    use infrastructure::semantic_dup::extractor::extract_code_fragments;
    use usecase::dry_check::DryCheckDiffSource as _;

    // Anchor diff discovery to the repo root.
    // `GitDryCheckDiffGetter` calls `SystemGitRepo::discover()` which uses the
    // process CWD; temporarily change CWD to `repo_root` so that the discover
    // call is rooted at the correct repo regardless of the caller's CWD.
    let original_cwd =
        std::env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    std::env::set_current_dir(repo_root)
        .map_err(|e| format!("cannot enter repo root '{}': {e}", repo_root.display()))?;

    let getter = GitDryCheckDiffGetter;
    let changed_hunks_result = getter.list_changed_hunks(base);

    // Restore CWD before propagating any error.
    let restore_err = std::env::set_current_dir(&original_cwd).err();
    if let Some(e) = restore_err {
        eprintln!("[warn] fixpoint-resolve: failed to restore CWD: {e}");
    }

    let changed_hunks =
        changed_hunks_result.map_err(|e| format!("list_changed_hunks failed: {e}"))?;

    let raw_fragments = extract_code_fragments(canonical_root)
        .map_err(|e| format!("fragment extraction failed: {e}"))?;

    // Normalize to repo-relative paths (strip the git repo root prefix so that
    // fragment paths match the repo-relative hunk paths emitted by
    // `GitDryCheckDiffGetter`).
    let mut normalized: Vec<CodeFragment> = Vec::with_capacity(raw_fragments.len());
    for frag in raw_fragments {
        let rel = frag
            .source_path
            .strip_prefix(repo_root)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| frag.source_path.clone());
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let rebuilt = CodeFragment::new(
            std::path::PathBuf::from(&rel_str),
            frag.content().to_owned(),
            frag.start_line(),
            frag.end_line(),
        )
        .map_err(|e| format!("fragment rebuild failed: {e}"))?;
        normalized.push(rebuilt);
    }

    let changed_paths: std::collections::HashSet<String> =
        changed_hunks.iter().map(|h| h.path().as_str().to_owned()).collect();

    let candidates: Vec<CodeFragment> = normalized
        .iter()
        .filter(|f| {
            let key = f.source_path.to_string_lossy().replace('\\', "/");
            changed_paths.contains(key.as_str())
        })
        .cloned()
        .collect();

    let diff_fragments = fragments_overlapping_hunks(&candidates, &changed_hunks);

    let mut refs = BTreeSet::new();
    for fragment in &diff_fragments {
        let r = fragment_ref_of(fragment)
            .map_err(|e| format!("fixpoint-resolve: fragment ref failed: {e}"))?;
        refs.insert(r);
    }
    Ok(refs)
}

// ── Format helpers ────────────────────────────────────────────────────────────

/// Format a [`FixpointStep`] as the canonical output string.
///
/// Output contracts:
/// - `RunDfp` → `"run-dfp"`
/// - `RunRfp { scopes }` → `"run-rfp scopes=<s1>,<s2>..."` (BTreeSet iteration order)
/// - `RunRefVerify` → `"run-ref-verify"`
/// - `Commit` → `"commit"`
pub fn format_fixpoint_step(step: FixpointStep) -> String {
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
    pub fn fixpoint_resolve(&self, input: FixpointResolveInput) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::SystemGitRepo;

        // ── Validate inputs ───────────────────────────────────────────────────
        super::validate_track_id_str(&input.track_id)
            .map_err(|e| format!("invalid --track-id: {e}"))?;

        let track_id = TrackId::try_new(input.track_id.clone())
            .map_err(|e| format!("invalid track ID: {e}"))?;

        let current_branch = FixpointCurrentBranch::try_new(input.current_branch.clone())
            .map_err(|e| format!("invalid --current-branch: {e}"))?;

        // ── Track-not-active guard ────────────────────────────────────────────
        let expected_branch = format!("track/{}", track_id.as_ref());
        if current_branch.as_str() != expected_branch {
            return Err(
                FixpointResolveError::TrackNotActive { branch: expected_branch }.to_string()
            );
        }

        // ── Resolve items_dir and discover git repo ───────────────────────────
        // The canonical items_dir is the source of truth for all subsequent
        // path operations.  For absolute --items-dir values, canonicalize
        // directly.  For relative values, anchor to the git repo root (discovered
        // from CWD) so that `track/items` resolves correctly regardless of the
        // caller's working directory — consistent with `resolve_existing_dir_under_repo`
        // in `dry/shared.rs` and other track commands.
        let canonical_items_dir = if input.items_dir.is_absolute() {
            input.items_dir.canonicalize().map_err(|_| {
                format!(
                    "--items-dir '{}' must be an existing directory under the repository root",
                    input.items_dir.display()
                )
            })?
        } else {
            // Discover the repo root from CWD first, then resolve the relative
            // items_dir path against it.
            let cwd_repo =
                SystemGitRepo::discover().map_err(|e| format!("cannot discover git repo: {e}"))?;
            let cwd_repo_root = {
                use infrastructure::git_cli::GitRepository as _;
                cwd_repo
                    .root()
                    .canonicalize()
                    .map_err(|e| format!("cannot canonicalize repo root: {e}"))?
            };
            cwd_repo_root.join(&input.items_dir).canonicalize().map_err(|_| {
                format!(
                    "--items-dir '{}' must be an existing directory under the repository root",
                    input.items_dir.display()
                )
            })?
        };

        // Derive canonical_root (project root = parent of track/) from the
        // canonical items_dir.  This is correct regardless of whether --items-dir
        // was absolute or relative.
        let canonical_root = resolve_project_root(&canonical_items_dir)
            .and_then(|p| {
                p.canonicalize().map_err(|e| format!("cannot canonicalize project root: {e}"))
            })
            .map_err(|e| format!("cannot derive project root from items_dir: {e}"))?;

        // Verify the git repo is accessible from the validated project root.
        let repo = SystemGitRepo::discover_from(&canonical_root)
            .map_err(|e| format!("cannot discover git repo: {e}"))?;
        let repo_root = {
            use infrastructure::git_cli::GitRepository as _;
            repo.root().canonicalize().map_err(|e| format!("cannot canonicalize repo root: {e}"))?
        };

        if !canonical_items_dir.starts_with(&repo_root) {
            return Err(format!(
                "--items-dir '{}' must be an existing directory under the repository root",
                input.items_dir.display()
            ));
        }

        // Verify items_dir is actually a directory (not a file or missing).
        if !canonical_items_dir.is_dir() {
            return Err(format!(
                "--items-dir '{}' must be an existing directory under the repository root",
                input.items_dir.display()
            ));
        }

        // ── Build dry-gate fragment refs ──────────────────────────────────────
        let track_dir = canonical_items_dir.join(track_id.as_ref());

        // Anchor all git operations (diff-base ancestry check, diff getter) to
        // the validated repo root.  Both `resolve_dry_diff_base_for_track`
        // (through `FsDryCheckCommitHashStore::read` → `SystemGitRepo::discover`)
        // and `build_current_fragment_refs` (through `GitDryCheckDiffGetter` →
        // `SystemGitRepo::discover`) rely on CWD for repo discovery.
        let original_cwd =
            std::env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
        std::env::set_current_dir(&repo_root)
            .map_err(|e| format!("cannot enter repo root '{}': {e}", repo_root.display()))?;

        let base_result = resolve_dry_diff_base_for_track(&track_dir, &canonical_root);

        // Restore CWD before `build_current_fragment_refs` (which manages its
        // own CWD guard internally).
        if let Err(e) = std::env::set_current_dir(&original_cwd) {
            eprintln!("[warn] fixpoint-resolve: failed to restore CWD after diff-base: {e}");
        }

        let base = base_result?;
        let current_fragment_refs =
            build_current_fragment_refs(&canonical_root, &repo_root, &base)?;

        // ── Build dry-gate interactor (read-only) ─────────────────────────────
        let dry_check_json_path = track_dir.join("dry-check.json");
        let dry_check_coverage_path = track_dir.join("dry-check-coverage.json");
        let store = Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root.clone()));
        let coverage = Arc::new(FsDryCheckCoverageAdapter::new(
            dry_check_coverage_path,
            canonical_root.clone(),
        ));
        let dry_approval = Arc::new(DryCheckApprovalInteractor::new(store, coverage));

        // ── Build review and ref-verify gate adapters ─────────────────────────
        // Use the canonical (absolute, validated) items_dir so that these adapters
        // resolve track artifacts correctly regardless of the caller's CWD.
        let review_state =
            Arc::new(FsReviewGateStateAdapter { items_dir: canonical_items_dir.clone() })
                as Arc<dyn ReviewGateStatePort>;
        let ref_verify_results =
            Arc::new(FsRefVerifyGateStateAdapter { items_dir: canonical_items_dir.clone() })
                as Arc<dyn RefVerifyGateStatePort>;

        // ── Construct and run the interactor ──────────────────────────────────
        let interactor =
            FixpointResolveInteractor::new(dry_approval, review_state, ref_verify_results);

        let cmd = FixpointResolveCommand { track_id, current_branch, current_fragment_refs };

        let step = interactor.resolve(&cmd).map_err(|e| format!("fixpoint-resolve failed: {e}"))?;

        Ok(CommandOutcome::success(Some(format_fixpoint_step(step))))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;

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
        let msg = result.unwrap_err();
        assert!(
            msg.contains("items_dir")
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
        let msg = result.unwrap_err();
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
        let msg = result.unwrap_err();
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
        let msg = result.unwrap_err();
        assert!(
            msg.contains("track-id") || msg.contains("track id"),
            "error must mention the invalid track-id, got: {msg}"
        );
    }

    // ── Dry gate blocked → "run-dfp" ─────────────────────────────────────────

    /// When the dry coverage record is absent (coverage port returns `Ok(None)`),
    /// the dry gate must be `Blocked` and the step must be `RunDfp`.
    ///
    /// The review and ref-verify adapters are never reached because the dry gate
    /// has priority, so `review-scope.json` and ref-verify cache are not needed.
    ///
    /// This test uses the actual workspace git repository (discovered via CWD) because
    /// `GitDryCheckDiffGetter::list_changed_hunks` calls `SystemGitRepo::discover()`
    /// from CWD.  We write the workspace HEAD SHA to `.commit_hash` inside a temporary
    /// `items_dir` that lives under the workspace root, so that:
    ///   - The items_dir containment check passes (within the repo).
    ///   - `git merge-base HEAD <sha>` succeeds (sha is a real workspace commit).
    ///   - `dry-check-coverage.json` is absent → dry gate is Blocked → `run-dfp`.
    #[test]
    fn test_fixpoint_resolve_missing_coverage_record_returns_run_dfp() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        use crate::test_support::repo_root_for_tests;

        let workspace_root = repo_root_for_tests();

        // Create a temp fixture tree under target/ that matches the `<root>/track/items`
        // structure required by `resolve_project_root`.  Using target/ keeps it inside the
        // repo root (items_dir containment check passes) without polluting track/items.
        let base = workspace_root.join("target").join("fixpoint-resolve-tests");
        std::fs::create_dir_all(&base).unwrap();

        let temp_fixture = tempfile::Builder::new()
            .prefix("fixture-")
            .tempdir_in(&base)
            .expect("fixture temp dir under workspace must be created");
        // Build `<temp_fixture>/track/items/<track_id>` so resolve_project_root sees
        // `<temp_fixture>/track/items` as ending in `track/items`.
        let track_id_str = "dfp-track-2026";
        let items_dir = temp_fixture.path().join("track").join("items");
        let track_dir = items_dir.join(track_id_str);
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write the workspace HEAD SHA to .commit_hash so merge-base resolves correctly
        // (GitDryCheckDiffGetter uses the CWD git repo — the workspace — not the temp repo).
        let head_sha_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&workspace_root)
            .output()
            .expect("git rev-parse HEAD must succeed in workspace");
        let head_sha = String::from_utf8_lossy(&head_sha_output.stdout).trim().to_owned();
        std::fs::write(track_dir.join(".commit_hash"), &head_sha).unwrap();

        // No dry-check-coverage.json → dry gate is Blocked.
        let outcome = CliApp::new()
            .fixpoint_resolve(FixpointResolveInput {
                track_id: track_id_str.to_owned(),
                current_branch: format!("track/{track_id_str}"),
                items_dir: items_dir.clone(),
            })
            .expect("fixpoint-resolve with missing coverage must succeed (not Err)");

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("run-dfp"),
            "missing coverage record must yield run-dfp"
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
        let msg = result.unwrap_err();
        assert!(
            msg.contains("directory") || msg.contains("items_dir") || msg.contains("items-dir"),
            "error must mention directory constraint, got: {msg}"
        );
    }

    // ── FsReviewGateStateAdapter unit tests ───────────────────────────────────

    /// `FsReviewGateStateAdapter::review_status` must return `Err(GateQueryFailed)`
    /// when the underlying `check_approved_str` fails (no `review-scope.json`).
    ///
    /// This exercises the error-mapping path in the adapter.  The `run-rfp`,
    /// `run-ref-verify`, and `commit` step variants are exercised at the usecase
    /// layer (`libs/usecase/src/fixpoint_resolve.rs`) using stub adapters, so the
    /// composition layer tests focus on adapter wiring correctness.
    #[test]
    fn test_fs_review_gate_state_adapter_returns_gate_query_failed_when_no_scope_config() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        use domain::TrackId;

        let (dir, items_dir) = seed_track_repo("my-track-2026");

        // `review-scope.json` is absent in the isolated repo — `check_approved_str`
        // will fail to load the scope config and return an error.
        let adapter = FsReviewGateStateAdapter { items_dir };
        let track_id = TrackId::try_new("my-track-2026".to_owned()).unwrap();
        let result = adapter.review_status(&track_id);

        drop(dir);

        assert!(result.is_err(), "expected Err when review-scope.json is absent");
        assert!(
            matches!(result.unwrap_err(), FixpointResolveError::GateQueryFailed { .. }),
            "error must be GateQueryFailed"
        );
    }

    // ── FsRefVerifyGateStateAdapter unit tests ────────────────────────────────

    /// `FsRefVerifyGateStateAdapter::ref_verify_status` must return
    /// `Ok(RefVerifyGateStatus::Approved)` when there are no production ref-verify
    /// pairs (no `spec.json` in the track dir → zero Chain-1 pairs).
    ///
    /// Uses an isolated temp git repo on `track/my-track-2026` (via `seed_track_repo`)
    /// so that the branch check inside `ref_verify_check_approved` passes without
    /// relying on the host checkout state.  The track dir has no `spec.json`
    /// (zero Chain-1 pairs), so the gate returns `Approved`.
    ///
    /// Exercises the `RefVerifyGateStatus::Approved` path in the adapter.
    #[test]
    fn test_fs_ref_verify_gate_state_adapter_returns_approved_when_no_production_pairs() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        use domain::TrackId;

        let track_id_str = "my-track-2026";
        // seed_track_repo creates an isolated git repo on `track/<id>` with the
        // track dir already present.  No spec.json → zero Chain-1 pairs → Approved.
        let (dir, items_dir) = seed_track_repo(track_id_str);

        let adapter = FsRefVerifyGateStateAdapter { items_dir };
        let track_id = TrackId::try_new(track_id_str.to_owned()).unwrap();
        let result = adapter.ref_verify_status(&track_id);

        drop(dir);

        assert!(
            matches!(result, Ok(RefVerifyGateStatus::Approved)),
            "expected Ok(Approved) when no production pairs, got: {result:?}"
        );
    }
}
