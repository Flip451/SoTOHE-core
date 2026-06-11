//! CLI handler for `sotp track archive`.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::CliApp;

use crate::CliError;

/// Execute `sotp track archive --track-id <id> [--items-dir <dir>]`.
///
/// Moves `<items_dir>/<track_id>/` to `<project_root>/track/archive/<track_id>/`
/// via `git mv` (for git-tracked content) and additionally moves any gitignored
/// `logs/` subdirectory via a filesystem rename so that telemetry is preserved
/// alongside the archived track (CN-03 / GO-03).
///
/// Missing `logs/` is the normal case (no telemetry was written) — handled silently.
///
/// # Errors
///
/// Returns `CliError::Message` when the archive operation fails (git mv error,
/// destination already exists, track not found, etc.).
pub(super) fn execute_archive(items_dir: PathBuf, track_id: String) -> Result<ExitCode, CliError> {
    let app = CliApp::new();
    let outcome = app.track_archive(items_dir, track_id).map_err(CliError::Message)?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::fs;

    use super::*;
    use crate::commands::track::test_support::{run_git, seed_repo};

    fn write_tracked_file(track_dir: &std::path::Path) {
        fs::write(track_dir.join("tracked.txt"), "archive fixture\n").unwrap();
    }

    /// Happy-path: archive moves the track dir and the gitignored `logs/` dir.
    ///
    /// CN-03 / GO-03: archived track's `logs/telemetry.jsonl` must be present
    /// under `track/archive/<id>/logs/` after the operation.
    #[test]
    fn test_archive_moves_track_dir_and_logs_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Set up a minimal git repository on a non-track branch so archive is not
        // write-guarded by the branch check used in other write operations.
        seed_repo(root, "main");

        let items_dir = root.join("track").join("items");
        let track_id = "my-track-2026";
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_tracked_file(&track_dir);

        // Create the gitignored logs/ subdir with a telemetry file.
        let logs_dir = track_dir.join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let telemetry_path = logs_dir.join("telemetry.jsonl");
        fs::write(&telemetry_path, r#"{"event_type":"TrackSubcommand"}"#).unwrap();

        // Stage the git-tracked content (not logs/, which is gitignored).
        run_git(root, &["add", "track/items/my-track-2026/tracked.txt"]);
        run_git(root, &["commit", "-m", "add track", "--no-gpg-sign"]);

        // Create the archive root directory.
        let archive_items_dir = root.join("track").join("archive");
        fs::create_dir_all(&archive_items_dir).unwrap();

        let result = execute_archive(items_dir, track_id.to_owned());
        assert!(result.is_ok(), "execute_archive must succeed: {result:?}");
        assert_eq!(result.unwrap(), ExitCode::SUCCESS);

        // The track dir must now be under archive/.
        let archived_dir = archive_items_dir.join(track_id);
        assert!(
            archived_dir.is_dir(),
            "track dir must exist under track/archive/ after archive: {archived_dir:?}"
        );
        assert!(
            archived_dir.join("tracked.txt").is_file(),
            "tracked file must be present in archived track dir"
        );

        // The gitignored logs/ must have been moved alongside the track dir.
        let archived_logs = archived_dir.join("logs").join("telemetry.jsonl");
        assert!(
            archived_logs.is_file(),
            "telemetry.jsonl must be present under archived track/logs/ after archive: {archived_logs:?}"
        );

        // The original logs/ must no longer exist at the source location.
        assert!(
            !telemetry_path.exists(),
            "telemetry.jsonl must not remain at the source location after archive"
        );
    }

    /// Normal case: archive without a `logs/` subdir must succeed silently.
    ///
    /// CN-03: missing `logs/` is the normal pre-telemetry case.
    #[test]
    fn test_archive_without_logs_subdir_succeeds_silently() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        seed_repo(root, "main");

        let items_dir = root.join("track").join("items");
        let track_id = "no-logs-track-2026";
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_tracked_file(&track_dir);

        run_git(root, &["add", "track/items/no-logs-track-2026/tracked.txt"]);
        run_git(root, &["commit", "-m", "add track", "--no-gpg-sign"]);

        let archive_items_dir = root.join("track").join("archive");
        fs::create_dir_all(&archive_items_dir).unwrap();

        let result = execute_archive(items_dir, track_id.to_owned());
        assert!(result.is_ok(), "archive without logs/ must succeed: {result:?}");
        assert_eq!(result.unwrap(), ExitCode::SUCCESS);

        let archived_dir = archive_items_dir.join(track_id);
        assert!(archived_dir.is_dir(), "archived dir must exist: {archived_dir:?}");
        // No logs/ subdir — that is expected and silent.
        assert!(
            !archived_dir.join("logs").exists(),
            "logs/ must not be created when source logs/ did not exist"
        );
    }
}
