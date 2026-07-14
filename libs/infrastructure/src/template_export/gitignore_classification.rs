//! Git-backed classification of untracked, ignored worktree entries.

use std::path::Path;
use std::process::{Command, Stdio};

use domain::FreeText;
use usecase::template_export::TemplateExportPortError;

use super::filesystem::io_error;

/// Returns whether an unclassified path is Git-untracked and ignored.
pub(super) fn is_gitignored_untracked(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<bool, TemplateExportPortError> {
    let git_dir = workspace_root.join(".git");
    match std::fs::symlink_metadata(&git_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "refusing to follow a symlinked .git entry",
            );
            return Err(io_error(&git_dir, &error));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error(&git_dir, &error)),
    }
    if git_predicate(workspace_root, &["ls-files", "--error-unmatch", "--", relative_path])? {
        return Ok(false);
    }

    git_predicate(workspace_root, &["check-ignore", "--quiet", "--", relative_path])
}

/// Runs a Git predicate; unexpected exits fail the export closed.
fn git_predicate(workspace_root: &Path, args: &[&str]) -> Result<bool, TemplateExportPortError> {
    let status = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| io_error(workspace_root, &error))?;

    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => {
            let reason = format!("git {} failed with {status}", args.join(" "));
            Err(TemplateExportPortError::Io {
                path: workspace_root.to_path_buf(),
                reason: FreeText::new(reason),
            })
        }
    }
}
