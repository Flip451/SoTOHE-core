use std::path::Path;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::track::TrackInput;

use crate::CliError;

use super::state_ops::track_driver_outcome_to_result;
use super::{ViewAction, resolve_track_id_from_root_for_write};

pub(super) fn execute_views(action: ViewAction) -> Result<ExitCode, CliError> {
    let driver = TrackCompositionRoot::new().track_driver();
    match action {
        ViewAction::Validate { project_root } => {
            let outcome = driver.handle(TrackInput::ViewsValidate { project_root });
            track_driver_outcome_to_result(outcome)
        }
        ViewAction::Sync { project_root, track_id } => {
            // When an explicit --track-id is given, the WRITE guard validates
            // that it matches the current git branch (AC-18, D7): a mismatch
            // is fail-closed. This prevents accidentally syncing views for a
            // different track than the one the developer is working on.
            let resolved_track_id = match track_id {
                Some(id) => {
                    let validated_id =
                        resolve_track_id_from_root_for_write(Some(id), &project_root)
                            .map_err(CliError::Message)?;
                    Some(validated_id)
                }
                None => detect_active_track_from_branch(&project_root),
            };
            let outcome =
                driver.handle(TrackInput::ViewsSync { project_root, track_id: resolved_track_id });
            track_driver_outcome_to_result(outcome)
        }
    }
}

/// Detect the active track id from the current git branch.
///
/// Only `track/<id>` branches are resolved; any other branch (e.g. `main`,
/// detached HEAD) or git failure resolves to `None` so the caller can fall
/// back to registry-only mode without surfacing an error.
fn detect_active_track_from_branch(project_root: &Path) -> Option<String> {
    TrackCompositionRoot::new().detect_active_track_from_branch(project_root)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Initialise a temporary git repository with a single commit on `main`
    /// and then check out `branch_name` (which may use a slash-separated
    /// namespace such as `track/foo` or `plan/bar`). Returns the path to
    /// the repo root so callers can hand it to `detect_active_track_from_branch`.
    fn init_repo_on_branch(tmp: &tempfile::TempDir, branch_name: &str) -> std::path::PathBuf {
        let root = tmp.path().to_path_buf();
        // Initialise a repo with `main` as the default branch so the branch
        // layout is deterministic across developer/CI git versions.
        let status = Command::new("git")
            .args(["init", "-q", "--initial-branch=main"])
            .current_dir(&root)
            .status()
            .expect("failed to spawn git init");
        assert!(status.success(), "git init failed");

        // Local identity and commit.gpgsign off so the seed commit succeeds
        // regardless of the developer's global git config.
        for args in [
            &["config", "user.email", "test@example.com"][..],
            &["config", "user.name", "Test User"][..],
            &["config", "commit.gpgsign", "false"][..],
        ] {
            let status = Command::new("git")
                .args(args)
                .current_dir(&root)
                .status()
                .expect("failed to spawn git config");
            assert!(status.success(), "git config failed");
        }

        std::fs::write(root.join(".seed"), b"seed\n").expect("write seed file");
        let status = Command::new("git")
            .args(["add", ".seed"])
            .current_dir(&root)
            .status()
            .expect("failed to spawn git add");
        assert!(status.success(), "git add failed");

        let status = Command::new("git")
            .args(["commit", "-qm", "seed"])
            .current_dir(&root)
            .status()
            .expect("failed to spawn git commit");
        assert!(status.success(), "git commit failed");

        // Switch to the target branch only if the caller asked for something
        // other than `main` — the initial branch is already `main`.
        if branch_name != "main" {
            let status = Command::new("git")
                .args(["switch", "-qc", branch_name])
                .current_dir(&root)
                .status()
                .expect("failed to spawn git switch");
            assert!(status.success(), "git switch failed");
        }

        root
    }

    #[test]
    fn detect_active_track_from_branch_resolves_track_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let root = init_repo_on_branch(&tmp, "track/my-feature-2026-04-10");
        assert_eq!(
            detect_active_track_from_branch(&root),
            Some("my-feature-2026-04-10".to_owned())
        );
    }

    #[test]
    fn detect_active_track_from_branch_returns_none_on_main() {
        let tmp = tempfile::tempdir().unwrap();
        let root = init_repo_on_branch(&tmp, "main");
        assert_eq!(detect_active_track_from_branch(&root), None);
    }

    #[test]
    fn detect_active_track_from_branch_returns_none_on_feature_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let root = init_repo_on_branch(&tmp, "feature/unrelated");
        assert_eq!(detect_active_track_from_branch(&root), None);
    }

    #[test]
    fn detect_active_track_from_branch_returns_none_outside_git_repo() {
        // `tempdir()` produces an empty directory — no `.git` anywhere up
        // the tree inside the tmpfs-backed path, so git discover fails
        // and auto-detection must silently return None.
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(detect_active_track_from_branch(tmp.path()), None);
    }

    // ── WRITE guard integration tests (AC-18 / D7 / T016) ───────────────────

    /// AC-18 / D7: explicit --track-id that mismatches the current branch must be
    /// rejected by the WRITE guard before sync_rendered_views is attempted.
    ///
    /// Uses a synthetic git repo on `track/real-track` so branch discovery succeeds.
    /// Supplying `other-track` as the explicit id triggers the mismatch error.
    #[test]
    fn sync_write_guard_rejects_explicit_id_mismatching_branch() {
        let tmp = tempfile::tempdir().unwrap();
        // Set up a real git repo on track/real-track so branch discovery finds it.
        let root = init_repo_on_branch(&tmp, "track/real-track");

        let result = execute_views(ViewAction::Sync {
            project_root: root.clone(),
            track_id: Some("other-track".to_owned()),
        });

        assert!(result.is_err(), "WRITE guard must reject mismatched explicit track-id");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("WRITE operation rejected") || err_msg.contains("other-track"),
            "error must explain the mismatch, got: {err_msg}"
        );
    }

    /// AC-18 / D7: explicit --track-id that matches the current branch passes the
    /// WRITE guard (the sync itself may fail due to missing registry files, but the
    /// guard itself must not reject the call).
    #[test]
    fn sync_write_guard_allows_explicit_id_matching_branch() {
        let tmp = tempfile::tempdir().unwrap();
        // Set up a real git repo on track/my-track so branch discovery finds it.
        let root = init_repo_on_branch(&tmp, "track/my-track");

        // Note: sync_rendered_views will fail because `track/items` etc. are absent
        // in the synthetic repo. The WRITE guard itself must pass (not return
        // "WRITE operation rejected"). We only check the guard layer — any downstream
        // filesystem error is acceptable.
        let result = execute_views(ViewAction::Sync {
            project_root: root.clone(),
            track_id: Some("my-track".to_owned()),
        });

        // Acceptable outcomes: Ok (unlikely — registry missing) or Err from render,
        // but must NOT be a WRITE guard rejection.
        if let Err(ref e) = result {
            let msg = format!("{e:?}");
            assert!(
                !msg.contains("WRITE operation rejected"),
                "WRITE guard must pass for matching track-id, got: {msg}"
            );
        }
    }
}
