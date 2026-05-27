use std::sync::Arc;

use infrastructure::git_cli::SystemGitRepo;
use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService as _};

use crate::CliError;

use super::*;

pub(super) fn execute_views(action: ViewAction) -> Result<ExitCode, CliError> {
    match action {
        ViewAction::Validate { project_root } => {
            render::validate_track_snapshots(&project_root).map_err(|err| {
                CliError::Message(format!("track metadata validation failed: {err}"))
            })?;
            println!("[OK] Track metadata is valid");
            Ok(ExitCode::SUCCESS)
        }
        ViewAction::Sync { project_root, track_id } => {
            // If `--track-id` was not given, try to detect the active track from
            // the current git branch (`track/<id>`). This makes
            // `cargo make track-sync-views` "do the right thing" inside an
            // active track checkout without requiring the caller to repeat the
            // track id. When the current branch is not a track/plan branch
            // (e.g., on `main`), or when git is unavailable, fall back to
            // registry-only mode (CN-01 documented exception: no-track mode
            // is preserved for track views sync because registry.md is
            // track-independent).
            //
            // Explicit ids are validated before use to prevent path-traversal
            // via malformed inputs (e.g. `../../tmp`) reaching sync_rendered_views.
            let resolved_track_id = match track_id {
                Some(id) => {
                    super::validate_track_id_str(&id).map_err(CliError::Message)?;
                    Some(id)
                }
                None => detect_active_track_from_branch(&project_root),
            };
            let changed = render::sync_rendered_views(&project_root, resolved_track_id.as_deref())
                .map_err(|err| CliError::Message(format!("sync-views failed: {err}")))?;
            if changed.is_empty() {
                println!("[OK] All views already up to date");
            } else {
                for path in changed {
                    match path.strip_prefix(&project_root) {
                        Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                        Err(_) => println!("[OK] Rendered: {}", path.display()),
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Detect the active track id from the current git branch via the shared
/// `ActiveTrackResolveInteractor` (IN-09: consolidates individual auto-detect
/// implementations onto the shared interactor path).
///
/// Only `track/<id>` branches are resolved; any other branch (e.g. `main`,
/// detached HEAD) or git failure resolves to `None` so the caller can fall
/// back to registry-only mode without surfacing an error.
///
/// Uses `project_root` for git discovery so that auto-detection is consistent
/// with `--project-root` invocations and does not depend on the process CWD.
fn detect_active_track_from_branch(project_root: &std::path::Path) -> Option<String> {
    let repo = SystemGitRepo::discover_from(project_root).ok()?;
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    interactor.resolve_active_track().ok()
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
}
