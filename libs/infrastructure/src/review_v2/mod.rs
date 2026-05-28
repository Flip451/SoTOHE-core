//! Review System v2 infrastructure adapters.
//!
//! Implements usecase and domain port traits using git CLI and filesystem I/O.

pub mod claude_reviewer;
pub mod codex_reviewer;
pub mod diff_getter;
pub mod hasher;
pub mod persistence;
pub mod scope_config_loader;

pub use claude_reviewer::ClaudeReviewer;
pub use codex_reviewer::CodexReviewer;
pub use diff_getter::GitDiffGetter;
pub use hasher::SystemReviewHasher;
pub use persistence::{FsCommitHashStore, FsReviewStore};
pub use scope_config_loader::{ScopeConfigLoadError, load_v2_scope_config};

/// Persists the current HEAD SHA to `.commit_hash` for the given track (v2 incremental diff base).
///
/// This function encapsulates all domain type construction (`TrackId`, `CommitHash`,
/// `CommitHashWriter`) so that the CLI layer (`commands/make.rs`) does not need to import
/// `domain::CommitHash`, `domain::TrackId`, or `domain::review_v2::CommitHashWriter`
/// directly (CN-01 / AC-03).
///
/// Discovers the git root internally and uses `<root>/track/items` as the items directory.
///
/// Returns `Ok(head_sha)` on success (the SHA string that was written), or `Err(message)`
/// on any failure (validation, git, or I/O).
///
/// # Errors
///
/// Returns an error string describing the failure (invalid track id, branch mismatch,
/// git failure, or I/O error).
pub fn persist_commit_hash_for_track(track_id: &str) -> Result<String, String> {
    use crate::git_cli::{GitRepository, SystemGitRepo};
    use domain::CommitHash;
    use domain::review_v2::CommitHashWriter;

    let validated_id =
        domain::TrackId::try_new(track_id).map_err(|e| format!("invalid track id: {e}"))?;

    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let root = git.root().to_path_buf();

    // Branch guard: prevent cross-track corruption.
    let branch_output = git
        .output(&["rev-parse", "--abbrev-ref", "HEAD"])
        .map_err(|e| format!("git rev-parse --abbrev-ref HEAD: {e}"))?;
    if !branch_output.status.success() {
        return Err("git rev-parse --abbrev-ref HEAD failed (cannot verify branch)".to_owned());
    }
    let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_owned();
    let expected = format!("track/{validated_id}");
    if branch != expected {
        return Err(format!(
            "current branch '{branch}' does not match track branch '{expected}'. \
             Run from the correct track branch to prevent cross-track corruption."
        ));
    }

    let head_output =
        git.output(&["rev-parse", "HEAD"]).map_err(|e| format!("git rev-parse HEAD: {e}"))?;
    if !head_output.status.success() {
        return Err("git rev-parse HEAD failed".to_owned());
    }
    let head_sha = String::from_utf8_lossy(&head_output.stdout).trim().to_owned();
    let commit_hash = CommitHash::try_new(&head_sha).map_err(|e| format!("{e}"))?;

    // Use the canonicalized repo root as the trusted_root for symlink guards:
    // `canonicalize()` resolves symlinks and returns the physical path, so
    // `canonical_root` is guaranteed non-symlink and safe as a trusted root.
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize repo root {}: {e}", root.display()))?;
    let items_dir = canonical_root.join("track").join("items");
    let track_dir = items_dir.join(validated_id.as_ref());
    if !track_dir.is_dir() {
        return Err(format!(
            "track directory '{}' does not exist. \
             Cannot write .commit_hash for non-existent track '{validated_id}'.",
            track_dir.display(),
        ));
    }
    let commit_hash_path = track_dir.join(".commit_hash");
    let store = FsCommitHashStore::new(commit_hash_path, canonical_root);
    store.write(&commit_hash).map_err(|e| format!("{e}"))?;

    Ok(head_sha)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::persist_commit_hash_for_track;

    /// Guard that restores the working directory when dropped.
    struct CwdGuard {
        original: std::path::PathBuf,
    }

    impl CwdGuard {
        fn change_to(path: &Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Initialize a minimal git repo at `path` on a specific branch.
    fn init_git_repo(path: &Path, branch: &str) {
        let run =
            |args: &[&str]| Command::new("git").args(args).current_dir(path).output().unwrap();
        run(&["init", "-b", branch]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);
        // Create an initial commit so HEAD and the branch ref are valid.
        let readme = path.join("README.md");
        std::fs::write(&readme, "test\n").unwrap();
        run(&["add", "README.md"]);
        run(&["commit", "-m", "init"]);
    }

    // Mutex so git-based tests don't race on CWD changes.
    static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    fn env_lock() -> &'static std::sync::Mutex<()> {
        ENV_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    #[test]
    fn test_persist_commit_hash_rejects_invalid_track_id() {
        // track_id validation happens before any git operation, so no git repo needed.
        let result = persist_commit_hash_for_track("../evil");
        assert!(result.is_err(), "invalid track id must be rejected: {result:?}");
        let err = result.unwrap_err();
        assert!(
            err.contains("invalid track id"),
            "error must mention invalid track id, got: {err}"
        );
    }

    #[test]
    fn test_persist_commit_hash_rejects_wrong_branch() {
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        // Initialize the repo on `main`, not `track/my-track-2026`.
        init_git_repo(dir.path(), "main");
        let _cwd = CwdGuard::change_to(dir.path());

        let result = persist_commit_hash_for_track("my-track-2026");
        assert!(result.is_err(), "wrong branch must be rejected: {result:?}");
        let err = result.unwrap_err();
        assert!(
            err.contains("does not match track branch"),
            "error must mention branch mismatch, got: {err}"
        );
    }

    #[test]
    fn test_persist_commit_hash_rejects_missing_track_dir() {
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        // Initialize the repo on the correct branch.
        init_git_repo(dir.path(), "track/my-track-2026");
        let _cwd = CwdGuard::change_to(dir.path());

        // Do NOT create track/items/my-track-2026 — the function must reject.
        let result = persist_commit_hash_for_track("my-track-2026");
        assert!(result.is_err(), "missing track dir must be rejected: {result:?}");
        let err = result.unwrap_err();
        assert!(
            err.contains("does not exist"),
            "error must mention missing track directory, got: {err}"
        );
    }

    #[test]
    fn test_persist_commit_hash_writes_commit_hash_on_happy_path() {
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        // Initialize the repo on the correct branch.
        init_git_repo(dir.path(), "track/my-track-2026");
        let _cwd = CwdGuard::change_to(dir.path());

        // Create the track directory so the function can write .commit_hash.
        let track_dir = dir.path().join("track").join("items").join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();

        let result = persist_commit_hash_for_track("my-track-2026");
        assert!(result.is_ok(), "happy path must succeed: {result:?}");
        let sha = result.unwrap();
        assert!(!sha.is_empty(), "returned SHA must be non-empty");
        assert_eq!(sha.len(), 40, "returned SHA must be a 40-char hex string, got: {sha}");

        // Verify the file was written.
        let commit_hash_path = track_dir.join(".commit_hash");
        assert!(commit_hash_path.exists(), ".commit_hash must be written on success");
        let written = std::fs::read_to_string(&commit_hash_path).unwrap();
        assert_eq!(written.trim(), sha, ".commit_hash content must match the returned SHA");
    }
}
