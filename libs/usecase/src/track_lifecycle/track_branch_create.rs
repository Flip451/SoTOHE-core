//! Application boundary for creating a track branch.

use std::path::PathBuf;
use std::sync::Arc;

use domain::{TrackBranch, TrackId};

use crate::git_workflow::{
    DiagnosticText, GitPrimitivePort, TrackArchiveFsPort, TrackGitInteractor,
};

use super::{TrackBranchStrategyPort, TrackItemsDirectory, TrackLifecycleIdInput};

/// Typed input for creating a track branch.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackBranchCreateCommand {
    /// Directory containing track item directories.
    pub items_dir: TrackItemsDirectory,
    /// Validated track identity.
    pub track_id: TrackId,
}

impl TrackBranchCreateCommand {
    /// Creates a branch command from validated primary-adapter inputs.
    #[must_use]
    pub fn new(items_dir: TrackItemsDirectory, track_id: TrackLifecycleIdInput) -> Self {
        Self { items_dir, track_id: track_id.into_track_id() }
    }
}

/// Presentation-free result of track branch creation.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackBranchCreateResult {
    /// The branch created and checked out by the operation.
    pub branch: TrackBranch,
}

/// Error returned by the track-branch-create boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackBranchCreateError {
    /// Branch strategy or git execution failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackBranchCreateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackBranchCreateError {}

/// Application service for creating a track branch.
pub trait TrackBranchCreateService: Send + Sync {
    /// Resolves the configured base branch and creates the requested branch.
    fn execute(
        &self,
        command: TrackBranchCreateCommand,
    ) -> Result<TrackBranchCreateResult, TrackBranchCreateError>;
}

/// Interactor for the track-branch-create command context.
pub struct TrackBranchCreateInteractor {
    git: TrackGitInteractor,
    branch_strategy: Arc<dyn TrackBranchStrategyPort>,
}

impl TrackBranchCreateInteractor {
    /// Creates an interactor from the existing git workflow and strategy ports.
    #[must_use]
    pub fn new(
        git: Arc<dyn GitPrimitivePort>,
        fs: Arc<dyn TrackArchiveFsPort>,
        branch_strategy: Arc<dyn TrackBranchStrategyPort>,
    ) -> Self {
        Self { git: TrackGitInteractor::new(git, fs), branch_strategy }
    }
}

impl TrackBranchCreateService for TrackBranchCreateInteractor {
    fn execute(
        &self,
        command: TrackBranchCreateCommand,
    ) -> Result<TrackBranchCreateResult, TrackBranchCreateError> {
        let project_root = project_root_for_items(&command.items_dir)?;
        let strategy = self
            .branch_strategy
            .global_for_items(&command.items_dir)
            .map_err(|error| execution_failed(error.to_string()))?;
        let branch = TrackBranch::try_new(format!("track/{}", command.track_id))
            .map_err(|error| execution_failed(format!("invalid created branch: {error}")))?;

        self.git
            .create_track_branch(&project_root, &command.track_id, strategy.base_branch())
            .map_err(|error| execution_failed(error.to_string()))?;

        Ok(TrackBranchCreateResult { branch })
    }
}

fn project_root_for_items(
    items_dir: &TrackItemsDirectory,
) -> Result<PathBuf, TrackBranchCreateError> {
    let track_dir = items_dir
        .as_path()
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no track parent"))?;
    let root = track_dir
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no workspace root"))?;
    Ok(if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() })
}

fn execution_failed(error: impl Into<String>) -> TrackBranchCreateError {
    TrackBranchCreateError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use mockall::mock;

    use super::*;
    use crate::git_workflow::{ExplicitTrackBranch, GitWorkflowError, TrackBranchClaim};
    use crate::track_lifecycle::TrackWorkspaceRoot;

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

    mock! {
        Fs {}
        impl TrackArchiveFsPort for Fs {
            fn path_is_dir(&self, path: &Path) -> Result<bool, GitWorkflowError>;
            fn path_exists(&self, path: &Path) -> Result<bool, GitWorkflowError>;
            fn create_dir_all(&self, path: &Path) -> Result<(), GitWorkflowError>;
            fn rename_path(&self, src: &Path, dst: &Path) -> Result<(), GitWorkflowError>;
            fn list_dir_file_names(&self, path: &Path) -> Result<Vec<PathBuf>, GitWorkflowError>;
            fn remove_dir(&self, path: &Path) -> Result<(), GitWorkflowError>;
        }
    }

    fn snapshot() -> domain::BranchStrategySnapshot {
        domain::BranchStrategySnapshot::new(
            domain::NonEmptyString::try_new("main").expect("base branch is valid"),
            domain::NonEmptyString::try_new("main").expect("merge target is valid"),
            domain::MergeMethod::Merge,
        )
    }

    struct RecordingBranchStrategy {
        error: Option<DiagnosticText>,
    }

    impl TrackBranchStrategyPort for RecordingBranchStrategy {
        fn global_for_items(
            &self,
            items_dir: &TrackItemsDirectory,
        ) -> Result<domain::BranchStrategySnapshot, DiagnosticText> {
            assert_eq!(items_dir.as_path(), Path::new("workspace/track/items"));
            self.error.clone().map_or_else(|| Ok(snapshot()), Err)
        }

        fn snapshot_for_track(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _track_id: &TrackId,
        ) -> Result<domain::BranchStrategySnapshot, DiagnosticText> {
            Ok(snapshot())
        }
    }

    fn command() -> TrackBranchCreateCommand {
        TrackBranchCreateCommand {
            items_dir: TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            track_id: TrackId::try_new("new-track").expect("track id is valid"),
        }
    }

    fn configure_success_git(git: &mut MockGit) {
        git.expect_current_branch()
            .withf(|root| *root == Some(Path::new("workspace")))
            .returning(|_| Ok(Some("main".to_owned())));
        git.expect_branch_exists().returning(|_, _| Ok(false));
        git.expect_create_branch()
            .withf(|root, branch, base| {
                *root == Some(Path::new("workspace"))
                    && branch == "track/new-track"
                    && base == "main"
            })
            .returning(|_, _, _| Ok(()));
    }

    #[test]
    fn test_track_branch_create_interactor_valid_command_returns_created_branch() {
        let mut git = MockGit::new();
        configure_success_git(&mut git);
        let interactor = TrackBranchCreateInteractor::new(
            Arc::new(git),
            Arc::new(MockFs::new()),
            Arc::new(RecordingBranchStrategy { error: None }),
        );

        let result = interactor.execute(command()).expect("branch creation succeeds");

        assert_eq!(result.branch.as_ref(), "track/new-track");
    }

    #[test]
    fn test_track_branch_create_interactor_strategy_failure_returns_execution_error() {
        let interactor = TrackBranchCreateInteractor::new(
            Arc::new(MockGit::new()),
            Arc::new(MockFs::new()),
            Arc::new(RecordingBranchStrategy {
                error: Some(DiagnosticText::new("branch strategy unavailable")),
            }),
        );

        let error = interactor.execute(command()).expect_err("strategy failure must propagate");

        assert_eq!(error.to_string(), "branch strategy unavailable");
    }

    #[test]
    fn test_track_branch_create_interactor_git_failure_returns_execution_error() {
        let mut git = MockGit::new();
        git.expect_current_branch().returning(|_| Ok(Some("main".to_owned())));
        git.expect_branch_exists().returning(|_, _| Ok(false));
        git.expect_create_branch().returning(|_, _, _| {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new("git unavailable")))
        });
        let interactor = TrackBranchCreateInteractor::new(
            Arc::new(git),
            Arc::new(MockFs::new()),
            Arc::new(RecordingBranchStrategy { error: None }),
        );

        let error = interactor.execute(command()).expect_err("git failure must propagate");

        assert!(matches!(
            error,
            TrackBranchCreateError::ExecutionFailed(diagnostic)
                if diagnostic.as_str() == "git workflow unavailable: git unavailable"
        ));
    }
}
