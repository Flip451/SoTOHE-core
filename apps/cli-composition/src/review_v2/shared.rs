//! Shared v2 review composition types, builders, and diff-base resolution.

use std::path::Path;

use domain::review_v2::{CommitHashReader, FilePath, ReviewScopeConfig};
use domain::{CommitHash, TrackId};

use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::review_v2::{
    ClaudeReviewer, CodexReviewer, FsCommitHashStore, FsReviewStore, GitDiffGetter,
    SystemReviewHasher, load_v2_scope_config,
};
use usecase::review_v2::{
    ReviewCycle, ScopeQueryInteractor, error::DiffGetError, ports::DiffGetter,
};

use super::null_reviewer::NullReviewer;

/// Stub `DiffGetter` for pure-logic use cases (e.g. `classify`) that do not
/// need diff listings. `list_diff_files` always returns an empty list.
pub(crate) struct NullDiffGetter;

impl DiffGetter for NullDiffGetter {
    fn list_diff_files(&self, _base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
        Ok(Vec::new())
    }
}

/// All v2 adapters needed for status/check-approved operations (NullReviewer).
///
/// Fields are intentionally private — callers access behaviour through the
/// string-accepting free functions in this module (`check_approved_str`,
/// `render_review_results_str`, etc.) so that concrete infrastructure types
/// (`ReviewCycle`, `FsReviewStore`, …) are not exposed to the CLI layer.
///
/// `commit_hash_store` is retained for future commands that need to read or
/// reset the diff base without going through the full composition.
#[allow(dead_code)]
pub struct ReviewV2Composition {
    pub(super) cycle: ReviewCycle<NullReviewer, SystemReviewHasher, GitDiffGetter>,
    pub(super) review_store: FsReviewStore,
    pub(super) commit_hash_store: FsCommitHashStore,
    pub(super) base: CommitHash,
}

/// All v2 adapters needed for actual review (CodexReviewer).
///
/// Fields are intentionally private — callers access behaviour through the
/// string-accepting free functions in this module (`run_codex_review_str`,
/// etc.) so that concrete infrastructure types are not exposed to the CLI layer.
///
/// `commit_hash_store` and `base` are retained for future commands that need
/// to read or reset the diff base without going through the full composition.
#[allow(dead_code)]
pub(crate) struct ReviewV2CompositionWithCodex {
    pub(super) cycle: ReviewCycle<CodexReviewer, SystemReviewHasher, GitDiffGetter>,
    pub(super) review_store: FsReviewStore,
    pub(super) commit_hash_store: FsCommitHashStore,
    pub(super) base: CommitHash,
}

/// All v2 adapters needed for actual review (ClaudeReviewer).
///
/// Fields are intentionally private — callers access behaviour through the
/// string-accepting free functions in this module (`run_claude_review_str`,
/// etc.) so that concrete infrastructure types are not exposed to the CLI layer.
///
/// `commit_hash_store` and `base` are retained for future commands that need
/// to read or reset the diff base without going through the full composition.
#[allow(dead_code)]
pub(crate) struct ReviewV2CompositionWithClaude {
    pub(super) cycle: ReviewCycle<ClaudeReviewer, SystemReviewHasher, GitDiffGetter>,
    pub(super) review_store: FsReviewStore,
    pub(super) commit_hash_store: FsCommitHashStore,
    pub(super) base: CommitHash,
}

/// Outcome of a single Codex review run dispatched via `run_codex_review_str`.
///
/// Returned by `run_codex_review_str` so the CLI can emit output without
/// importing `domain::review_v2::Verdict`, `FastVerdict`, or `ReviewOutcome`
/// directly (CN-01 / AC-03).
pub enum CodexReviewOutcome {
    /// The scope had no diff files — review was skipped.
    Skipped { scope_label: String },
    /// A final review completed: verdict JSON to emit and the exit code (0 or 2).
    FinalCompleted { verdict_json: String, exit_code: u8 },
    /// A fast review completed: verdict JSON to emit and the exit code (0 or 2).
    FastCompleted { verdict_json: String, exit_code: u8 },
}

/// Shared setup logic for both `build_review_v2` and `build_review_v2_with_reviewer`.
///
/// Returns `(scope_config, review_store, commit_hash_store, base)`.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub(super) fn build_v2_shared(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<(ReviewScopeConfig, FsReviewStore, FsCommitHashStore, CommitHash), String> {
    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let root = git.root().to_path_buf();

    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize repo root {}: {e}", root.display()))?;
    let items_dir_abs =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { root.join(items_dir) };
    // Canonicalize directly: resolve all symlinks and `..` components together.
    // `normalize_path_components` + partial-walk is unsound when intermediate path
    // components are symlinks (the `..` is stripped before the symlink is resolved,
    // so traversal can escape the repo root without detection).
    // If `items_dir` does not exist on the filesystem, we cannot verify containment
    // and treat it as if it were outside the repository root (fail-closed).
    let canonical_items_dir = items_dir_abs.canonicalize().map_err(|_| {
        format!(
            "items_dir '{}' is outside the repository root '{}' or does not exist. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        )
    })?;
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(format!(
            "items_dir '{}' is outside the repository root '{}'. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        ));
    }

    let track_dir = items_dir_abs.join(track_id.as_ref());
    if !track_dir.is_dir() {
        return Err(format!(
            "track directory '{}' does not exist. \
             Check --track-id '{}' and --items-dir '{}'.",
            track_dir.display(),
            track_id.as_ref(),
            items_dir.display(),
        ));
    }

    // Use the canonicalized repo root as the trusted_root for symlink guards:
    // `canonicalize()` resolves symlinks and returns the physical path, so
    // `canonical_root` is guaranteed non-symlink and safe as a trusted root.
    let scope_json_path = canonical_root.join("track/review-scope.json");
    let scope_config = load_v2_scope_config(&scope_json_path, track_id, &canonical_root)
        .map_err(|e| format!("load review-scope.json: {e}"))?;

    let review_json_path = track_dir.join("review.json");
    let commit_hash_path = track_dir.join(".commit_hash");
    let review_store = FsReviewStore::new(review_json_path, canonical_root.clone());
    let commit_hash_store = FsCommitHashStore::new(commit_hash_path, canonical_root.clone());

    let base = resolve_diff_base(&commit_hash_store, &git)?;

    Ok((scope_config, review_store, commit_hash_store, base))
}

/// Resolves the diff base commit hash.
pub(super) fn resolve_diff_base(
    store: &FsCommitHashStore,
    git: &SystemGitRepo,
) -> Result<CommitHash, String> {
    match store.read() {
        Ok(Some(hash)) => return Ok(hash),
        Ok(None) => {}
        Err(e) => {
            eprintln!("[warn] failed to read .commit_hash, falling back to main: {e}");
        }
    }

    let output =
        git.output(&["rev-parse", "main"]).map_err(|e| format!("git rev-parse main: {e}"))?;
    if !output.status.success() {
        return Err("git rev-parse main failed".to_owned());
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    CommitHash::try_new(&sha).map_err(|e| format!("invalid main SHA: {e}"))
}

/// Builds the v2 review composition with a real `CodexReviewer`.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub(crate) fn build_review_v2_with_reviewer(
    track_id: &TrackId,
    items_dir: &Path,
    reviewer: CodexReviewer,
) -> Result<ReviewV2CompositionWithCodex, String> {
    let (scope_config, review_store, commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    let cycle =
        ReviewCycle::new(base.clone(), scope_config, reviewer, GitDiffGetter, SystemReviewHasher);
    Ok(ReviewV2CompositionWithCodex { cycle, review_store, commit_hash_store, base })
}

/// Builds the v2 review composition.
///
/// 1. Discovers git root
/// 2. Validates that `items_dir` resolves under the repo root (path traversal guard)
/// 3. Loads review-scope.json → `ReviewScopeConfig`
/// 4. Reads `.commit_hash` → `CommitHash` (fallback: `git rev-parse main`)
/// 5. Constructs `FsReviewStore`, `FsCommitHashStore`
/// 6. Returns `ReviewCycle` with `NullReviewer` (status/check-approved only)
///
/// # Errors
/// Returns a human-readable error string on failure.
pub(crate) fn build_review_v2(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<ReviewV2Composition, String> {
    let (scope_config, review_store, commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    let cycle = ReviewCycle::new(
        base.clone(),
        scope_config,
        NullReviewer,
        GitDiffGetter,
        SystemReviewHasher,
    );
    Ok(ReviewV2Composition { cycle, review_store, commit_hash_store, base })
}

/// String-accepting variant of `build_review_v2`.
///
/// Converts `track_id_str` to `TrackId` and delegates. Callers that must not
/// import `domain::TrackId` use this entry point (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn build_review_v2_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewV2Composition, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    build_review_v2(&track_id, items_dir)
}

/// Builds the v2 review composition with a real `ClaudeReviewer`.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub(crate) fn build_review_v2_with_claude_reviewer(
    track_id: &TrackId,
    items_dir: &Path,
    reviewer: ClaudeReviewer,
) -> Result<ReviewV2CompositionWithClaude, String> {
    let (scope_config, review_store, commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    let cycle =
        ReviewCycle::new(base.clone(), scope_config, reviewer, GitDiffGetter, SystemReviewHasher);
    Ok(ReviewV2CompositionWithClaude { cycle, review_store, commit_hash_store, base })
}

/// String-accepting variant of `build_review_v2_with_claude_reviewer`.
///
/// Converts `track_id_str` to `TrackId` and delegates. Callers that must not
/// import `domain::TrackId` use this entry point (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
#[allow(dead_code)] // used in run.rs tests only
pub(crate) fn build_review_v2_with_claude_reviewer_str(
    track_id_str: &str,
    items_dir: &Path,
    reviewer: ClaudeReviewer,
) -> Result<ReviewV2CompositionWithClaude, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    build_review_v2_with_claude_reviewer(&track_id, items_dir, reviewer)
}

/// Resolves the diff base + diff getter for `sotp review files`.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub(crate) fn resolve_diff_base_and_getter(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<(GitDiffGetter, CommitHash), String> {
    let (_scope_config, _review_store, _commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    Ok((GitDiffGetter, base))
}

/// Builds a `ScopeQueryInteractor` from string `track_id` and `items_dir`.
///
/// Callers that must not import `domain::TrackId` or `domain::CommitHash` use
/// this entry point (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
pub(crate) fn build_scope_query_interactor_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ScopeQueryInteractor<GitDiffGetter>, String> {
    use super::scope::load_scope_config_only;

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let (diff_getter, base) = resolve_diff_base_and_getter(&track_id, items_dir)?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;
    Ok(ScopeQueryInteractor::new(scope_config, diff_getter, base))
}

/// Builds a `ScopeQueryInteractor` for pure-logic use (no diff I/O).
///
/// Uses [`NullDiffGetter`] and a placeholder `CommitHash`. Suitable for the
/// `sotp review classify` command which only calls
/// [`usecase::review_v2::ScopeQueryService::classify_by_strings`].
///
/// Skips the `.commit_hash` lookup and `git rev-parse main` fallback that the
/// full builder performs, so it works in repositories without a recorded diff
/// base.
///
/// # Errors
/// Returns a human-readable error string on failure (invalid track id,
/// scope-config load failure, items_dir traversal guard).
pub(crate) fn build_scope_query_interactor_no_diff_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ScopeQueryInteractor<NullDiffGetter>, String> {
    use super::scope::load_scope_config_only;

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;
    let placeholder_base = CommitHash::try_new("0".repeat(40))
        .map_err(|e| format!("internal: placeholder commit hash construction failed: {e}"))?;
    Ok(ScopeQueryInteractor::new(scope_config, NullDiffGetter, placeholder_base))
}
