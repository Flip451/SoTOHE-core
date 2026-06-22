//! `sotp track set-commit-hash` — persist the current HEAD SHA to `.commit_hash`.

use std::process::ExitCode;

use cli_composition::CliApp;

use crate::CliError;

/// Persist the current HEAD SHA to `.commit_hash` for the active track.
///
/// Delegates to `CliApp::track_set_commit_hash`, which encapsulates all domain
/// type construction and I/O.  On failure the composition layer emits the error
/// and recovery hint to stderr, and the process exits with a non-zero code.
///
/// # Errors
///
/// Returns `CliError` when the composition layer itself returns an unexpected
/// `Err` (distinct from a failure `CommandOutcome`).
pub fn execute_set_commit_hash(track_id: String) -> Result<ExitCode, CliError> {
    let app = CliApp::new();
    let outcome =
        app.track_set_commit_hash(&track_id).map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::process::ExitCode;

    use super::*;
    use crate::commands::track::test_support::{
        capture_stderr, create_track_dir, process_env_lock, run_in_dir, seed_repo,
    };

    #[test]
    fn test_execute_set_commit_hash_wrong_branch_returns_failure_and_single_recovery_hint() {
        let _guard = process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        seed_repo(dir.path(), "main");
        create_track_dir(dir.path(), "my-track-2026");

        let (result, stderr) = run_in_dir(dir.path(), || {
            capture_stderr(|| execute_set_commit_hash("my-track-2026".to_owned()))
        });

        assert_eq!(result.unwrap(), ExitCode::FAILURE);
        assert!(
            stderr.contains("[set-commit-hash] ERROR: current branch 'main'"),
            "stderr must include the composition-layer error, got: {stderr}"
        );
        assert!(
            stderr.contains("Recovery: run `bin/sotp track set-commit-hash`"),
            "stderr must include the recovery hint, got: {stderr}"
        );
        let branch_error_lines =
            stderr.lines().filter(|line| line.contains("current branch 'main'")).count();
        assert_eq!(
            branch_error_lines, 1,
            "CLI must not re-print the bare outcome stderr after composition emits it: {stderr}"
        );
    }
}
