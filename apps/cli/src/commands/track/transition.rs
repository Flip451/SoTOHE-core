use crate::CliError;

use super::*;
use usecase::task_ops::TaskOperationService as _;
use usecase::track_resolution::{BranchReadError, BranchReaderPort};

#[derive(Debug)]
struct LazyBranchReader {
    project_root: PathBuf,
}

impl LazyBranchReader {
    fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

impl BranchReaderPort for LazyBranchReader {
    fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
        let repo = SystemGitRepo::discover_from(&self.project_root).map_err(|e| {
            BranchReadError::ReadFailed(format!("failed to discover git repo: {e}"))
        })?;
        BranchReaderPort::current_branch(&repo)
    }
}

fn build_branch_reader(project_root: &std::path::Path) -> Option<Arc<dyn BranchReaderPort>> {
    Some(Arc::new(LazyBranchReader::new(project_root.to_path_buf())))
}

pub(super) fn execute_transition(
    items_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id as a safe slug before any filesystem probe.
    validate_track_id_str(&track_id).map_err(CliError::Message)?;

    // Preserve items_dir for project root resolution.
    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    // Build store and BranchReaderPort for TaskOperationInteractor.
    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let branch_reader = build_branch_reader(&project_root);
    let service =
        usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

    let cmd = usecase::task_ops::TaskTransitionCommand {
        items_dir,
        track_id: track_id.clone(),
        task_id: task_id.clone(),
        target_status: target_status.clone(),
        commit_hash,
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
