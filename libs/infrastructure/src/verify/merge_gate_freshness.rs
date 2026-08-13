//! Git operations used to validate the freshness of generated merge-gate artifacts.

use std::path::Path;

use domain::{CommitHash, validate_branch_ref};

use crate::git_cli::isolation::isolated_bounded_git_output;

const MAX_BRANCH_COMMIT_BYTES: usize = 8 * 1024;

/// Resolves the commit against which the branch's committed signal artifact
/// was evaluated. Signal generation records the checked-out HEAD before the
/// generated artifact is committed, so the branch tip's first parent is the
/// evaluation commit for a committed signal file.
pub(super) fn read_branch_evaluation_commit(
    repo_root: &Path,
    branch: &str,
) -> Result<CommitHash, String> {
    validate_branch_ref(branch).map_err(|error| format!("invalid branch ref: {error}"))?;
    let revision = format!("origin/{branch}^1^{{commit}}");
    let args = ["rev-parse", "--verify", revision.as_str()];
    let output = isolated_bounded_git_output(repo_root, &args, MAX_BRANCH_COMMIT_BYTES)
        .map_err(|error| format!("failed to resolve branch evaluation commit: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git rev-parse failed for branch evaluation commit (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let commit = std::str::from_utf8(&output.stdout)
        .map_err(|error| format!("branch evaluation commit is not UTF-8: {error}"))?
        .trim();
    CommitHash::try_new(commit.to_owned())
        .map_err(|error| format!("branch evaluation commit is invalid: {error}"))
}
