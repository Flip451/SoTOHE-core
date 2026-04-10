use crate::CliError;

use super::*;
use usecase::track_resolution::resolve_track_or_plan_id_from_branch;

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
            // the current git branch (`track/<id>` or `plan/<id>`). This makes
            // `cargo make track-sync-views` "do the right thing" inside an
            // active track checkout without requiring the caller to repeat the
            // track id. When the current branch is not a track/plan branch
            // (e.g., on `main`), fall back to the registry-only mode.
            let resolved_track_id = match track_id {
                Some(id) => Some(id),
                None => detect_track_id_from_branch(&project_root),
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

/// Detect the active track id from the current git branch using the shared
/// lenient resolver (`resolve_track_or_plan_id_from_branch`). Both
/// `track/<id>` and `plan/<id>` branches map to the same bare track id; any
/// other branch (e.g. `main`, detached HEAD) or git failure resolves to
/// `None` so the caller can fall back to registry-only mode without
/// surfacing an error.
///
/// Uses `project_root` as the working directory for the underlying git
/// command so that auto-detection is consistent with `--project-root`
/// invocations and does not depend on the process CWD.
fn detect_track_id_from_branch(project_root: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    resolve_track_or_plan_id_from_branch(Some(&branch)).ok()
}
