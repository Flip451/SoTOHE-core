use std::path::Path;

use usecase::base_merge::BaseMergeGitError;
use usecase::git_workflow::DiagnosticText;

use crate::git_cli::isolated_bounded_git_output;

use super::{
    MAX_BASE_MERGE_GIT_OUTPUT_BYTES, TRACK_WRITER_LOCK_FILE, errors::git_execution_error,
    resolve_workspace_repository_root,
};

pub(super) fn ensure_worktree_clean(workspace_root: &Path) -> Result<(), BaseMergeGitError> {
    let repository_root =
        resolve_workspace_repository_root(workspace_root).map_err(git_execution_error)?;
    let current_branch =
        super::read_current_track_branch(&repository_root).map_err(git_execution_error)?;
    let track_id = super::track_id_from_branch(&current_branch).map_err(git_execution_error)?;
    let lock_pathspec =
        format!(":(exclude)track/items/{}/{TRACK_WRITER_LOCK_FILE}", track_id.as_ref());
    let output = isolated_bounded_git_output(
        &repository_root,
        &["status", "--porcelain", "--untracked-files=all", "--", ".", lock_pathspec.as_str()],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| {
        git_execution_error(format!("git status --porcelain could not be collected: {error}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if stderr.is_empty() {
            format!("git status --porcelain failed (exit {})", output.status.code().unwrap_or(-1))
        } else {
            format!(
                "git status --porcelain failed (exit {}): {stderr}",
                output.status.code().unwrap_or(-1)
            )
        };
        return Err(git_execution_error(detail));
    }

    if output.stdout.is_empty() {
        return Ok(());
    }

    let summary = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Err(BaseMergeGitError::DirtyWorktree(DiagnosticText::new(if summary.is_empty() {
        "worktree status reported changes"
    } else {
        summary.as_str()
    })))
}
