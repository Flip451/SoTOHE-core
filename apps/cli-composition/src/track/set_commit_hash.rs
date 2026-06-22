//! `sotp track set-commit-hash` — persist the current HEAD SHA to `.commit_hash`.
//!
//! Encapsulates the `.commit_hash` write operation and failure-recovery hint output.
//! The underlying persist logic is provided by `review_v2::persist_commit_hash_for_track`.

use crate::error::CompositionError;
use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Persist the current HEAD SHA to `.commit_hash` for the given track.
    ///
    /// Delegates to `review_v2::persist_commit_hash_for_track` for all domain type
    /// construction and I/O operations.  On success, emits a confirmation line on
    /// stderr and returns `Ok(CommandOutcome::success)`.  On failure, emits the
    /// error together with a recovery hint and returns `Ok(CommandOutcome::failure)`.
    ///
    /// The `track_id` is the string form of the track identifier (e.g.
    /// `"my-feature-2026"`).  It must pass `domain::TrackId::try_new` validation
    /// and the current branch must be `track/<track_id>`; both are enforced inside
    /// `persist_commit_hash_for_track`.
    ///
    /// # Errors
    ///
    /// Returns `Err` only on unexpected internal failures that prevent even forming
    /// the outcome (currently not reachable — all failures are returned as
    /// `Ok(CommandOutcome::failure)`).
    pub fn track_set_commit_hash(
        &self,
        track_id: &str,
    ) -> Result<CommandOutcome, CompositionError> {
        match crate::review_v2::persist_commit_hash_for_track(track_id) {
            Ok(sha) => {
                eprintln!("[set-commit-hash] Recorded .commit_hash: {sha}");
                Ok(CommandOutcome::success(Some(format!("Recorded .commit_hash: {sha}"))))
            }
            Err(msg) => {
                eprintln!("[set-commit-hash] ERROR: {msg}");
                eprintln!(
                    "[set-commit-hash] Recovery: run `bin/sotp track set-commit-hash` \
                     to set the v2 diff base manually."
                );
                Ok(CommandOutcome::failure(Some(msg)))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    use std::path::Path;
    use std::process::Command;

    use crate::CliApp;

    fn seed_track_repo(path: &Path) {
        let init = Command::new("git").args(["init", "-q"]).current_dir(path).status().unwrap();
        assert!(init.success(), "git init failed with {init}");

        let checkout = Command::new("git")
            .args(["checkout", "-B", "track/my-track-2026"])
            .current_dir(path)
            .status()
            .unwrap();
        assert!(checkout.success(), "git checkout failed with {checkout}");

        let commit = Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@test.com",
                "commit",
                "--allow-empty",
                "-m",
                "init",
                "--no-gpg-sign",
            ])
            .current_dir(path)
            .status()
            .unwrap();
        assert!(commit.success(), "git commit failed with {commit}");
    }

    fn from_working_dir<T>(path: &Path, run: impl FnOnce() -> T) -> T {
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let result = catch_unwind(AssertUnwindSafe(run));
        std::env::set_current_dir(previous).unwrap();
        match result {
            Ok(value) => value,
            Err(payload) => resume_unwind(payload),
        }
    }

    #[test]
    fn test_track_set_commit_hash_with_invalid_track_id_returns_failure_outcome() {
        // track_id validation happens before any git operation.
        let app = CliApp::new();
        let result = app.track_set_commit_hash("../evil");
        assert!(result.is_ok(), "method must return Ok(outcome), not Err: {result:?}");
        let outcome = result.unwrap();
        assert_ne!(outcome.exit_code, 0, "invalid track id must produce failure exit code");
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(
            stderr.contains("invalid track id"),
            "stderr must mention invalid track id, got: {stderr}"
        );
    }

    #[test]
    fn test_track_set_commit_hash_on_correct_branch_writes_commit_hash() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        seed_track_repo(dir.path());

        let track_dir = dir.path().join("track").join("items").join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();

        let outcome = from_working_dir(dir.path(), || {
            let app = CliApp::new();
            let result = app.track_set_commit_hash("my-track-2026");
            assert!(result.is_ok(), "method must return Ok(outcome): {result:?}");
            result.unwrap()
        });
        assert_eq!(outcome.exit_code, 0, "happy path must succeed, stderr: {:?}", outcome.stderr);

        let commit_hash_path = track_dir.join(".commit_hash");
        assert!(commit_hash_path.exists(), ".commit_hash must be written on success");

        let written = std::fs::read_to_string(&commit_hash_path).unwrap();
        assert_eq!(written.trim().len(), 40, "written SHA must be 40 hex chars");
    }
}
