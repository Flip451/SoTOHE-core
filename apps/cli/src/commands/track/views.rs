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
            let changed = render::sync_rendered_views(&project_root, track_id.as_deref())
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
