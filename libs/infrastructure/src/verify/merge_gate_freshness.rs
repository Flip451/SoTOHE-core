//! Git operations used to validate the freshness of generated merge-gate artifacts.

use std::path::Path;

use domain::{CommitHash, validate_branch_ref};

use crate::git_cli::isolation::isolated_bounded_git_output;

const MAX_BRANCH_COMMIT_BYTES: usize = 8 * 1024;

/// Derives the signal filename for a declaration filename by the same rule
/// as `TdddLayerBinding::signal_file()` (infrastructure/verify/tddd_layers,
/// T003): strip `.json`, drop a trailing `s` if present, append
/// `-signals.json`. This keeps the signal-path binding next to the branch
/// evaluation-commit freshness check.
pub(super) fn signal_file_name_for(catalogue_filename: &str) -> String {
    let stem = catalogue_filename.strip_suffix(".json").unwrap_or(catalogue_filename);
    let signal_stem = if let Some(trimmed) = stem.strip_suffix('s') {
        format!("{trimmed}-signals")
    } else {
        format!("{stem}-signals")
    };
    format!("{signal_stem}.json")
}

/// Resolves the commit against which the branch's committed signal artifact
/// was evaluated. Signal generation records the checked-out HEAD before the
/// generated artifact is committed, so the branch tip's first parent is the
/// evaluation commit for a committed signal file.
pub(super) fn read_branch_evaluation_commit(
    repo_root: &Path,
    branch: &str,
) -> Result<CommitHash, String> {
    validate_branch_ref(branch).map_err(|error| format!("invalid branch ref: {error}"))?;
    let evaluation_revision = format!("origin/{branch}^1^{{commit}}");
    resolve_revision(repo_root, &evaluation_revision, "branch evaluation commit")
}

fn resolve_revision(
    repo_root: &Path,
    revision: &str,
    description: &str,
) -> Result<CommitHash, String> {
    let args = ["rev-parse", "--verify", revision];
    let output = isolated_bounded_git_output(repo_root, &args, MAX_BRANCH_COMMIT_BYTES)
        .map_err(|error| format!("failed to resolve {description}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git rev-parse failed for {description} (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let commit = std::str::from_utf8(&output.stdout)
        .map_err(|error| format!("{description} is not UTF-8: {error}"))?
        .trim();
    CommitHash::try_new(commit.to_owned())
        .map_err(|error| format!("{description} is invalid: {error}"))
}
