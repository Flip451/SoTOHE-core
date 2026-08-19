//! Application boundary for switching to the configured base branch.

use std::sync::Arc;

use domain::BaseBranchName;

use crate::git_workflow::{DiagnosticText, GitPrimitivePort, GitWorkflowError};

use super::{ProcessExitCode, TrackBranchStrategyPort, TrackSelectionPort, TrackWorkspaceRoot};

/// Typed input for switching to the active track's configured base branch.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackSwitchBaseCommand {
    /// Workspace root containing `track/`.
    pub workspace_root: TrackWorkspaceRoot,
}

impl TrackSwitchBaseCommand {
    /// Creates a switch-base command from a validated workspace root.
    #[must_use]
    pub fn new(workspace_root: TrackWorkspaceRoot) -> Self {
        Self { workspace_root }
    }
}

/// Presentation-free result of switching to the configured base branch.
#[derive(Debug, PartialEq, Eq)]
pub enum TrackSwitchBaseResult {
    /// The base branch was checked out and the following sync succeeded.
    Synced {
        /// The configured base branch that is now current.
        branch: BaseBranchName,
    },
    /// The base branch was checked out, but the following sync was non-fatal.
    SyncWarning {
        /// The configured base branch that is now current.
        branch: BaseBranchName,
    },
    /// `git switch` failed; the caller renders the legacy checkout outcome.
    CheckoutFailed {
        /// The configured base branch that could not be checked out.
        branch: BaseBranchName,
        /// Process exit code clamped to `u8`.
        exit_code: ProcessExitCode,
    },
}

/// Error returned by the track switch-base boundary.
#[derive(Debug)]
pub enum TrackSwitchBaseError {
    /// Selection, snapshot, or git execution failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackSwitchBaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackSwitchBaseError {}

/// Application service for switching to the configured base branch.
pub trait TrackSwitchBaseService: Send + Sync {
    /// Resolves the active track snapshot, switches to its base branch, and syncs.
    fn execute(
        &self,
        command: TrackSwitchBaseCommand,
    ) -> Result<TrackSwitchBaseResult, TrackSwitchBaseError>;
}

/// Interactor for the track switch-base command context.
pub struct TrackSwitchBaseInteractor {
    selection: Arc<dyn TrackSelectionPort>,
    branch_strategy: Arc<dyn TrackBranchStrategyPort>,
    git: Arc<dyn GitPrimitivePort>,
}

impl TrackSwitchBaseInteractor {
    /// Creates an interactor from the git, selection, and snapshot ports.
    #[must_use]
    pub fn new(
        git: Arc<dyn GitPrimitivePort>,
        resolver: Arc<dyn TrackSelectionPort>,
        branch_strategy: Arc<dyn TrackBranchStrategyPort>,
    ) -> Self {
        Self { selection: resolver, branch_strategy, git }
    }
}

impl TrackSwitchBaseService for TrackSwitchBaseInteractor {
    fn execute(
        &self,
        command: TrackSwitchBaseCommand,
    ) -> Result<TrackSwitchBaseResult, TrackSwitchBaseError> {
        let track_id = self
            .selection
            .resolve_active(&command.workspace_root)
            .map_err(|error| execution_failed(error.to_string()))?;
        let snapshot = self
            .branch_strategy
            .snapshot_for_track(&command.workspace_root, &track_id)
            .map_err(|error| execution_failed(error.to_string()))?;
        let branch = BaseBranchName::try_new(snapshot.base_branch().to_owned())
            .map_err(|error| execution_failed(format!("invalid base branch: {error}")))?;

        match self.git.switch_branch(Some(command.workspace_root.as_path()), branch.as_str()) {
            Ok(()) => {}
            Err(GitWorkflowError::SwitchFailed { exit_code, .. }) => {
                return Ok(TrackSwitchBaseResult::CheckoutFailed {
                    branch,
                    exit_code: ProcessExitCode::new(clamp_exit_code(exit_code)),
                });
            }
            Err(error) => return Err(execution_failed(error.to_string())),
        }

        match self.git.sync_current_branch(Some(command.workspace_root.as_path())) {
            Ok(()) => Ok(TrackSwitchBaseResult::Synced { branch }),
            Err(
                GitWorkflowError::SyncUpstreamNotSet
                | GitWorkflowError::SyncNonFastForward { .. }
                | GitWorkflowError::SyncWorktreeUnresolved { .. }
                | GitWorkflowError::Unavailable(_),
            ) => Ok(TrackSwitchBaseResult::SyncWarning { branch }),
            Err(error) => Err(execution_failed(error.to_string())),
        }
    }
}

fn clamp_exit_code(exit_code: i32) -> u8 {
    u8::try_from(exit_code).unwrap_or(1)
}

fn execution_failed(error: impl Into<String>) -> TrackSwitchBaseError {
    TrackSwitchBaseError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use domain::{MergeMethod, NonEmptyString, TrackId};
    use mockall::mock;

    use super::*;
    use crate::git_workflow::{ExplicitTrackBranch, TrackBranchClaim};
    use crate::track_lifecycle::{TrackItemsDirectory, TrackSelection, TrackViewsScope};

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

    struct StubSelection {
        result: Result<TrackId, String>,
    }

    impl TrackSelectionPort for StubSelection {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            panic!("switch-base uses resolve_active")
        }

        fn resolve_active(
            &self,
            workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            assert_eq!(workspace_root.as_path(), Path::new("workspace"));
            match &self.result {
                Ok(track_id) => Ok(track_id.clone()),
                Err(error) => Err(DiagnosticText::new(error)),
            }
        }

        fn resolve_views_scope(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _selection: &TrackSelection,
        ) -> Result<TrackViewsScope, DiagnosticText> {
            panic!("switch-base does not resolve view scope")
        }
    }

    struct StubBranchStrategy {
        error: Option<String>,
    }

    impl TrackBranchStrategyPort for StubBranchStrategy {
        fn global_for_items(
            &self,
            _items_dir: &TrackItemsDirectory,
        ) -> Result<domain::BranchStrategySnapshot, DiagnosticText> {
            panic!("switch-base uses snapshot_for_track")
        }

        fn snapshot_for_track(
            &self,
            workspace_root: &TrackWorkspaceRoot,
            track_id: &TrackId,
        ) -> Result<domain::BranchStrategySnapshot, DiagnosticText> {
            assert_eq!(workspace_root.as_path(), Path::new("workspace"));
            assert_eq!(track_id.as_ref(), "active-track");
            if let Some(error) = &self.error {
                return Err(DiagnosticText::new(error));
            }
            Ok(domain::BranchStrategySnapshot::new(
                NonEmptyString::try_new("main").expect("base branch is valid"),
                NonEmptyString::try_new("main").expect("merge target is valid"),
                MergeMethod::Merge,
            ))
        }
    }

    fn command() -> TrackSwitchBaseCommand {
        TrackSwitchBaseCommand::new(
            TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace is valid"),
        )
    }

    fn interactor(
        git: MockGit,
        selection_error: Option<&str>,
        strategy_error: Option<&str>,
    ) -> TrackSwitchBaseInteractor {
        let selection = StubSelection {
            result: selection_error.map_or_else(
                || Ok(TrackId::try_new("active-track").expect("track id is valid")),
                |error| Err(error.to_owned()),
            ),
        };
        TrackSwitchBaseInteractor::new(
            Arc::new(git),
            Arc::new(selection),
            Arc::new(StubBranchStrategy { error: strategy_error.map(str::to_owned) }),
        )
    }

    #[test]
    fn test_track_switch_base_error_shares_command_context_module() {
        let error = std::any::type_name::<TrackSwitchBaseError>();
        let interactor = std::any::type_name::<TrackSwitchBaseInteractor>();
        let service = std::any::type_name::<dyn TrackSwitchBaseService>();
        assert!(error.contains("track_switch_base"), "error module: {error}");
        assert!(interactor.contains("track_switch_base"), "interactor module: {interactor}");
        assert!(service.contains("track_switch_base"), "service module: {service}");
    }

    #[test]
    fn test_track_switch_base_interactor_successful_sync_returns_synced() {
        let mut git = MockGit::new();
        git.expect_switch_branch()
            .withf(|root, branch| *root == Some(Path::new("workspace")) && branch == "main")
            .returning(|_, _| Ok(()));
        git.expect_sync_current_branch()
            .withf(|root| *root == Some(Path::new("workspace")))
            .returning(|_| Ok(()));

        let result = interactor(git, None, None).execute(command()).expect("switch-base succeeds");

        assert!(matches!(
            result,
            TrackSwitchBaseResult::Synced { branch } if branch.as_str() == "main"
        ));
    }

    #[test]
    fn test_track_switch_base_interactor_non_fast_forward_sync_returns_warning() {
        let mut git = MockGit::new();
        git.expect_switch_branch().returning(|_, _| Ok(()));
        git.expect_sync_current_branch().returning(|_| {
            Err(GitWorkflowError::SyncNonFastForward { stderr: DiagnosticText::new("diverged") })
        });

        let result = interactor(git, None, None).execute(command()).expect("warning is non-fatal");

        assert!(matches!(
            result,
            TrackSwitchBaseResult::SyncWarning { branch } if branch.as_str() == "main"
        ));
    }

    #[test]
    fn test_track_switch_base_interactor_switch_failure_returns_checkout_failed() {
        let mut git = MockGit::new();
        git.expect_switch_branch().returning(|_, _| {
            Err(GitWorkflowError::SwitchFailed {
                branch: DiagnosticText::new("main"),
                exit_code: 7,
            })
        });

        let result =
            interactor(git, None, None).execute(command()).expect("checkout failure is a result");

        assert!(matches!(
            result,
            TrackSwitchBaseResult::CheckoutFailed { branch, exit_code }
                if branch.as_str() == "main" && exit_code.value() == 7
        ));
    }

    #[test]
    fn test_track_switch_base_interactor_resolution_failure_returns_execution_error() {
        let git = MockGit::new();
        let error = match interactor(git, Some("not a track branch"), None).execute(command()) {
            Ok(_) => panic!("resolution failure must fail"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "not a track branch");
    }

    #[test]
    fn test_track_switch_base_interactor_unexpected_git_error_returns_execution_error() {
        let mut git = MockGit::new();
        git.expect_switch_branch().returning(|_, _| {
            Err(GitWorkflowError::Unavailable(DiagnosticText::new("git missing")))
        });

        let error = match interactor(git, None, None).execute(command()) {
            Ok(_) => panic!("unexpected git failure must fail"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "git workflow unavailable: git missing");
    }
}
