//! Gate-state adapters for the fixpoint resolver.
//!
//! Provides filesystem-backed implementations of the two gate-state ports used
//! by [`usecase::fixpoint_resolve::FixpointResolveInteractor`]:
//!
//! - [`FsReviewGateStateAdapter`] — implements [`ReviewGateStatePort`] by
//!   reading the review gate approval state from the review store (same logic
//!   as `sotp review check-approved`).
//! - [`FsRefVerifyGateStateAdapter`] — implements [`RefVerifyGateStatePort`]
//!   by reading ref-verify pass-cache entries for all production pairs (same
//!   logic as `sotp ref-verify check-approved`, minus the branch check which
//!   is done upstream by the fixpoint resolver).
//!
//! Both adapters were relocated from
//! `apps/cli-composition/src/track/fixpoint_resolve.rs` per ADR 1328 D7
//! (port-impl adapters belong in `libs/infrastructure`, not in the CLI
//! composition root).  A re-export shim in the original module keeps existing
//! wiring call sites unchanged.
//!
//! Design: D7 / IN-11 / CN-05 / AC-09.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::review_v2::{FastVerdict, LogInfo, ReviewTarget, Verdict};
use usecase::fixpoint_resolve::{
    FixpointResolveError, RefVerifyGateStatePort, RefVerifyGateStatus, ReviewGateStatePort,
    ReviewGateStatus,
};
use usecase::review_v2::{ReviewCycle, ReviewerError, ports::Reviewer};

use crate::git_cli::{GitRepository as _, SystemGitRepo};
use crate::ref_verify::{
    RefVerifyCacheAdapter, RefVerifyPairSourceAdapter, RefVerifyScopeResolver,
};
use crate::review_v2::{FsCommitHashStore, FsReviewStore, GitDiffGetter, SystemReviewHasher};
use domain::track_phase::ReviewScopeSet;

// ── helpers ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct GateStateError(String);

impl std::fmt::Display for GateStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl GateStateError {
    #[allow(dead_code)]
    fn contains(&self, s: &str) -> bool {
        self.0.contains(s)
    }
}

struct GatePathContext {
    canonical_root: PathBuf,
    canonical_items_dir: PathBuf,
}

struct RepoCwdGuard {
    original: PathBuf,
    restored: bool,
}

impl RepoCwdGuard {
    fn change_to(repo_root: &Path) -> Result<Self, GateStateError> {
        let original = std::env::current_dir()
            .map_err(|e| GateStateError(format!("failed to read current directory: {e}")))?;
        std::env::set_current_dir(repo_root).map_err(|e| {
            GateStateError(format!("failed to enter repo root {}: {e}", repo_root.display()))
        })?;
        Ok(Self { original, restored: false })
    }

    fn restore(&mut self) -> Result<(), GateStateError> {
        if self.restored {
            return Ok(());
        }
        std::env::set_current_dir(&self.original).map_err(|e| {
            GateStateError(format!(
                "failed to restore current directory {}: {e}",
                self.original.display()
            ))
        })?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for RepoCwdGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn with_repo_cwd<T>(
    repo_root: &Path,
    f: impl FnOnce() -> Result<T, GateStateError>,
) -> Result<T, GateStateError> {
    let mut guard = RepoCwdGuard::change_to(repo_root)?;
    let result = f();
    if let Err(e) = guard.restore() {
        eprintln!("[warn] gate-state: {e}");
    }
    result
}

/// Resolves `<project-root>/track/items` → `<project-root>`.
///
/// Mirrors `apps/cli-composition::track::resolve_project_root`; kept private so
/// the infrastructure crate does not export a duplicate.
fn resolve_project_root_from_items_dir(items_dir: &Path) -> Result<PathBuf, GateStateError> {
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
        _ => Err(GateStateError(format!(
            "items_dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        ))),
    }
}

fn resolve_gate_path_context(items_dir: &Path) -> Result<GatePathContext, GateStateError> {
    // Use the caller's current repository as the primary trust anchor.
    // Absolute `items_dir` values are still untrusted input; discovering from
    // them first can select a nested checkout instead of the caller's repo.
    let caller_git =
        SystemGitRepo::discover().map_err(|e| GateStateError(format!("git discover: {e}")))?;

    let canonical_repo_root = caller_git.root().canonicalize().map_err(|e| {
        GateStateError(format!("cannot canonicalize git root {}: {e}", caller_git.root().display()))
    })?;

    let items_dir_abs = if items_dir.is_absolute() {
        items_dir.to_path_buf()
    } else {
        canonical_repo_root.join(items_dir)
    };
    // Guard the caller-supplied path before canonicalizing it. This catches
    // absolute paths that reach the repository through an out-of-repo symlink
    // and would otherwise appear safe only after `canonicalize()`.
    crate::track::symlink_guard::reject_symlinks_below(&items_dir_abs, &canonical_repo_root)
        .map_err(|e| {
            GateStateError(format!(
                "symlink guard: refusing to use items_dir {}: {e}",
                items_dir.display()
            ))
        })?;
    match items_dir_abs.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(GateStateError(format!(
                "symlink guard: refusing to use symlinked items_dir: {}",
                items_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(GateStateError(format!(
                "symlink guard: cannot stat items_dir {}: {e}",
                items_dir.display()
            )));
        }
    }
    let canonical_items_dir = items_dir_abs.canonicalize().map_err(|_| {
        GateStateError(format!(
            "items_dir '{}' is outside the repository root '{}' or does not exist",
            items_dir.display(),
            canonical_repo_root.display()
        ))
    })?;
    if !canonical_items_dir.starts_with(&canonical_repo_root) {
        return Err(GateStateError(format!(
            "items_dir '{}' resolves outside the repository root '{}'",
            items_dir.display(),
            canonical_repo_root.display()
        )));
    }
    if !canonical_items_dir.is_dir() {
        return Err(GateStateError(format!(
            "items_dir '{}' is not a directory",
            items_dir.display()
        )));
    }

    let project_root = resolve_project_root_from_items_dir(&canonical_items_dir)?;
    let canonical_root = project_root
        .canonicalize()
        .map_err(|e| GateStateError(format!("cannot canonicalize project root: {e}")))?;
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(GateStateError(format!(
            "items_dir '{}' resolves outside the project root '{}'",
            items_dir.display(),
            canonical_root.display()
        )));
    }

    Ok(GatePathContext { canonical_root, canonical_items_dir })
}

// ── NullReviewer ──────────────────────────────────────────────────────────────

/// Null reviewer — never called; exists only to satisfy the `ReviewCycle` type
/// parameter for the status/check-approved path.
struct NullReviewer;

impl Reviewer for NullReviewer {
    fn review(&self, _target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected("NullReviewer: review() must not be called".to_owned()))
    }

    fn fast_review(&self, _target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected("NullReviewer: fast_review() must not be called".to_owned()))
    }
}

// ── FsReviewGateStateAdapter ──────────────────────────────────────────────────

/// Filesystem adapter implementing [`ReviewGateStatePort`].
///
/// Reads review gate approval state from the items_dir using the existing
/// review-v2 approval infrastructure. Relocated from
/// `cli_composition::track::fixpoint_resolve` per ADR 1328 D7.
pub struct FsReviewGateStateAdapter {
    items_dir: PathBuf,
}

impl FsReviewGateStateAdapter {
    /// Creates a new adapter anchored to `items_dir` (the `track/items` directory).
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }
}

impl ReviewGateStatePort for FsReviewGateStateAdapter {
    /// Query the current review gate status for `track_id`.
    ///
    /// Replicates the logic of `cli_composition::review_v2::approved::check_approved_str`:
    /// 1. Discover git repo and derive the canonical project root.
    /// 2. Load `review-scope.json` from `.harness/config/`.
    /// 3. Build `FsReviewStore` + `FsCommitHashStore` for the track directory.
    /// 4. Evaluate the approval verdict via `ReviewCycle::evaluate_approval`.
    /// 5. Map the verdict to [`ReviewGateStatus`].
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] when any I/O or
    /// evaluation step fails.
    fn review_status(&self, track_id: &TrackId) -> Result<ReviewGateStatus, FixpointResolveError> {
        let gate_err = |msg: String| FixpointResolveError::GateQueryFailed {
            gate: "review".to_owned(),
            message: msg,
        };

        let GatePathContext { canonical_root, canonical_items_dir } =
            resolve_gate_path_context(&self.items_dir)
                .map_err(|e| gate_err(format!("cannot resolve gate paths: {e}")))?;

        // Load review-scope.json.
        let scope_json_path = canonical_root.join(".harness/config/review-scope.json");
        let scope_config =
            crate::review_v2::load_v2_scope_config(&scope_json_path, track_id, &canonical_root)
                .map_err(|e| gate_err(format!("load review-scope.json: {e}")))?;

        // Build store paths.
        let track_dir = canonical_items_dir.join(track_id.as_ref());
        if !track_dir.is_dir() {
            return Err(gate_err(format!(
                "track directory '{}' does not exist. Check track id '{}' and items_dir '{}'",
                track_dir.display(),
                track_id.as_ref(),
                self.items_dir.display()
            )));
        }
        let review_json_path = track_dir.join("review.json");
        let commit_hash_path = track_dir.join(".commit_hash");

        let review_store = FsReviewStore::new(review_json_path, canonical_root.clone());
        let commit_hash_store = FsCommitHashStore::new(commit_hash_path, canonical_root.clone());

        // Resolve diff base (read .commit_hash; fallback to git rev-parse main).
        let base = with_repo_cwd(&canonical_root, || {
            use domain::review_v2::CommitHashReader as _;
            match commit_hash_store.read() {
                Ok(Some(hash)) => Ok(hash),
                Ok(None) | Err(_) => {
                    // Fallback: git rev-parse main (same as build_v2_shared in cli_composition).
                    let git = SystemGitRepo::discover()
                        .map_err(|e| GateStateError(format!("git discover: {e}")))?;
                    let output = git
                        .output(&["rev-parse", "main"])
                        .map_err(|e| GateStateError(format!("git rev-parse main: {e}")))?;
                    if !output.status.success() {
                        return Err(GateStateError("git rev-parse main failed".to_owned()));
                    }
                    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    domain::CommitHash::try_new(&sha)
                        .map_err(|e| GateStateError(format!("invalid main SHA: {e}")))
                }
            }
        })
        .map_err(|e| gate_err(e.to_string()))?;

        // Build ReviewCycle with NullReviewer (we only call evaluate_approval).
        let cycle =
            ReviewCycle::new(base, scope_config, NullReviewer, GitDiffGetter, SystemReviewHasher);

        // Check whether review.json exists (needed by evaluate_approval).
        let review_json_exists = {
            use domain::review_v2::ReviewExistsPort as _;
            review_store
                .review_json_exists()
                .map_err(|e| gate_err(format!("review store read failed: {e}")))?
        };

        let verdict = with_repo_cwd(&canonical_root, || {
            cycle
                .evaluate_approval(&review_store, review_json_exists)
                .map_err(|e| GateStateError(format!("review approval evaluation failed: {e}")))
        })
        .map_err(|e| gate_err(e.to_string()))?;

        match verdict {
            domain::review_v2::ReviewApprovalVerdict::Approved
            | domain::review_v2::ReviewApprovalVerdict::ApprovedWithBypass { .. } => {
                Ok(ReviewGateStatus::Approved)
            }
            domain::review_v2::ReviewApprovalVerdict::Blocked { required_scopes } => {
                let mut set = BTreeSet::new();
                for s in &required_scopes {
                    set.insert(s.to_string());
                }
                let scopes = ReviewScopeSet::try_new(set)
                    .map_err(|e| gate_err(format!("review scope set construction failed: {e}")))?;
                Ok(ReviewGateStatus::NeedsReview { scopes })
            }
        }
    }
}

// ── FsRefVerifyGateStateAdapter ───────────────────────────────────────────────

/// Filesystem adapter implementing [`RefVerifyGateStatePort`].
///
/// Reads ref-verify gate approval state from the items_dir. Relocated from
/// `cli_composition::track::fixpoint_resolve` per ADR 1328 D7.
pub struct FsRefVerifyGateStateAdapter {
    items_dir: PathBuf,
}

impl FsRefVerifyGateStateAdapter {
    /// Creates a new adapter anchored to `items_dir` (the `track/items` directory).
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }
}

impl RefVerifyGateStatePort for FsRefVerifyGateStateAdapter {
    /// Query the current ref-verify gate status for `track_id`.
    ///
    /// Replicates the logic of `cli_composition::ref_verify::ref_verify_check_approved`
    /// minus the current-branch check (which is done upstream by the fixpoint
    /// resolver before this adapter is called):
    /// 1. Derive the canonical project root from `items_dir`.
    /// 2. Resolve the ref-verify scope from on-disk artifact existence.
    /// 3. Load production pairs via [`RefVerifyPairSourceAdapter`].
    /// 4. For each production pair, verify a Pass cache entry exists via
    ///    [`RefVerifyCacheAdapter`].
    /// 5. Return [`RefVerifyGateStatus::Approved`] if all pairs are verified,
    ///    [`RefVerifyGateStatus::Blocked`] otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] when I/O fails
    /// (scope resolution, pair loading, or cache reading).
    fn ref_verify_status(
        &self,
        track_id: &TrackId,
    ) -> Result<RefVerifyGateStatus, FixpointResolveError> {
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let gate_err = |msg: String| FixpointResolveError::GateQueryFailed {
            gate: "ref_verify".to_owned(),
            message: msg,
        };

        let GatePathContext { canonical_root, .. } = resolve_gate_path_context(&self.items_dir)
            .map_err(|e| gate_err(format!("cannot resolve gate paths: {e}")))?;

        // Resolve the ref-verify scope from artifact existence.
        let resolver = RefVerifyScopeResolver::new(canonical_root.clone());
        let scope = resolver
            .resolve(track_id.as_ref())
            .map_err(|e| gate_err(format!("ref-verify scope resolution failed: {e}")))?;

        // Build the RefVerifyCommand. The current_branch is set to the expected
        // branch for this track; the branch check itself was done upstream by the
        // fixpoint resolver, so we set it structurally correct here.
        let current_branch = format!("track/{}", track_id.as_ref());
        let cmd = usecase::ref_verify::RefVerifyCommand {
            track_id: track_id.clone(),
            scope,
            current_branch,
        };
        let config = usecase::ref_verify::RefVerifyConfig::default();

        // Load all pairs for the resolved scope.
        let pair_source = RefVerifyPairSourceAdapter::new(canonical_root.clone());
        let pairs = pair_source
            .load_pairs(&cmd, &config)
            .map_err(|e| gate_err(format!("failed to load pairs: {e}")))?;

        // Filter to production pairs only (exclude known-bad probes).
        let production_pairs: Vec<_> = pairs.into_iter().filter(|p| !p.known_bad).collect();

        // Zero production pairs → gate passes vacuously.
        if production_pairs.is_empty() {
            return Ok(RefVerifyGateStatus::Approved);
        }

        // Load and check cache entries for each production pair.
        let cache_adapter = RefVerifyCacheAdapter::new(canonical_root);

        // Group pairs by cache scope to minimise cache file reads.
        let mut scope_keys: std::collections::HashMap<
            RefVerifyCacheScope,
            Vec<(domain::ContentHash, domain::ContentHash)>,
        > = std::collections::HashMap::new();
        for pair in &production_pairs {
            scope_keys
                .entry(pair.cache_scope.clone())
                .or_default()
                .push((pair.claim_hash.clone(), pair.evidence_hash.clone()));
        }

        for (cache_scope, pair_keys) in &scope_keys {
            let entries = cache_adapter.load_entries(&cmd, cache_scope).map_err(|e| {
                gate_err(format!("failed to read verify-cache for {cache_scope:?}: {e}"))
            })?;

            use domain::tddd::semantic_verify::SemanticVerdict;
            for (claim_hash, evidence_hash) in pair_keys {
                let matching_entries = entries
                    .iter()
                    .filter(|entry| {
                        entry.claim_hash == *claim_hash && entry.evidence_hash == *evidence_hash
                    })
                    .collect::<Vec<_>>();

                // Any missing or non-Pass entry blocks the gate.
                if matching_entries.is_empty() {
                    return Ok(RefVerifyGateStatus::Blocked);
                }
                if matching_entries
                    .iter()
                    .any(|entry| !matches!(entry.verdict, SemanticVerdict::Pass { .. }))
                {
                    return Ok(RefVerifyGateStatus::Blocked);
                }
            }
        }

        Ok(RefVerifyGateStatus::Approved)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use domain::TrackId;
    use usecase::fixpoint_resolve::{
        FixpointResolveError, RefVerifyGateStatePort as _, RefVerifyGateStatus,
        ReviewGateStatePort as _, ReviewGateStatus,
    };

    use super::{FsRefVerifyGateStateAdapter, FsReviewGateStateAdapter};

    // ── test helpers ──────────────────────────────────────────────────────────

    fn cwd_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        fn enter(path: &Path) -> Self {
            let original = std::env::current_dir().expect("current_dir must be readable");
            std::env::set_current_dir(path).expect("test must enter temp repo");
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
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
        assert!(status.success(), "git {:?} failed with {status}", args);
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

    fn assert_gate_query_failed_contains<T: std::fmt::Debug>(
        result: Result<T, FixpointResolveError>,
        expected: &str,
    ) {
        let err = result.expect_err("expected GateQueryFailed");
        match err {
            FixpointResolveError::GateQueryFailed { message, .. } => {
                assert!(
                    message.contains(expected),
                    "expected message to contain {expected:?}, got: {message}"
                );
            }
            other => panic!("expected GateQueryFailed, got: {other:?}"),
        }
    }

    // ── FsReviewGateStateAdapter tests ────────────────────────────────────────

    /// When `review-scope.json` is absent, `review_status` must return
    /// `Err(GateQueryFailed)` (the review store cannot be configured).
    #[test]
    fn test_fs_review_gate_state_adapter_returns_gate_query_failed_when_no_scope_config() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let (dir, items_dir) = seed_track_repo("my-track-2026");
        let _cwd = CwdGuard::enter(dir.path());

        // No `.harness/config/review-scope.json` in the isolated repo.
        let adapter = FsReviewGateStateAdapter::new(items_dir);
        let track_id = TrackId::try_new("my-track-2026".to_owned()).unwrap();
        let result = adapter.review_status(&track_id);

        assert!(result.is_err(), "expected Err when review-scope.json is absent");
        assert!(
            matches!(result.unwrap_err(), FixpointResolveError::GateQueryFailed { .. }),
            "error must be GateQueryFailed"
        );
    }

    #[test]
    fn test_fs_review_gate_state_adapter_rejects_relative_items_dir_escape() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let repo_root = parent.path().join("repo");
        let escaped_items_dir = parent.path().join("outside").join("track").join("items");
        std::fs::create_dir_all(&repo_root).unwrap();
        std::fs::create_dir_all(&escaped_items_dir).unwrap();
        run_git(&repo_root, &["init", "-q"]);

        let _cwd = CwdGuard::enter(&repo_root);
        let adapter = FsReviewGateStateAdapter::new(PathBuf::from("../outside/track/items"));
        let track_id = TrackId::try_new("my-track-2026".to_owned()).unwrap();
        let result = adapter.review_status(&track_id);

        assert_gate_query_failed_contains(result, "outside the repository root");
    }

    #[test]
    fn test_resolve_gate_path_context_rejects_absolute_items_dir_in_other_repo() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let caller_root = parent.path().join("caller");
        let other_root = parent.path().join("other");
        let other_items_dir = other_root.join("track").join("items");
        std::fs::create_dir_all(&caller_root).unwrap();
        std::fs::create_dir_all(other_items_dir.join("my-track-2026")).unwrap();
        run_git(&caller_root, &["init", "-q"]);
        run_git(&other_root, &["init", "-q"]);

        let _cwd = CwdGuard::enter(&caller_root);
        let result = super::resolve_gate_path_context(&other_items_dir);

        match result {
            Ok(_) => panic!("absolute items_dir in a different repo must be rejected"),
            Err(err) => assert!(
                err.contains("outside the repository root"),
                "expected repository-root containment error, got: {err}"
            ),
        }
    }

    #[test]
    fn test_resolve_gate_path_context_accepts_nested_absolute_items_dir_under_caller_repo() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let caller_root = parent.path().join("caller");
        let nested_root = caller_root.join("nested");
        let nested_items_dir = nested_root.join("track").join("items");
        std::fs::create_dir_all(nested_items_dir.join("my-track-2026")).unwrap();
        run_git(&caller_root, &["init", "-q"]);
        run_git(&nested_root, &["init", "-q"]);

        let _cwd = CwdGuard::enter(&caller_root);
        let context = super::resolve_gate_path_context(&nested_items_dir)
            .expect("nested items_dir under caller repo should resolve");

        assert_eq!(context.canonical_items_dir, nested_items_dir.canonicalize().unwrap());
        assert_eq!(context.canonical_root, nested_root.canonicalize().unwrap());
    }

    #[test]
    fn test_fs_review_gate_state_adapter_anchors_review_evaluation_to_project_root() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let caller_root = parent.path().join("caller");
        let nested_root = caller_root.join("nested");
        let config_dir = nested_root.join(".harness").join("config");
        let nested_items_dir = nested_root.join("track").join("items");
        let track_id_str = "my-track-2026";
        let track_dir = nested_items_dir.join(track_id_str);
        std::fs::create_dir_all(&caller_root).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        run_git(&caller_root, &["init", "-q"]);
        run_git(&nested_root, &["init", "-q"]);
        run_git(&nested_root, &["config", "commit.gpgsign", "false"]);
        run_git(&nested_root, &["checkout", "-B", "main"]);
        std::fs::write(
            config_dir.join("review-scope.json"),
            r#"{"version":2,"groups":{"infra":{"patterns":["src/**"]}}}"#,
        )
        .unwrap();
        std::fs::write(nested_root.join("README.md"), "init\n").unwrap();
        run_git(&nested_root, &["add", "."]);
        run_git(&nested_root, &["commit", "--no-gpg-sign", "-m", "init"]);
        run_git(&nested_root, &["checkout", "-b", &format!("track/{track_id_str}")]);
        let head_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&nested_root)
            .output()
            .unwrap();
        assert!(head_output.status.success(), "git rev-parse HEAD failed");
        let head_sha = String::from_utf8_lossy(&head_output.stdout).trim().to_owned();
        std::fs::write(track_dir.join(".commit_hash"), head_sha).unwrap();

        let _cwd = CwdGuard::enter(&caller_root);
        let adapter = FsReviewGateStateAdapter::new(nested_items_dir);
        let track_id = TrackId::try_new(track_id_str.to_owned()).unwrap();
        let result = adapter.review_status(&track_id);

        assert!(
            matches!(result, Ok(ReviewGateStatus::Approved)),
            "expected nested project-root evaluation to approve, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_gate_path_context_rejects_symlinked_items_dir() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let repo_root = parent.path().join("repo");
        let outside_items_dir = parent.path().join("outside").join("track").join("items");
        let track_dir = repo_root.join("track");
        let symlinked_items_dir = track_dir.join("items");
        std::fs::create_dir_all(&repo_root).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::create_dir_all(&outside_items_dir).unwrap();
        std::os::unix::fs::symlink(&outside_items_dir, &symlinked_items_dir).unwrap();
        run_git(&repo_root, &["init", "-q"]);

        let _cwd = CwdGuard::enter(&repo_root);
        let result = super::resolve_gate_path_context(&symlinked_items_dir);

        match result {
            Ok(_) => panic!("symlinked items_dir must be rejected"),
            Err(err) => {
                assert!(err.contains("symlink guard"), "expected symlink guard error, got: {err}")
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_gate_path_context_rejects_symlinked_items_dir_parent() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let repo_root = parent.path().join("repo");
        let outside_track_dir = parent.path().join("outside").join("track");
        let symlinked_track_dir = repo_root.join("track");
        let symlinked_items_dir = symlinked_track_dir.join("items");
        std::fs::create_dir_all(&repo_root).unwrap();
        std::fs::create_dir_all(outside_track_dir.join("items")).unwrap();
        std::os::unix::fs::symlink(&outside_track_dir, &symlinked_track_dir).unwrap();
        run_git(&repo_root, &["init", "-q"]);

        let _cwd = CwdGuard::enter(&repo_root);
        let result = super::resolve_gate_path_context(&symlinked_items_dir);

        match result {
            Ok(_) => panic!("symlinked items_dir parent must be rejected"),
            Err(err) => {
                assert!(err.contains("symlink guard"), "expected symlink guard error, got: {err}")
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_gate_path_context_rejects_absolute_symlink_parent_outside_repo() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let repo_root = parent.path().join("repo");
        let items_dir = repo_root.join("track").join("items");
        let outside_track_link = parent.path().join("outside-track");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::os::unix::fs::symlink(repo_root.join("track"), &outside_track_link).unwrap();
        run_git(&repo_root, &["init", "-q"]);

        let _cwd = CwdGuard::enter(&repo_root);
        let result = super::resolve_gate_path_context(&outside_track_link.join("items"));

        match result {
            Ok(_) => panic!("absolute items_dir through out-of-repo symlink must be rejected"),
            Err(err) => {
                assert!(err.contains("symlink guard"), "expected symlink guard error, got: {err}")
            }
        }
    }

    #[test]
    fn test_fs_review_gate_state_adapter_returns_error_when_track_dir_missing() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        let config_dir = root.join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("review-scope.json"),
            r#"{"version":2,"groups":{"infra":{"patterns":["src/**"]}}}"#,
        )
        .unwrap();
        let items_dir = root.join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);

        let adapter = FsReviewGateStateAdapter::new(items_dir);
        let track_id = TrackId::try_new("missing-track-2026".to_owned()).unwrap();
        let _cwd = CwdGuard::enter(root);
        let result = adapter.review_status(&track_id);

        assert_gate_query_failed_contains(result, "track directory");
    }

    /// When `.commit_hash` points to HEAD of the outer workspace and `diff HEAD HEAD`
    /// is empty, all scope groups classify zero files → every scope is
    /// `NotRequired(Empty)` → `evaluate_approval` returns `Approved` directly.
    ///
    /// `GitDiffGetter` always uses the process CWD to discover the git repo, so this
    /// test must operate within the outer workspace.  We create an isolated temp
    /// subdirectory inside the real `track/items/` directory so that:
    ///   - `items_dir` points to the workspace's `track/items/`
    ///   - `canonical_root` resolves to the workspace root
    ///   - `review-scope.json` exists at `.harness/config/review-scope.json`
    ///   - `.commit_hash` contains the current HEAD SHA → diff is empty
    #[test]
    fn test_fs_review_gate_state_adapter_returns_approved_when_diff_is_empty() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        use crate::git_cli::{GitRepository as _, SystemGitRepo};

        // Derive workspace root from CARGO_MANIFEST_DIR (libs/infrastructure → ../.. = workspace).
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
        let items_dir = workspace_root.join("track").join("items");

        // Bail if the workspace items directory does not exist (atypical CI env).
        if !items_dir.is_dir() {
            eprintln!(
                "[skip] test_fs_review_gate_state_adapter_returns_approved_when_diff_is_empty: \
                 items_dir {items_dir:?} absent"
            );
            return;
        }
        let _cwd = CwdGuard::enter(workspace_root);

        // Create an isolated subdirectory inside track/items/ so the adapter sees
        // it as a real track directory.  tempdir_in ensures cleanup on drop.
        let track_temp =
            tempfile::tempdir_in(&items_dir).expect("must create temp track dir in items_dir");
        let track_id_str = track_temp.path().file_name().unwrap().to_str().unwrap().to_owned();
        // track IDs must be valid; tempdir names are alphanumeric + '.' which is fine.
        let track_id = match TrackId::try_new(track_id_str.clone()) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("[skip] temp dir name is not a valid track ID ({e}); using fallback");
                // Fallback: create the dir with a known-valid name inside items_dir.
                let fallback_id = "test-review-gate-2026-tmp";
                let fallback_dir = items_dir.join(fallback_id);
                std::fs::create_dir_all(&fallback_dir).unwrap();
                let id = TrackId::try_new(fallback_id.to_owned()).unwrap();
                drop(track_temp);
                // Write .commit_hash anchored at HEAD so the adapter doesn't fall
                // back to `git rev-parse main` (which is unavailable on shallow CI
                // checkouts that fetch only the PR branch).
                let git = SystemGitRepo::discover_from(workspace_root)
                    .expect("outer workspace must be a git repo");
                let head_output = git.output(&["rev-parse", "HEAD"]).expect("git rev-parse HEAD");
                assert!(head_output.status.success(), "git rev-parse HEAD failed");
                let head_sha = String::from_utf8_lossy(&head_output.stdout).trim().to_owned();
                std::fs::write(fallback_dir.join(".commit_hash"), &head_sha).unwrap();
                let adapter = FsReviewGateStateAdapter::new(items_dir.clone());
                let result = adapter.review_status(&id);
                let _ = std::fs::remove_dir_all(&fallback_dir);
                assert!(
                    matches!(result, Ok(ReviewGateStatus::Approved)),
                    "expected Ok(Approved) for empty diff, got: {result:?}"
                );
                return;
            }
        };

        // Get the current HEAD SHA of the outer workspace so that diff base = HEAD.
        // `diff HEAD HEAD` = empty diff → no classified files → Approved.
        let git = SystemGitRepo::discover_from(workspace_root)
            .expect("outer workspace must be a git repo");
        let head_output = git.output(&["rev-parse", "HEAD"]).expect("git rev-parse HEAD");
        assert!(head_output.status.success(), "git rev-parse HEAD failed");
        let head_sha = String::from_utf8_lossy(&head_output.stdout).trim().to_owned();

        // Write .commit_hash into the temp track dir so the adapter uses HEAD as base.
        std::fs::write(track_temp.path().join(".commit_hash"), &head_sha).unwrap();

        let adapter = FsReviewGateStateAdapter::new(items_dir);
        let result = adapter.review_status(&track_id);

        drop(track_temp);

        // With diff HEAD..HEAD = empty and no review.json, evaluate_approval
        // returns Approved (all scopes Empty → no Required entries).
        assert!(
            matches!(result, Ok(ReviewGateStatus::Approved)),
            "expected Ok(Approved) for empty diff, got: {result:?}"
        );
    }

    // ── FsRefVerifyGateStateAdapter tests ─────────────────────────────────────

    /// When the track directory has no `spec.json` (zero Chain-1 pairs) and no
    /// TDDD layer catalogues (zero Chain-2 pairs), `ref_verify_status` must return
    /// `Ok(RefVerifyGateStatus::Approved)` (vacuous approval).
    ///
    /// Uses an isolated temp git repo via `seed_track_repo` so that git discovery
    /// works without depending on the host checkout state.
    #[test]
    fn test_fs_ref_verify_gate_state_adapter_returns_approved_when_no_production_pairs() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let track_id_str = "my-track-2026";
        let (dir, items_dir) = seed_track_repo(track_id_str);
        let root = dir.path();
        let _cwd = CwdGuard::enter(root);

        // Write minimal architecture-rules.json (no TDDD layers → no Chain-2 pairs).
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"placeholder-no-tddd"}]}"#,
        )
        .unwrap();

        let adapter = FsRefVerifyGateStateAdapter::new(items_dir);
        let track_id = TrackId::try_new(track_id_str.to_owned()).unwrap();
        let result = adapter.ref_verify_status(&track_id);

        assert!(
            matches!(result, Ok(RefVerifyGateStatus::Approved)),
            "expected Ok(Approved) when no production pairs, got: {result:?}"
        );
    }

    #[test]
    fn test_fs_ref_verify_gate_state_adapter_rejects_relative_items_dir_escape() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let parent = tempfile::tempdir().unwrap();
        let repo_root = parent.path().join("repo");
        let escaped_items_dir = parent.path().join("outside").join("track").join("items");
        std::fs::create_dir_all(&repo_root).unwrap();
        std::fs::create_dir_all(&escaped_items_dir).unwrap();
        run_git(&repo_root, &["init", "-q"]);

        let _cwd = CwdGuard::enter(&repo_root);
        let adapter = FsRefVerifyGateStateAdapter::new(PathBuf::from("../outside/track/items"));
        let track_id = TrackId::try_new("my-track-2026".to_owned()).unwrap();
        let result = adapter.ref_verify_status(&track_id);

        assert_gate_query_failed_contains(result, "outside the repository root");
    }

    /// When `items_dir` does not end in `track/items`, the adapter must return
    /// `Err(GateQueryFailed)` because the project root cannot be derived.
    #[test]
    fn test_fs_ref_verify_gate_state_adapter_returns_error_for_invalid_items_dir() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        // Not a `track/items`-style path.
        let bad_items_dir = root.join("wrong_path");
        std::fs::create_dir_all(&bad_items_dir).unwrap();
        let _cwd = CwdGuard::enter(root);

        let adapter = FsRefVerifyGateStateAdapter::new(bad_items_dir);
        let track_id = TrackId::try_new("my-track-2026".to_owned()).unwrap();
        let result = adapter.ref_verify_status(&track_id);

        assert!(
            matches!(result, Err(FixpointResolveError::GateQueryFailed { .. })),
            "expected GateQueryFailed for bad items_dir, got: {result:?}"
        );
    }

    /// When `items_dir` does not end in `track/items`, the review adapter must
    /// also return `Err(GateQueryFailed)`.
    #[test]
    fn test_fs_review_gate_state_adapter_returns_error_for_invalid_items_dir() {
        let _lock = cwd_lock().lock().expect("cwd lock must not be poisoned");
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        let bad_items_dir = root.join("wrong_path");
        std::fs::create_dir_all(&bad_items_dir).unwrap();
        let _cwd = CwdGuard::enter(root);

        let adapter = FsReviewGateStateAdapter::new(bad_items_dir);
        let track_id = TrackId::try_new("my-track-2026".to_owned()).unwrap();
        let result = adapter.review_status(&track_id);

        assert!(
            matches!(result, Err(FixpointResolveError::GateQueryFailed { .. })),
            "expected GateQueryFailed for bad items_dir, got: {result:?}"
        );
    }
}
