//! Shared branch primitives for track branch operations.
//!
//! Contains the implementation of `track branch create` and `track branch switch`,
//! along with shared git helpers used by both operations.

use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::track::TrackInput;

use crate::CliError;

use super::state_ops::track_driver_outcome_to_result;
use super::{BranchAction, BranchArgs};

const _: fn(&str) -> Result<(), super::validate::TrackValidateError> =
    super::validate_track_branch_str;

pub(super) fn execute_branch(action: BranchAction) -> Result<ExitCode, CliError> {
    match action {
        BranchAction::Create(args) => execute_branch_create(args),
        BranchAction::Switch(args) => execute_branch_switch(args),
    }
}

/// Creates a new `track/<track-id>` branch from the configured base branch and switches to it.
///
/// # Errors
/// Returns `CliError::Message` when the track driver reports a branch creation failure.
fn execute_branch_create(args: BranchArgs) -> Result<ExitCode, CliError> {
    let BranchArgs { items_dir, track_id } = args;

    let outcome = TrackCompositionRoot::new()
        .track_driver()
        .handle(TrackInput::BranchCreate { items_dir, track_id });
    track_driver_outcome_to_result(outcome)
}

/// Switches to an existing `track/<track-id>` branch.
///
/// # Errors
/// Returns `CliError::Message` when the track driver reports a branch switch failure.
fn execute_branch_switch(args: BranchArgs) -> Result<ExitCode, CliError> {
    let BranchArgs { items_dir, track_id } = args;

    let outcome = TrackCompositionRoot::new()
        .track_driver()
        .handle(TrackInput::BranchSwitch { items_dir, track_id });
    track_driver_outcome_to_result(outcome)
}

// The former `#[cfg(test)] mod tests { ... }` block (StubRepo / RecordingRepo
// trait-based test scaffolding + `branch_create_git_commands` /
// `preflight_branch_operation` / `branch_create_execute` helpers) has been
// removed as part of the T008 cutover: the `GitRepository` trait no longer
// exists (its methods moved to `pub` inherent methods on `SystemGitRepo`), and
// the actual branch-create / branch-switch orchestration lives in
// `usecase::git_workflow::TrackGitInteractor` (T006) with its own mock-port
// unit tests in `libs/usecase/src/git_workflow.rs::tests`. The lightweight
// `resolve_project_root` sanity tests are preserved below.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::super::resolve_project_root;

    #[test]
    fn resolve_project_root_accepts_standard_track_items_layout() {
        assert_eq!(
            resolve_project_root(Path::new("repo/track/items")),
            Ok(std::path::PathBuf::from("repo"))
        );
    }

    #[test]
    fn resolve_project_root_rejects_non_standard_layout() {
        assert!(matches!(
            resolve_project_root(Path::new("repo/custom-items")),
            Err(err) if err.to_string().contains("track/items")
        ));
    }

    #[test]
    fn resolve_project_root_returns_dot_for_relative_track_items_path() {
        // When items_dir is the bare relative path "track/items" (no leading ancestor
        // component), Path::parent() resolves the grandparent to an empty path "".
        // resolve_project_root must return "." instead of "" so that callers can pass
        // the result to Command::current_dir without triggering ENOENT (empty cwd).
        assert_eq!(resolve_project_root(Path::new("track/items")), Ok(PathBuf::from(".")));
    }
}
