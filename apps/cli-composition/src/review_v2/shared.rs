//! Shared v2 review composition types, builders, and diff-base resolution.

use std::path::{Path, PathBuf};
use std::time::Instant;

use domain::review_v2::{CommitHashReader, FilePath, ReviewScopeConfig};
use domain::{CommitHash, TrackId};

use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::review_v2::{
    ClaudeReviewer, CodexReviewer, FsCommitHashStore, FsReviewStore, GitDiffGetter,
    SystemReviewHasher, load_v2_scope_config,
};
use thiserror::Error;
use usecase::review_v2::{
    ReviewCycle, ScopeQueryInteractor, error::DiffGetError, ports::DiffGetter,
};

use super::null_reviewer::NullReviewer;

/// Typed error for shared v2 review composition helpers.
///
/// Replaces the stringly-typed error boundary previously used by the internal
/// builder functions in this module. All variants carry a human-readable
/// message so callers can convert to string via `.to_string()` when needed.
#[derive(Debug, PartialEq, Error)]
pub enum ReviewSharedError {
    /// Git repository discovery or operation failed.
    #[error("{0}")]
    Git(String),

    /// Filesystem or path operation failed.
    #[error("{0}")]
    Path(String),

    /// Configuration loading or parsing failed.
    #[error("{0}")]
    Config(String),

    /// Invalid input (e.g. bad track-id, invalid commit hash).
    #[error("{0}")]
    InvalidInput(String),
}

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
    /// The scope had no diff files — review was skipped (no subprocess ran).
    Skipped { scope_label: String },
    /// A final review completed: verdict JSON to emit and the exit code (0 or 2).
    FinalCompleted {
        verdict_json: String,
        exit_code: u8,
        findings_count: u32,
        subprocess_started_at: Instant,
    },
    /// A fast review completed: verdict JSON to emit and the exit code (0 or 2).
    FastCompleted {
        verdict_json: String,
        exit_code: u8,
        findings_count: u32,
        subprocess_started_at: Instant,
    },
    /// The reviewer subprocess was launched but failed.
    ///
    /// `verdict_parse_failed` is `true` only when the subprocess ran successfully but
    /// its stdout could not be parsed as a valid verdict (e.g. `ReviewerError::IllegalVerdict`
    /// / `DryCheckAgentError::IllegalOutput`). For all other failures (timeout, abort,
    /// file-changed race, record-write error) it is `false`.
    ///
    /// `findings_count` carries the actual count from the subprocess when the verdict was
    /// successfully parsed before failure (e.g. persistence failure after a good verdict).
    /// It is `0` for failures where no verdict was obtained.
    ///
    /// Distinct from `Err(String)` returned by `run_codex_review_str` /
    /// `run_claude_review_str`, which represents pre-subprocess failures (diff/hash
    /// computation, scope lookup, composition build) that never launched the subprocess.
    SubprocessFailed {
        error: String,
        round_type: String,
        verdict_parse_failed: bool,
        findings_count: u32,
        subprocess_started_at: Instant,
    },
}

/// Shared setup logic for both `build_review_v2` and `build_review_v2_with_reviewer`.
///
/// Returns `(scope_config, review_store, commit_hash_store, base)`.
///
/// # Errors
/// Returns [`ReviewSharedError`] on failure.
pub(super) fn build_v2_shared(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<(ReviewScopeConfig, FsReviewStore, FsCommitHashStore, CommitHash), ReviewSharedError> {
    let git = discover_repo_from_items_dir(items_dir)?;
    let root = git.root().to_path_buf();

    let canonical_root = root.canonicalize().map_err(|e| {
        ReviewSharedError::Path(format!("failed to canonicalize repo root {}: {e}", root.display()))
    })?;
    let items_dir_abs =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { root.join(items_dir) };
    // Canonicalize directly: resolve all symlinks and `..` components together.
    // `normalize_path_components` + partial-walk is unsound when intermediate path
    // components are symlinks (the `..` is stripped before the symlink is resolved,
    // so traversal can escape the repo root without detection).
    // If `items_dir` does not exist on the filesystem, we cannot verify containment
    // and treat it as if it were outside the repository root (fail-closed).
    let canonical_items_dir = items_dir_abs.canonicalize().map_err(|_| {
        ReviewSharedError::Path(format!(
            "items_dir '{}' is outside the repository root '{}' or does not exist. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        ))
    })?;
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(ReviewSharedError::Path(format!(
            "items_dir '{}' is outside the repository root '{}'. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        )));
    }

    let track_dir = items_dir_abs.join(track_id.as_ref());
    if !track_dir.is_dir() {
        return Err(ReviewSharedError::Path(format!(
            "track directory '{}' does not exist. \
             Check --track-id '{}' and --items-dir '{}'.",
            track_dir.display(),
            track_id.as_ref(),
            items_dir.display(),
        )));
    }

    // Use the canonicalized repo root as the trusted_root for symlink guards:
    // `canonicalize()` resolves symlinks and returns the physical path, so
    // `canonical_root` is guaranteed non-symlink and safe as a trusted root.
    let scope_json_path = canonical_root.join(".harness/config/review-scope.json");
    let scope_config = load_v2_scope_config(&scope_json_path, track_id, &canonical_root)
        .map_err(|e| ReviewSharedError::Config(format!("load review-scope.json: {e}")))?;

    let review_json_path = track_dir.join("review.json");
    let commit_hash_path = track_dir.join(".commit_hash");
    let review_store = FsReviewStore::new(review_json_path, canonical_root.clone());
    let commit_hash_store = FsCommitHashStore::new(commit_hash_path, canonical_root.clone());

    let base = with_repo_cwd(&canonical_root, || resolve_diff_base(&commit_hash_store, &git))?;

    Ok((scope_config, review_store, commit_hash_store, base))
}

fn discover_repo_from_items_dir(items_dir: &Path) -> Result<SystemGitRepo, ReviewSharedError> {
    if let Ok(repo) = SystemGitRepo::discover_from(items_dir) {
        return Ok(repo);
    }
    if let Ok(project_root) = crate::track::resolve_project_root(items_dir) {
        if let Ok(repo) = SystemGitRepo::discover_from(&project_root) {
            return Ok(repo);
        }
    }
    SystemGitRepo::discover().map_err(|e| ReviewSharedError::Git(format!("git discover: {e}")))
}

pub(super) fn repo_root_from_items_dir(items_dir: &Path) -> Result<PathBuf, ReviewSharedError> {
    Ok(discover_repo_from_items_dir(items_dir)?.root().to_path_buf())
}

struct RepoCwdGuard {
    original: PathBuf,
    restored: bool,
}

impl RepoCwdGuard {
    fn change_to(repo_root: &Path) -> Result<Self, ReviewSharedError> {
        let original = std::env::current_dir().map_err(|e| {
            ReviewSharedError::Path(format!("failed to read current directory: {e}"))
        })?;
        std::env::set_current_dir(repo_root).map_err(|e| {
            ReviewSharedError::Path(format!(
                "failed to enter repo root {}: {e}",
                repo_root.display()
            ))
        })?;
        Ok(Self { original, restored: false })
    }

    fn restore(&mut self) -> Result<(), ReviewSharedError> {
        if self.restored {
            return Ok(());
        }
        std::env::set_current_dir(&self.original).map_err(|e| {
            ReviewSharedError::Path(format!(
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

pub(super) fn with_repo_cwd<T>(
    repo_root: &Path,
    f: impl FnOnce() -> Result<T, ReviewSharedError>,
) -> Result<T, ReviewSharedError> {
    let mut guard = RepoCwdGuard::change_to(repo_root)?;
    let result = f();
    if let Err(e) = guard.restore() {
        eprintln!("[warn] {e}");
    }
    result
}

/// Resolves the diff base commit hash.
pub(super) fn resolve_diff_base(
    store: &FsCommitHashStore,
    git: &SystemGitRepo,
) -> Result<CommitHash, ReviewSharedError> {
    match store.read() {
        Ok(Some(hash)) => return Ok(hash),
        Ok(None) => {}
        Err(e) => {
            eprintln!("[warn] failed to read .commit_hash, falling back to main: {e}");
        }
    }

    let output = git
        .output(&["rev-parse", "main"])
        .map_err(|e| ReviewSharedError::Git(format!("git rev-parse main: {e}")))?;
    if !output.status.success() {
        return Err(ReviewSharedError::Git("git rev-parse main failed".to_owned()));
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    CommitHash::try_new(&sha)
        .map_err(|e| ReviewSharedError::InvalidInput(format!("invalid main SHA: {e}")))
}

/// Builds the v2 review composition with a real `CodexReviewer`.
///
/// # Errors
/// Returns [`ReviewSharedError`] on failure.
#[allow(dead_code)]
pub(crate) fn build_review_v2_with_reviewer(
    track_id: &TrackId,
    items_dir: &Path,
    reviewer: CodexReviewer,
) -> Result<ReviewV2CompositionWithCodex, ReviewSharedError> {
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
/// Returns [`ReviewSharedError`] on failure.
pub(crate) fn build_review_v2(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<ReviewV2Composition, ReviewSharedError> {
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
/// Returns [`ReviewSharedError`] on failure.
pub fn build_review_v2_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewV2Composition, ReviewSharedError> {
    let track_id = TrackId::try_new(track_id_str)
        .map_err(|e| ReviewSharedError::InvalidInput(format!("invalid --track-id: {e}")))?;
    build_review_v2(&track_id, items_dir)
}

/// Builds the v2 review composition with a real `ClaudeReviewer`.
///
/// # Errors
/// Returns [`ReviewSharedError`] on failure.
pub(crate) fn build_review_v2_with_claude_reviewer(
    track_id: &TrackId,
    items_dir: &Path,
    reviewer: ClaudeReviewer,
) -> Result<ReviewV2CompositionWithClaude, ReviewSharedError> {
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
/// Returns [`ReviewSharedError`] on failure.
#[allow(dead_code)] // used in run.rs tests only
pub(crate) fn build_review_v2_with_claude_reviewer_str(
    track_id_str: &str,
    items_dir: &Path,
    reviewer: ClaudeReviewer,
) -> Result<ReviewV2CompositionWithClaude, ReviewSharedError> {
    let track_id = TrackId::try_new(track_id_str)
        .map_err(|e| ReviewSharedError::InvalidInput(format!("invalid --track-id: {e}")))?;
    build_review_v2_with_claude_reviewer(&track_id, items_dir, reviewer)
}

/// Resolves the diff base + diff getter for `sotp review files`.
///
/// # Errors
/// Returns [`ReviewSharedError`] on failure.
pub(crate) fn resolve_diff_base_and_getter(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<(GitDiffGetter, CommitHash), ReviewSharedError> {
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
/// Returns [`ReviewSharedError`] on failure.
pub(crate) fn build_scope_query_interactor_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ScopeQueryInteractor<GitDiffGetter>, ReviewSharedError> {
    use super::scope::load_scope_config_only;

    let track_id = TrackId::try_new(track_id_str)
        .map_err(|e| ReviewSharedError::InvalidInput(format!("invalid --track-id: {e}")))?;
    let (diff_getter, base) = resolve_diff_base_and_getter(&track_id, items_dir)?;
    let scope_config =
        load_scope_config_only(&track_id, items_dir).map_err(ReviewSharedError::Config)?;
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
/// Returns [`ReviewSharedError`] on failure (invalid track id,
/// scope-config load failure, items_dir traversal guard).
pub(crate) fn build_scope_query_interactor_no_diff_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ScopeQueryInteractor<NullDiffGetter>, ReviewSharedError> {
    use super::scope::load_scope_config_only;

    let track_id = TrackId::try_new(track_id_str)
        .map_err(|e| ReviewSharedError::InvalidInput(format!("invalid --track-id: {e}")))?;
    let scope_config =
        load_scope_config_only(&track_id, items_dir).map_err(ReviewSharedError::Config)?;
    let placeholder_base = CommitHash::try_new("0".repeat(40)).map_err(|e| {
        ReviewSharedError::InvalidInput(format!(
            "internal: placeholder commit hash construction failed: {e}"
        ))
    })?;
    Ok(ScopeQueryInteractor::new(scope_config, NullDiffGetter, placeholder_base))
}

// ---------------------------------------------------------------------------
// Profile-loading helpers shared across review commands
// ---------------------------------------------------------------------------

/// Discovers the git repository root, loads `agent-profiles.json`, and
/// returns the parsed `AgentProfiles`.
///
/// When `items_dir` is `Some`, git discovery is anchored to the project root
/// derived from `items_dir` (stripping the trailing `track/items` segments) so
/// that launchers or wrappers that `cd` to an arbitrary working directory do not
/// break profile loading when the target repository is discoverable from
/// `items_dir`. When `items_dir` is `None`, falls back to CWD-based discovery
/// (legacy behaviour for callers that do not have an `items_dir`).
///
/// # Errors
/// Returns `Err` when the repository cannot be discovered or the profiles
/// file is missing / malformed.
pub(crate) fn load_agent_profiles_from_repo(
    items_dir: Option<&Path>,
) -> Result<infrastructure::agent_profiles::AgentProfiles, ReviewSharedError> {
    let repo = if let Some(dir) = items_dir {
        let project_root = crate::track::resolve_project_root(dir)
            .map_err(|e| ReviewSharedError::Path(e.to_string()))?;
        SystemGitRepo::discover_from(&project_root).map_err(|e| {
            ReviewSharedError::Git(format!("[ERROR] failed to discover git repository root: {e}"))
        })?
    } else {
        SystemGitRepo::discover().map_err(|e| {
            ReviewSharedError::Git(format!("[ERROR] failed to discover git repository root: {e}"))
        })?
    };
    let profiles_path = repo.root().join(infrastructure::agent_profiles::AGENT_PROFILES_PATH);
    infrastructure::agent_profiles::AgentProfiles::load(&profiles_path).map_err(|e| {
        ReviewSharedError::Config(format!("[ERROR] failed to load agent-profiles.json: {e}"))
    })
}

pub(crate) struct ResolvedAgentExecution {
    pub(crate) provider: String,
    pub(crate) model: String,
}

/// Resolves an agent capability execution from `agent-profiles.json`, applying
/// the caller's optional model override.
///
/// # Errors
/// Returns `Err` when profiles cannot be loaded, the capability is missing, or
/// neither the caller nor profile supplies a model.
pub(crate) fn resolve_agent_execution(
    items_dir: Option<&Path>,
    capability: &str,
    round_type: infrastructure::agent_profiles::RoundType,
    model_override: Option<&str>,
) -> Result<ResolvedAgentExecution, ReviewSharedError> {
    let profiles = load_agent_profiles_from_repo(items_dir)?;
    let resolved = profiles.resolve_execution(capability, round_type).ok_or_else(|| {
        ReviewSharedError::Config(format!(
            "[ERROR] {capability} capability not defined in agent-profiles.json"
        ))
    })?;
    let model =
        model_override.map(str::to_owned).or_else(|| resolved.model.clone()).ok_or_else(|| {
            ReviewSharedError::Config(format!(
                "[ERROR] no model specified: pass --model or set model in agent-profiles.json \
                 {capability} capability"
            ))
        })?;
    Ok(ResolvedAgentExecution { provider: resolved.provider.clone(), model })
}

/// Parses a `round_type` string (`"fast"` or `"final"`) into the infra
/// `RoundType` enum.
///
/// Delegates to `RoundType`'s `FromStr` implementation so the accepted values
/// and error format are defined in a single place.
///
/// # Errors
/// Returns `Err` with a human-readable message for any unrecognised value.
pub(crate) fn parse_round_type(
    s: &str,
) -> Result<infrastructure::agent_profiles::RoundType, ReviewSharedError> {
    s.parse().map_err(ReviewSharedError::InvalidInput)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use super::{ReviewSharedError, with_repo_cwd};

    struct CwdRestore(PathBuf);

    impl Drop for CwdRestore {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_with_repo_cwd_restore_failure_preserves_primary_result() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let saved_cwd = std::env::current_dir().unwrap();
        let _cwd_restore = CwdRestore(saved_cwd);
        let original = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let original_path = original.path().to_path_buf();

        std::env::set_current_dir(&original_path).unwrap();

        let result = with_repo_cwd(repo.path(), || {
            std::fs::remove_dir_all(&original_path).unwrap();
            Ok::<u32, ReviewSharedError>(42)
        });

        assert_eq!(result, Ok(42));
    }
}
