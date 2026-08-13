//! Read-only probes of the repository's merge state.
//!
//! Part of the `base_merge` adapter, in its own file so the adapter stays
//! within the workspace module-size limit. Everything here answers one
//! question about the repository at a path — whether a merge is in progress,
//! what `MERGE_HEAD` records, whether the base is already merged, whether the
//! worktree carries unmerged paths — and none of it mutates anything.

use std::path::Path;

use domain::CommitHash;
use usecase::base_merge::BaseMergeGitError;

use super::{MAX_BASE_MERGE_GIT_OUTPUT_BYTES, git_execution_error};
use crate::git_cli::isolated_bounded_git_output;

pub(super) fn merge_head_is_present(repository_root: &Path) -> Result<bool, BaseMergeGitError> {
    Ok(read_merge_head(repository_root)?.is_some())
}

pub(super) fn merge_head_matches_commit(
    repository_root: &Path,
    expected: &CommitHash,
) -> Result<bool, BaseMergeGitError> {
    Ok(read_merge_head(repository_root)?.is_some_and(|actual| actual == *expected))
}

pub(super) fn read_merge_head(
    repository_root: &Path,
) -> Result<Option<CommitHash>, BaseMergeGitError> {
    let path_output = isolated_bounded_git_output(
        repository_root,
        &["rev-parse", "--git-path", "MERGE_HEAD"],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| git_execution_error("merge state could not be inspected"))?;
    if !path_output.status.success() {
        return Err(git_execution_error("merge state could not be inspected"));
    }
    let raw_path = std::str::from_utf8(&path_output.stdout)
        .map_err(|_| git_execution_error("merge state could not be inspected"))?
        .trim();
    if raw_path.is_empty() {
        return Err(git_execution_error("merge state could not be inspected"));
    }
    let merge_head_path = Path::new(raw_path);
    let merge_head_path = if merge_head_path.is_absolute() {
        merge_head_path.to_owned()
    } else {
        repository_root.join(merge_head_path)
    };
    crate::track::symlink_guard::reject_symlinks_up_to_root(&merge_head_path)
        .map_err(|_| git_execution_error("merge state could not be inspected"))?;
    match merge_head_path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(git_execution_error("merge state could not be inspected"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(git_execution_error("merge state could not be inspected")),
    }

    let output = isolated_bounded_git_output(
        repository_root,
        &["rev-parse", "--verify", "--quiet", "MERGE_HEAD^{commit}"],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| git_execution_error("merge state could not be inspected"))?;
    match output.status.code() {
        Some(0) => {
            let commit = std::str::from_utf8(&output.stdout)
                .map_err(|_| git_execution_error("merge state could not be inspected"))?;
            let commit = CommitHash::try_new(commit.trim().to_owned())
                .map_err(|_| git_execution_error("merge state could not be inspected"))?;
            Ok(Some(commit))
        }
        Some(1) => Err(git_execution_error("merge state could not be inspected")),
        _ => Err(git_execution_error("merge state could not be inspected")),
    }
}

pub(super) fn base_commit_is_merged_into_head(
    repository_root: &Path,
    base_commit: &CommitHash,
) -> Result<bool, BaseMergeGitError> {
    let output = isolated_bounded_git_output(
        repository_root,
        &["merge-base", "--is-ancestor", base_commit.as_ref(), "HEAD"],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| git_execution_error("merged HEAD could not be inspected"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_execution_error("merged HEAD could not be inspected")),
    }
}

pub(super) fn has_unmerged_paths(repository_root: &Path) -> Result<bool, BaseMergeGitError> {
    let output = isolated_bounded_git_output(
        repository_root,
        &["diff", "--quiet", "--diff-filter=U", "--"],
        1,
    )
    .map_err(|_| git_execution_error("merge conflict state could not be inspected"))?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(git_execution_error("merge conflict state could not be inspected")),
    }
}
