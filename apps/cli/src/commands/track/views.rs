use super::*;

pub(super) fn execute_views(action: ViewAction) -> ExitCode {
    match action {
        ViewAction::Validate { project_root } => {
            match render::validate_track_snapshots(&project_root) {
                Ok(()) => {
                    println!("[OK] Track metadata is valid");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("track metadata validation failed: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        ViewAction::Sync { project_root, track_id } => {
            match render::sync_rendered_views(&project_root, track_id.as_deref()) {
                Ok(changed) => {
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
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("sync-views failed: {err}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
