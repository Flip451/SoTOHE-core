//! Application boundary for switching to an existing track branch.

use std::path::PathBuf;
use std::sync::Arc;

use domain::{TrackBranch, TrackId};

use crate::git_workflow::{DiagnosticText, GitPrimitivePort};

use super::{TrackItemsDirectory, TrackLifecycleIdInput};

/// Typed input for switching to an existing track branch.
pub struct TrackBranchSwitchCommand {
    /// Directory containing track item directories.
    pub items_dir: TrackItemsDirectory,
    /// Validated track identity.
    pub track_id: TrackId,
}

impl TrackBranchSwitchCommand {
    /// Creates a branch-switch command from validated primary-adapter inputs.
    #[must_use]
    pub fn new(items_dir: TrackItemsDirectory, track_id: TrackLifecycleIdInput) -> Self {
        Self { items_dir, track_id: track_id.into_track_id() }
    }
}

/// Presentation-free result of switching to a track branch.
pub struct TrackBranchSwitchResult {
    /// The branch selected by the operation.
    pub branch: TrackBranch,
}

/// Error returned by the track-branch-switch boundary.
#[derive(Debug)]
pub enum TrackBranchSwitchError {
    /// Branch lookup or git execution failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackBranchSwitchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackBranchSwitchError {}

/// Application service for switching to an existing track branch.
pub trait TrackBranchSwitchService: Send + Sync {
    /// Verifies and checks out the requested track branch.
    fn execute(
        &self,
        command: TrackBranchSwitchCommand,
    ) -> Result<TrackBranchSwitchResult, TrackBranchSwitchError>;
}

/// Interactor for the track-branch-switch command context.
pub struct TrackBranchSwitchInteractor {
    git: Arc<dyn GitPrimitivePort>,
}

impl TrackBranchSwitchInteractor {
    /// Creates an interactor from the existing git primitive port.
    #[must_use]
    pub fn new(git: Arc<dyn GitPrimitivePort>) -> Self {
        Self { git }
    }
}

impl TrackBranchSwitchService for TrackBranchSwitchInteractor {
    fn execute(
        &self,
        command: TrackBranchSwitchCommand,
    ) -> Result<TrackBranchSwitchResult, TrackBranchSwitchError> {
        let project_root = project_root_for_items(&command.items_dir)?;
        let branch = TrackBranch::try_new(format!("track/{}", command.track_id))
            .map_err(|error| execution_failed(format!("invalid track branch: {error}")))?;

        if !self
            .git
            .branch_exists(Some(&project_root), branch.as_ref())
            .map_err(|error| execution_failed(error.to_string()))?
        {
            return Err(execution_failed(format!("branch '{}' does not exist", branch.as_ref())));
        }

        self.git
            .switch_branch(Some(&project_root), branch.as_ref())
            .map_err(|error| execution_failed(error.to_string()))?;

        Ok(TrackBranchSwitchResult { branch })
    }
}

fn project_root_for_items(
    items_dir: &TrackItemsDirectory,
) -> Result<PathBuf, TrackBranchSwitchError> {
    let track_dir = items_dir
        .as_path()
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no track parent"))?;
    let root = track_dir
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no workspace root"))?;
    Ok(if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() })
}

fn execution_failed(error: impl Into<String>) -> TrackBranchSwitchError {
    TrackBranchSwitchError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use mockall::mock;

    use super::*;
    use crate::git_workflow::{ExplicitTrackBranch, GitWorkflowError, TrackBranchClaim};

    mock! {
        Git {}
        impl GitPrimitivePort for Git {
            fn current_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<Option<String>, GitWorkflowError>;
            fn sync_current_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<(), GitWorkflowError>;
            fn switch_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                branch: &str,
            ) -> Result<(), GitWorkflowError>;
            fn create_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                new_branch: &str,
                base_branch: &str,
            ) -> Result<(), GitWorkflowError>;
            fn branch_exists<'a>(
                &self,
                project_root: Option<&'a Path>,
                branch: &str,
            ) -> Result<bool, GitWorkflowError>;
            fn move_path<'a>(
                &self,
                project_root: Option<&'a Path>,
                src: &Path,
                dst: &Path,
            ) -> Result<(), GitWorkflowError>;
            fn fetch_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                branch: &str,
            ) -> Result<(), GitWorkflowError>;
            fn show_file_at_ref<'a>(
                &self,
                project_root: Option<&'a Path>,
                git_ref: &str,
                path: &Path,
            ) -> Result<String, GitWorkflowError>;
            fn resolve_commit<'a>(
                &self,
                project_root: Option<&'a Path>,
                rev: &str,
            ) -> Result<Option<domain::CommitHash>, GitWorkflowError>;
            fn resolve_repo_root<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<PathBuf, GitWorkflowError>;
            fn stage_all<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<(), GitWorkflowError>;
            fn stage_from_file<'a>(
                &self,
                project_root: Option<&'a Path>,
                path: &Path,
                cleanup: bool,
            ) -> Result<(), GitWorkflowError>;
            fn commit_from_message_file<'a>(
                &self,
                project_root: Option<&'a Path>,
                path: &Path,
                cleanup: bool,
            ) -> Result<(), GitWorkflowError>;
            fn note_from_file<'a>(
                &self,
                project_root: Option<&'a Path>,
                path: &Path,
                cleanup: bool,
            ) -> Result<(), GitWorkflowError>;
            fn unstage<'a>(
                &self,
                project_root: Option<&'a Path>,
                paths: &[PathBuf],
            ) -> Result<(), GitWorkflowError>;
            fn read_explicit_track_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                track_dir: &Path,
            ) -> Result<ExplicitTrackBranch, GitWorkflowError>;
            fn collect_track_branch_claims<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<Vec<TrackBranchClaim>, GitWorkflowError>;
        }
    }

    fn command() -> TrackBranchSwitchCommand {
        TrackBranchSwitchCommand {
            items_dir: TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            track_id: TrackId::try_new("switch-track").expect("track id is valid"),
        }
    }

    #[test]
    fn test_track_branch_switch_interactor_existing_branch_returns_typed_branch() {
        let mut git = MockGit::new();
        git.expect_branch_exists()
            .withf(|root, branch| {
                *root == Some(Path::new("workspace")) && branch == "track/switch-track"
            })
            .returning(|_, _| Ok(true));
        git.expect_switch_branch()
            .withf(|root, branch| {
                *root == Some(Path::new("workspace")) && branch == "track/switch-track"
            })
            .returning(|_, _| Ok(()));

        let interactor = TrackBranchSwitchInteractor::new(Arc::new(git));
        let result = interactor.execute(command()).expect("branch switch succeeds");

        assert_eq!(result.branch.as_ref(), "track/switch-track");
    }

    #[test]
    fn test_track_branch_switch_interactor_missing_branch_returns_execution_error() {
        let mut git = MockGit::new();
        git.expect_branch_exists().returning(|_, _| Ok(false));

        let interactor = TrackBranchSwitchInteractor::new(Arc::new(git));
        let error = match interactor.execute(command()) {
            Ok(_) => panic!("missing branch must fail"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "branch 'track/switch-track' does not exist");
    }

    #[test]
    fn test_track_branch_switch_interactor_git_failure_returns_execution_error() {
        let mut git = MockGit::new();
        git.expect_branch_exists().returning(|_, _| Ok(true));
        git.expect_switch_branch().returning(|_, _| {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new("git unavailable")))
        });

        let interactor = TrackBranchSwitchInteractor::new(Arc::new(git));
        let error = match interactor.execute(command()) {
            Ok(_) => panic!("git failure must propagate"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "git workflow unavailable: git unavailable");
    }
}
