//! Compatibility entry point for `sotp track set-commit-hash`.

use crate::CommandOutcome;
use crate::error::CompositionError;
use crate::track::composition_root::TrackCompositionRoot;

impl TrackCompositionRoot {
    /// Delegate the legacy composition call to the wired primary-adapter driver.
    ///
    /// # Errors
    ///
    pub fn track_set_commit_hash(
        &self,
        track_id: &str,
    ) -> Result<CommandOutcome, CompositionError> {
        let input =
            track_id.parse::<cli_driver::adr_baseline::TrackIdInput>().map_err(|error| {
                CompositionError::WiringFailed(format!("invalid track id: {error}"))
            })?;
        Ok(self.track_driver().handle_set_commit_hash(input))
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

    use crate::track::composition_root::TrackCompositionRoot;

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
    fn test_track_set_commit_hash_with_invalid_track_id_returns_wiring_error() {
        let app = TrackCompositionRoot::new();
        let result = app.track_set_commit_hash("../evil");
        let error = match result {
            Ok(outcome) => panic!("invalid track id must fail at wiring, got {outcome:?}"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains("invalid track id"), "got: {message}");
        assert!(
            !message.contains("[set-commit-hash] ERROR"),
            "composition must not template driver presentation: {message}"
        );
        assert!(
            !message.contains("Recovery:"),
            "composition must not template the driver recovery hint: {message}"
        );
    }

    #[test]
    fn test_track_set_commit_hash_call_site_preserves_cli_contract_across_migration() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        seed_track_repo(dir.path());

        let track_dir = dir.path().join("track").join("items").join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();

        let argv_track_id = "my-track-2026".to_owned();
        let outcome = from_working_dir(dir.path(), || {
            let app = TrackCompositionRoot::new();
            let result = app.track_set_commit_hash(&argv_track_id);
            assert!(result.is_ok(), "method must return Ok(outcome): {result:?}");
            result.unwrap()
        });
        assert_eq!(outcome.exit_code, 0, "happy path must succeed, stderr: {:?}", outcome.stderr);
        assert_eq!(argv_track_id, "my-track-2026");
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|stdout| { stdout.contains("Recorded .commit_hash") })
        );
        assert!(
            outcome.stderr.as_deref().is_some_and(|stderr| {
                stderr.contains("[set-commit-hash] Recorded .commit_hash")
            })
        );

        let commit_hash_path = track_dir.join(".commit_hash");
        assert!(commit_hash_path.exists(), ".commit_hash must be written on success");

        let written = std::fs::read_to_string(&commit_hash_path).unwrap();
        assert_eq!(written.trim().len(), 40, "written SHA must be 40 hex chars");
    }
}
