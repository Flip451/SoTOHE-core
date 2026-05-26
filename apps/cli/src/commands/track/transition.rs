use crate::CliError;

use super::*;
use usecase::task_ops::TaskOperationService as _;

pub(super) fn execute_transition(
    items_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    // Validate track_id as a safe slug before any filesystem probe.
    validate_track_id_str(&track_id).map_err(CliError::Message)?;

    // Preserve items_dir for branch guard and project root resolution.
    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    // Build store and branch reader for TaskOperationInteractor.
    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let repo_dir_for_reader = repo_dir.clone();
    let service =
        usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), move |_items_dir| {
            // Read the current branch from git for the branch guard.
            let output = std::process::Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&repo_dir_for_reader)
                .output()
                .map_err(|e| format!("failed to run git: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("git rev-parse failed: {stderr}"));
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        });

    let cmd = usecase::task_ops::TaskTransitionCommand {
        items_dir,
        track_id: track_id.clone(),
        task_id: task_id.clone(),
        target_status: target_status.clone(),
        commit_hash,
        skip_branch_check,
    };
    let output = service
        .transition_task(cmd)
        .map_err(|err| CliError::Message(format!("transition failed: {err}")))?;

    println!(
        "[OK] {}: transitioned to {} (track status: {})",
        task_id, target_status, output.derived_status,
    );
    let track_id_ref = output.track_id.as_str();
    match render::sync_rendered_views(&project_root, Some(track_id_ref)) {
        Ok(changed) => {
            for path in changed {
                match path.strip_prefix(&project_root) {
                    Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                    Err(_) => println!("[OK] Rendered: {}", path.display()),
                }
            }
        }
        Err(err) => {
            eprintln!("warning: transition persisted but sync-views failed: {err}");
        }
    }
    Ok(ExitCode::SUCCESS)
}
