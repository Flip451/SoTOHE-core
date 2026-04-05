//! v2 review system composition root.
//!
//! Builds `ReviewCycle` from concrete infrastructure adapters.

use std::path::Path;

use domain::review_v2::{CommitHashReader, ReviewScopeConfig};
use domain::{CommitHash, TrackId};
use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::review_v2::{
    CodexReviewer, FsCommitHashStore, FsReviewStore, GitDiffGetter, SystemReviewHasher,
    load_v2_scope_config,
};
use usecase::review_v2::ReviewCycle;

/// All v2 adapters needed for status/check-approved operations (NullReviewer).
#[allow(dead_code)] // Fields used incrementally as CLI commands are migrated to v2
pub(crate) struct ReviewV2Composition {
    pub(crate) cycle: ReviewCycle<NullReviewer, SystemReviewHasher, GitDiffGetter>,
    pub(crate) review_store: FsReviewStore,
    pub(crate) commit_hash_store: FsCommitHashStore,
    pub(crate) base: CommitHash,
}

/// All v2 adapters needed for actual review (CodexReviewer).
#[allow(dead_code)] // commit_hash_store and base used when further CLI commands are migrated
pub(crate) struct ReviewV2CompositionWithCodex {
    pub(crate) cycle: ReviewCycle<CodexReviewer, SystemReviewHasher, GitDiffGetter>,
    pub(crate) review_store: FsReviewStore,
    pub(crate) commit_hash_store: FsCommitHashStore,
    pub(crate) base: CommitHash,
}

/// Null reviewer — used when the composition only needs status/check-approved
/// (no actual review invocation). The Reviewer trait is required by ReviewCycle
/// but these operations only call `get_review_states()`.
pub(crate) struct NullReviewer;

impl usecase::review_v2::Reviewer for NullReviewer {
    fn review(
        &self,
        _target: &domain::review_v2::ReviewTarget,
    ) -> Result<
        (domain::review_v2::Verdict, domain::review_v2::LogInfo),
        usecase::review_v2::ReviewerError,
    > {
        Err(usecase::review_v2::ReviewerError::Unexpected(
            "NullReviewer: review() must not be called".to_owned(),
        ))
    }

    fn fast_review(
        &self,
        _target: &domain::review_v2::ReviewTarget,
    ) -> Result<
        (domain::review_v2::FastVerdict, domain::review_v2::LogInfo),
        usecase::review_v2::ReviewerError,
    > {
        Err(usecase::review_v2::ReviewerError::Unexpected(
            "NullReviewer: fast_review() must not be called".to_owned(),
        ))
    }
}

/// Builds the v2 review composition with a real `CodexReviewer`.
///
/// Same setup as `build_review_v2` but injects the provided `reviewer` into
/// `ReviewCycle` instead of `NullReviewer`. Use this path when an actual
/// review invocation is needed.
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

/// Shared setup logic for both `build_review_v2` and `build_review_v2_with_reviewer`.
///
/// Returns `(scope_config, review_store, commit_hash_store, base)`.
///
/// # Errors
/// Returns a human-readable error string on failure.
fn build_v2_shared(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<(ReviewScopeConfig, FsReviewStore, FsCommitHashStore, CommitHash), String> {
    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let root = git.root().to_path_buf();

    // Security: verify items_dir resolves to a path under the repo root.
    // This prevents --items-dir /tmp/.. or symlink-based path traversal from
    // reading/writing review state outside the repository.
    //
    // Strategy:
    // 1. Canonicalize the git root (symlinks resolved, absolute).
    // 2. Resolve items_dir to an absolute path relative to the repo root.
    // 3. Logically normalize ".." components.
    // 4. Canonicalize the deepest existing ancestor to resolve symlinks in the prefix.
    // 5. Check that the canonicalized ancestor starts_with(canonical_root).
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize repo root {}: {e}", root.display()))?;
    let items_dir_abs =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { root.join(items_dir) };
    let items_dir_resolved = normalize_path_components(&items_dir_abs);
    let canonical_items_dir = {
        let mut probe = items_dir_resolved.as_path();
        loop {
            match probe.canonicalize() {
                Ok(canonical) => {
                    let suffix = items_dir_resolved
                        .strip_prefix(probe)
                        .unwrap_or_else(|_| std::path::Path::new(""));
                    break canonical.join(suffix);
                }
                Err(_) => match probe.parent() {
                    Some(parent) => probe = parent,
                    None => break items_dir_resolved.clone(),
                },
            }
        }
    };
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(format!(
            "items_dir '{}' is outside the repository root '{}'. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        ));
    }

    // Fail-fast: verify the track directory exists before creating stores.
    // Without this check, a typo in --track-id silently creates orphan state.
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

    let scope_json_path = root.join("track/review-scope.json");
    let scope_config = load_v2_scope_config(&scope_json_path, track_id, &root)
        .map_err(|e| format!("load review-scope.json: {e}"))?;

    let review_json_path = track_dir.join("review.json");
    let commit_hash_path = track_dir.join(".commit_hash");
    let review_store = FsReviewStore::new(review_json_path, root.clone());
    let commit_hash_store = FsCommitHashStore::new(commit_hash_path, root.clone());

    let base = resolve_diff_base(&commit_hash_store, &git)?;

    Ok((scope_config, review_store, commit_hash_store, base))
}

/// Resolves the diff base commit hash.
///
/// Reads `.commit_hash` file. If it exists and contains a valid hash that is an
/// ancestor of HEAD, uses it. Otherwise falls back to `git rev-parse main`.
fn resolve_diff_base(store: &FsCommitHashStore, git: &SystemGitRepo) -> Result<CommitHash, String> {
    match store.read() {
        Ok(Some(hash)) => return Ok(hash),
        Ok(None) => {} // fallback
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

/// Logically resolves `..` and `.` components in a path without touching the filesystem.
fn normalize_path_components(path: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if matches!(components.last(), Some(Component::Normal(_))) {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}
