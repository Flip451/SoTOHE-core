//! Infrastructure adapters for review workflow port traits.
//!
//! - `RecordRoundProtocolImpl`: the genuinely complex two-phase git index
//!   commit protocol (PrivateIndex + stage + hash + swap).
//! - `SystemGitHasher`: thin delegation to `SystemGitRepo` for normalised hash.

use std::path::Path;

use domain::{ReviewConcern, ReviewGroupName, RoundType, Timestamp, TrackId, Verdict};
use usecase::review_workflow::usecases::{
    GitHasher, RecordRoundProtocol, RecordRoundProtocolError,
};

// ---------------------------------------------------------------------------
// GitHasher — thin delegation
// ---------------------------------------------------------------------------

/// Computes normalised git tree hashes via `SystemGitRepo`.
pub struct SystemGitHasher;

impl GitHasher for SystemGitHasher {
    fn normalized_hash(&self, items_dir: &Path, track_id: &TrackId) -> Result<String, String> {
        use crate::git_cli::{GitRepository, SystemGitRepo};

        let git = SystemGitRepo::discover().map_err(|e| format!("git error: {e}"))?;
        let metadata_abs = items_dir.join(track_id.as_ref()).join("metadata.json");
        let metadata_rel = metadata_abs
            .strip_prefix(git.root())
            .unwrap_or(&metadata_abs)
            .to_string_lossy()
            .into_owned();

        git.index_tree_hash_normalizing(&metadata_rel).map_err(|e| format!("{e}"))
    }
}

// ---------------------------------------------------------------------------
// RecordRoundProtocol — two-phase git index commit
// ---------------------------------------------------------------------------

/// Atomic two-phase record-round protocol using PrivateIndex.
pub struct RecordRoundProtocolImpl {
    pub items_dir: std::path::PathBuf,
    pub group_display: String,
}

/// Maximum retries for stale-hash conflicts from parallel recordings.
const RECORD_ROUND_MAX_RETRIES: u8 = 3;

impl RecordRoundProtocol for RecordRoundProtocolImpl {
    #[allow(clippy::too_many_lines)]
    fn execute(
        &self,
        track_id: &TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundProtocolError> {
        use domain::{ReviewRoundResult, ReviewState};

        use crate::git_cli::private_index::PrivateIndex;
        use crate::git_cli::{GitRepository, SystemGitRepo};
        use crate::track::fs_store::FsTrackStore;

        let git = SystemGitRepo::discover()
            .map_err(|e| RecordRoundProtocolError::Other(format!("git error: {e}")))?;

        let metadata_abs = self.items_dir.join(track_id.as_ref()).join("metadata.json");
        let metadata_rel = metadata_abs
            .strip_prefix(git.root())
            .unwrap_or(&metadata_abs)
            .to_string_lossy()
            .into_owned();

        let store = FsTrackStore::new(&self.items_dir);

        // Retry loop: parallel recordings can cause stale-hash conflicts.
        // On conflict, recreate PrivateIndex from the (now-updated) real index and retry.
        for attempt in 0..=RECORD_ROUND_MAX_RETRIES {
            // Acquire a repo-wide exclusive advisory lock that spans the entire
            // record-round protocol: PrivateIndex creation → metadata write → index swap.
            // Repo-wide (not per-track) because PrivateIndex::swap_into_real replaces
            // the shared .git/index — cross-track conflicts must also be serialized.
            let lock_path = git.root().join(".git").join("sotp-record-round.lock");
            let lock_file = std::fs::File::create(&lock_path).map_err(|e| {
                RecordRoundProtocolError::Other(format!(
                    "failed to create lock file {}: {e}",
                    lock_path.display()
                ))
            })?;
            {
                use fs4::fs_std::FileExt;
                lock_file.lock_exclusive().map_err(|e| {
                    RecordRoundProtocolError::Other(format!(
                        "failed to acquire lock on {}: {e}",
                        lock_path.display()
                    ))
                })?;
            }

            let private_index =
                PrivateIndex::from_current(&git).map_err(RecordRoundProtocolError::Other)?;

            let pre_update_hash =
                private_index.normalized_tree_hash(&git, &metadata_rel).map_err(|e| {
                    RecordRoundProtocolError::Other(format!("normalized hash error: {e}"))
                })?;

            // Read current track state directly (lock is already held by us).
            let (mut track, mut meta) = store
                .find_with_meta(track_id)
                .map_err(|e| RecordRoundProtocolError::Other(format!("read error: {e}")))?
                .ok_or_else(|| {
                    RecordRoundProtocolError::Other(format!("track {track_id} not found"))
                })?;

            let review = track.review_mut().get_or_insert_with(ReviewState::new);
            let round_num = review
                .groups()
                .get(&group_name)
                .and_then(|g| match round_type {
                    domain::RoundType::Fast => g.fast().map(|r| r.round()),
                    domain::RoundType::Final => g.final_round().map(|r| r.round()),
                })
                .map(|n| n.saturating_add(1))
                .unwrap_or(1);

            let result = if concerns.is_empty() {
                ReviewRoundResult::new(round_num, verdict, timestamp.clone())
            } else {
                ReviewRoundResult::new_with_concerns(
                    round_num,
                    verdict,
                    timestamp.clone(),
                    concerns.clone(),
                )
            };

            let mut stale_error: Option<String> = None;
            match review.record_round_with_pending(
                round_type,
                &group_name,
                result,
                &expected_groups,
                &pre_update_hash,
            ) {
                Ok(()) => {}
                Err(domain::ReviewError::EscalationActive { concerns: blocked }) => {
                    return Err(RecordRoundProtocolError::EscalationBlocked(blocked));
                }
                Err(domain::ReviewError::StaleCodeHash { expected, actual }) => {
                    stale_error = Some(format!(
                        "code hash mismatch: review recorded against {expected}, \
                         but current code is {actual}"
                    ));
                    // Don't persist — retry will reload clean state from disk.
                }
                Err(e) => {
                    return Err(RecordRoundProtocolError::Other(format!(
                        "record_round_with_pending: {e}"
                    )));
                }
            }

            if let Some(err_msg) = stale_error {
                if attempt < RECORD_ROUND_MAX_RETRIES {
                    let jitter_ms = {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut h = DefaultHasher::new();
                        std::process::id().hash(&mut h);
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos()
                            .hash(&mut h);
                        h.finish() % 200 + 50 // 50-249ms
                    };
                    eprintln!(
                        "[RETRY] Stale hash on attempt {attempt} — \
                         retrying in {jitter_ms}ms with fresh index"
                    );
                    // Release lock before sleeping so peers can make progress.
                    drop(lock_file);
                    std::thread::sleep(std::time::Duration::from_millis(jitter_ms));
                    continue;
                }
                // Final retry exhausted: persist the invalidated state so disk
                // reflects the domain's invalidation (not stale approved/fast_passed).
                meta.updated_at = timestamp.to_string();
                meta.original_status = None;
                if let Err(e) = store.write_track(&track, &meta) {
                    return Err(RecordRoundProtocolError::Other(format!(
                        "stale hash AND failed to persist invalidation: {err_msg}; write error: {e}"
                    )));
                }
                return Err(RecordRoundProtocolError::StaleHash(err_msg));
            }

            // Two-phase write: stage pending state → compute hash → set hash → stage final.
            meta.updated_at = timestamp.to_string();
            meta.original_status = None;

            let pending_json = crate::track::codec::encode(&track, &meta)
                .map_err(|e| RecordRoundProtocolError::Other(format!("codec encode: {e}")))?;
            private_index
                .stage_bytes(&git, &metadata_rel, format!("{pending_json}\n").as_bytes())
                .map_err(RecordRoundProtocolError::Other)?;

            let h1 = private_index
                .normalized_tree_hash(&git, &metadata_rel)
                .map_err(|e| RecordRoundProtocolError::Other(format!("post-hash: {e}")))?;

            if let Some(r) = track.review_mut().as_mut() {
                r.set_code_hash(h1)
                    .map_err(|e| RecordRoundProtocolError::Other(format!("set_code_hash: {e}")))?;
            }

            let final_json = crate::track::codec::encode(&track, &meta)
                .map_err(|e| RecordRoundProtocolError::Other(format!("codec final: {e}")))?;
            private_index
                .stage_bytes(&git, &metadata_rel, format!("{final_json}\n").as_bytes())
                .map_err(RecordRoundProtocolError::Other)?;

            // Write metadata.json to disk atomically (for other processes to read).
            store
                .write_track(&track, &meta)
                .map_err(|e| RecordRoundProtocolError::Other(format!("write_track: {e}")))?;

            private_index.swap_into_real().map_err(RecordRoundProtocolError::Other)?;

            eprintln!(
                "[OK] Recorded {round_type} round for group '{}' (verdict: {verdict})",
                self.group_display
            );
            return Ok(());
        } // end retry loop

        Err(RecordRoundProtocolError::Other("record-round: max retries exceeded".to_owned()))
    }
}

// ---------------------------------------------------------------------------
// GitDiffScopeProvider — Git-backed DiffScope adapter
// ---------------------------------------------------------------------------

use usecase::review_workflow::scope::{DiffScope, DiffScopeProviderError, RepoRelativePath};

/// Git-backed [`DiffScopeProvider`] using merge-base diff.
///
/// Computes the set of changed files by:
/// 1. Finding the merge-base between `HEAD` and `base_ref`.
/// 2. Diffing `HEAD` against that merge-base (`--diff-filter=ACDMRT`).
/// 3. Adding staged (cached) changes.
/// 4. Adding untracked (non-ignored) files.
pub struct GitDiffScopeProvider;

impl usecase::review_workflow::scope::DiffScopeProvider for GitDiffScopeProvider {
    fn changed_files(&self, base_ref: &str) -> Result<DiffScope, DiffScopeProviderError> {
        use crate::git_cli::{GitRepository, SystemGitRepo};

        let git = SystemGitRepo::discover()
            .map_err(|e| DiffScopeProviderError::Other(format!("git error: {e}")))?;

        // 1. Find merge-base between HEAD and base_ref.
        let merge_base_output = git
            .output(&["merge-base", "HEAD", base_ref])
            .map_err(|e| DiffScopeProviderError::Other(format!("merge-base failed: {e}")))?;

        if !merge_base_output.status.success() {
            return Err(DiffScopeProviderError::UnknownBaseRef { base_ref: base_ref.to_owned() });
        }

        let merge_base = String::from_utf8_lossy(&merge_base_output.stdout).trim().to_owned();

        let mut files = Vec::new();

        // Helper: collect paths from git output, propagating errors.
        let mut collect_paths =
            |output: std::process::Output, label: &str| -> Result<(), DiffScopeProviderError> {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                    return Err(DiffScopeProviderError::Other(format!(
                        "{label} failed (exit {}): {stderr}",
                        output.status.code().unwrap_or(-1)
                    )));
                }
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        if let Some(path) = RepoRelativePath::normalize(trimmed) {
                            files.push(path);
                        }
                    }
                }
                Ok(())
            };

        // 2. Files changed between merge-base and HEAD (committed, includes renames).
        let diff_output = git
            .output(&["diff", "--name-only", "--diff-filter=ACDMRT", &merge_base, "HEAD"])
            .map_err(|e| DiffScopeProviderError::Other(format!("diff failed: {e}")))?;
        collect_paths(diff_output, "diff merge-base..HEAD")?;

        // 3. Staged but uncommitted changes.
        let staged_output = git
            .output(&["diff", "--name-only", "--cached"])
            .map_err(|e| DiffScopeProviderError::Other(format!("staged diff failed: {e}")))?;
        collect_paths(staged_output, "diff --cached")?;

        // 4. Unstaged worktree modifications to tracked files.
        let worktree_output = git
            .output(&["diff", "--name-only"])
            .map_err(|e| DiffScopeProviderError::Other(format!("worktree diff failed: {e}")))?;
        collect_paths(worktree_output, "diff (worktree)")?;

        // 5. Untracked (non-ignored) files.
        let untracked_output = git
            .output(&["ls-files", "--others", "--exclude-standard"])
            .map_err(|e| DiffScopeProviderError::Other(format!("ls-files failed: {e}")))?;
        collect_paths(untracked_output, "ls-files --others")?;

        Ok(DiffScope::new(files))
    }
}

// ---------------------------------------------------------------------------
// GitDiffScopeProvider — contract tests with tempdir git fixtures
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::process::Command;
    use std::sync::Mutex;

    use usecase::review_workflow::scope::{DiffScopeProvider, DiffScopeProviderError};

    use super::*;

    // Tests that call `set_current_dir` MUST run serially to avoid interfering
    // with each other or with tests in other modules that depend on cwd.
    // We use a process-wide Mutex as a lightweight serial gate — any test that
    // changes cwd acquires this lock for the duration of the call.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Creates a temporary git repo with an initial commit on "main" and
    /// checks out a fresh "test-branch".  The returned `TempDir` must be kept
    /// alive for the duration of the test.
    fn setup_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        let run = |args: &[&str]| {
            let out = Command::new("git").args(args).current_dir(path).output().unwrap();
            assert!(
                out.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        };

        run(&["init"]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);

        // Initial file + commit.
        std::fs::write(path.join("README.md"), "initial").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "initial"]);
        // Ensure the default branch is named "main".
        run(&["branch", "-M", "main"]);
        // Create and switch to a test branch so that "main" is a valid base ref.
        run(&["checkout", "-b", "test-branch"]);

        dir
    }

    /// Runs `GitDiffScopeProvider::changed_files` with the cwd temporarily set
    /// to `dir`.  The `CWD_LOCK` must be held by the caller for the duration of
    /// this call.
    fn run_provider_in_dir(
        dir: &std::path::Path,
        base_ref: &str,
    ) -> Result<DiffScope, DiffScopeProviderError> {
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        let result = GitDiffScopeProvider.changed_files(base_ref);
        std::env::set_current_dir(original).unwrap();
        result
    }

    /// Returns `true` if `scope` contains a [`RepoRelativePath`] for `raw`.
    fn scope_contains(scope: &DiffScope, raw: &str) -> bool {
        RepoRelativePath::normalize(raw).is_some_and(|p| scope.contains(&p))
    }

    // -----------------------------------------------------------------------
    // Contract tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_diff_scope_includes_committed_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Commit a new file on the test branch.
        std::fs::write(path.join("new_feature.rs"), "pub fn hello() {}").unwrap();
        Command::new("git").args(["add", "new_feature.rs"]).current_dir(path).output().unwrap();
        Command::new("git")
            .args(["commit", "-m", "add new_feature"])
            .current_dir(path)
            .output()
            .unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "new_feature.rs"), "committed file should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_staged_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Stage a new file without committing.
        std::fs::write(path.join("staged.rs"), "// staged").unwrap();
        Command::new("git").args(["add", "staged.rs"]).current_dir(path).output().unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "staged.rs"), "staged file should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_unstaged_worktree_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Modify a tracked file without staging.
        std::fs::write(path.join("README.md"), "modified").unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(
            scope_contains(&scope, "README.md"),
            "unstaged worktree modification should appear in scope"
        );
    }

    #[test]
    fn test_diff_scope_includes_untracked_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Create a new file without staging it.
        std::fs::write(path.join("untracked.txt"), "not staged").unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "untracked.txt"), "untracked file should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_renamed_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Rename a tracked file and commit it (tests the merge-base..HEAD diff path).
        Command::new("git")
            .args(["mv", "README.md", "RENAMED.md"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git").args(["commit", "-m", "rename"]).current_dir(path).output().unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "RENAMED.md"), "renamed destination should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_deleted_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Delete the tracked file and commit it (tests the merge-base..HEAD diff path).
        Command::new("git").args(["rm", "README.md"]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "delete"]).current_dir(path).output().unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "README.md"), "deleted file should appear in scope");
    }

    #[test]
    fn test_diff_scope_error_on_invalid_base_ref() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        let result = run_provider_in_dir(path, "nonexistent-branch-xyz-999");
        match result {
            Err(DiffScopeProviderError::UnknownBaseRef { base_ref }) => {
                assert_eq!(base_ref, "nonexistent-branch-xyz-999");
            }
            other => panic!("expected UnknownBaseRef, got {other:?}"),
        }
    }

    #[test]
    fn test_diff_scope_empty_for_no_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // No changes from base — scope should be empty.
        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope.is_empty(), "scope should be empty when there are no branch changes");
    }
}
