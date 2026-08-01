//! Base-to-track merge application service.
//!
//! This module coordinates the guarded merge through injected ports.  It does
//! not inspect the filesystem or invoke git itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::branch_strategy::BaseMergeDirection;
use domain::{TrackBranch, TrackId};
use thiserror::Error;

use crate::git_workflow::DiagnosticText;

/// Pre-cleanup result returned by the git-process port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseMergeAttemptOutcome {
    /// The base branch was merged without conflicts.
    Clean,
    /// Git reported conflicts; recovery remains orchestrator-owned.
    Conflicted,
}

/// Final result of a guarded base merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseMergeOutcome {
    /// The merge and every required cleanup stage completed.
    Completed,
    /// The merge conflicted and therefore did not run clean-merge cleanup.
    Conflicted,
}

/// Finite identifier for a clean-merge cleanup stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostMergeCleanupStage {
    /// Regenerate track-derived views.
    Views,
    /// Recapture the track baselines.
    Baseline,
    /// Record the base synchronization stamp.
    SyncBaseStamp,
}

/// Command for a guarded base merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseMergeCommand {
    /// Workspace root supplied by the CLI composition boundary.
    pub workspace_root: PathBuf,
}

/// Failures while loading the authoritative base-merge context.
#[derive(Debug, Error)]
pub enum BaseMergeContextError {
    /// The context could not be loaded.
    #[error("base-merge context unavailable: {0}")]
    Unavailable(DiagnosticText),
    /// The current branch differs from the active track branch.
    #[error(
        "base-merge rejected: current branch '{current}' does not match active track '{expected}'"
    )]
    ActiveTrackMismatch {
        /// Current checked-out branch.
        current: TrackBranch,
        /// Track branch materialized for the active track.
        expected: TrackBranch,
    },
}

/// Failures while invoking the guarded git merge.
#[derive(Debug, Error)]
pub enum BaseMergeGitError {
    /// The git invocation could not complete.
    #[error("base merge git operation failed: {0}")]
    Execution(DiagnosticText),
}

/// Error returned by [`BaseMergeService::execute`].
#[derive(Debug, Error)]
pub enum BaseMergeError {
    /// The merge context could not be obtained.
    #[error("base-merge context failed: {0}")]
    Context(DiagnosticText),
    /// The current branch differs from the active track branch.
    #[error(
        "base-merge rejected: current branch '{current}' does not match active track '{expected}'"
    )]
    ActiveTrackMismatch {
        /// Current checked-out branch.
        current: TrackBranch,
        /// Track branch materialized for the active track.
        expected: TrackBranch,
    },
    /// The guarded git merge failed.
    #[error("base-merge git failed: {0}")]
    Git(DiagnosticText),
    /// A required clean-merge cleanup stage failed.
    #[error("base-merge cleanup failed at {stage:?}: {detail}")]
    PostMergeCleanup {
        /// Cleanup stage that returned the error.
        stage: PostMergeCleanupStage,
        /// Opaque diagnostic provided by the cleanup adapter.
        detail: DiagnosticText,
    },
}

/// Loads the authoritative active-track direction for a merge.
pub trait BaseMergeContextPort: Send + Sync {
    /// Loads the only permitted base-to-track merge direction.
    ///
    /// # Errors
    ///
    /// Returns [`BaseMergeContextError`] when the context is unavailable or
    /// the checked-out branch is not the active track branch.
    fn load_direction(
        &self,
        workspace_root: &Path,
    ) -> Result<BaseMergeDirection, BaseMergeContextError>;
}

/// Performs the guarded git merge using an already-derived direction.
pub trait BaseMergeGitPort: Send + Sync {
    /// Attempts the base-to-track merge.
    ///
    /// # Errors
    ///
    /// Returns [`BaseMergeGitError`] when git cannot execute the guarded merge.
    fn merge_base(
        &self,
        workspace_root: &Path,
        direction: &BaseMergeDirection,
    ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError>;
}

/// Runs the cleanup stages that follow only a clean base merge.
pub trait BaseMergeCleanupPort: Send + Sync {
    /// Regenerates the active track's derived views.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when view regeneration fails.
    fn regenerate_views(
        &self,
        workspace_root: &Path,
        track_id: &TrackId,
    ) -> Result<(), DiagnosticText>;

    /// Recaptures the active track's baselines.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when baseline capture fails.
    fn recapture_baselines(
        &self,
        workspace_root: &Path,
        track_id: &TrackId,
    ) -> Result<(), DiagnosticText>;

    /// Records the base synchronization stamp for the active track.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when recording the sync-base stamp fails.
    fn record_sync_base_stamp(
        &self,
        workspace_root: &Path,
        track_id: &TrackId,
    ) -> Result<(), DiagnosticText>;
}

/// Application-service boundary for guarded base merges.
pub trait BaseMergeService: Send + Sync {
    /// Executes a guarded base merge and any clean-merge cleanup.
    ///
    /// # Errors
    ///
    /// Returns [`BaseMergeError`] when context loading, the git merge, or a
    /// required clean-merge cleanup stage fails.
    fn execute(&self, command: BaseMergeCommand) -> Result<BaseMergeOutcome, BaseMergeError>;
}

/// Dependency-injected interactor for [`BaseMergeService`].
pub struct BaseMergeInteractor {
    context: Arc<dyn BaseMergeContextPort>,
    git: Arc<dyn BaseMergeGitPort>,
    cleanup: Arc<dyn BaseMergeCleanupPort>,
}

impl BaseMergeInteractor {
    /// Creates an interactor from its secondary ports.
    #[must_use]
    pub fn new(
        context: Arc<dyn BaseMergeContextPort>,
        git: Arc<dyn BaseMergeGitPort>,
        cleanup: Arc<dyn BaseMergeCleanupPort>,
    ) -> Self {
        Self { context, git, cleanup }
    }
}

impl BaseMergeService for BaseMergeInteractor {
    fn execute(&self, command: BaseMergeCommand) -> Result<BaseMergeOutcome, BaseMergeError> {
        let direction =
            self.context.load_direction(&command.workspace_root).map_err(map_context_error)?;
        let track_id = direction.track_id();

        match self.git.merge_base(&command.workspace_root, &direction).map_err(map_git_error)? {
            BaseMergeAttemptOutcome::Conflicted => Ok(BaseMergeOutcome::Conflicted),
            BaseMergeAttemptOutcome::Clean => {
                self.cleanup.regenerate_views(&command.workspace_root, track_id).map_err(
                    |detail| BaseMergeError::PostMergeCleanup {
                        stage: PostMergeCleanupStage::Views,
                        detail,
                    },
                )?;
                self.cleanup.recapture_baselines(&command.workspace_root, track_id).map_err(
                    |detail| BaseMergeError::PostMergeCleanup {
                        stage: PostMergeCleanupStage::Baseline,
                        detail,
                    },
                )?;
                self.cleanup.record_sync_base_stamp(&command.workspace_root, track_id).map_err(
                    |detail| BaseMergeError::PostMergeCleanup {
                        stage: PostMergeCleanupStage::SyncBaseStamp,
                        detail,
                    },
                )?;
                Ok(BaseMergeOutcome::Completed)
            }
        }
    }
}

fn map_context_error(error: BaseMergeContextError) -> BaseMergeError {
    match error {
        BaseMergeContextError::Unavailable(detail) => BaseMergeError::Context(detail),
        BaseMergeContextError::ActiveTrackMismatch { current, expected } => {
            BaseMergeError::ActiveTrackMismatch { current, expected }
        }
    }
}

fn map_git_error(error: BaseMergeGitError) -> BaseMergeError {
    match error {
        BaseMergeGitError::Execution(detail) => BaseMergeError::Git(detail),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use domain::{BranchStrategySnapshot, MergeMethod, NonEmptyString, TrackMetadata};

    use super::*;

    struct SuccessfulContext {
        direction: BaseMergeDirection,
        workspace_roots: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl SuccessfulContext {
        fn new(direction: BaseMergeDirection) -> Self {
            Self { direction, workspace_roots: Arc::new(Mutex::new(Vec::new())) }
        }

        fn with_workspace_recording(
            direction: BaseMergeDirection,
            workspace_roots: Arc<Mutex<Vec<PathBuf>>>,
        ) -> Self {
            Self { direction, workspace_roots }
        }
    }

    impl BaseMergeContextPort for SuccessfulContext {
        fn load_direction(
            &self,
            workspace_root: &Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            self.workspace_roots.lock().unwrap().push(workspace_root.to_path_buf());
            Ok(self.direction.clone())
        }
    }

    struct UnavailableContext;

    impl BaseMergeContextPort for UnavailableContext {
        fn load_direction(
            &self,
            _workspace_root: &Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            Err(BaseMergeContextError::Unavailable(DiagnosticText::new("metadata unreadable")))
        }
    }

    struct MismatchedContext;

    impl BaseMergeContextPort for MismatchedContext {
        fn load_direction(
            &self,
            _workspace_root: &Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            Err(BaseMergeContextError::ActiveTrackMismatch {
                current: TrackBranch::try_new("track/other-track").unwrap(),
                expected: TrackBranch::try_new("track/merge-track").unwrap(),
            })
        }
    }

    struct SnapshotSourceMismatchContext;

    impl BaseMergeContextPort for SnapshotSourceMismatchContext {
        fn load_direction(
            &self,
            _workspace_root: &Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            Err(BaseMergeContextError::Unavailable(DiagnosticText::new(
                "requested source 'release' differs from snapshot base 'snapshot-base'",
            )))
        }
    }

    struct ReverseDirectionContext;

    impl BaseMergeContextPort for ReverseDirectionContext {
        fn load_direction(
            &self,
            _workspace_root: &Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            Err(BaseMergeContextError::Unavailable(DiagnosticText::new(
                "reverse-direction merge context is rejected",
            )))
        }
    }

    struct NonTrackContext;

    impl BaseMergeContextPort for NonTrackContext {
        fn load_direction(
            &self,
            _workspace_root: &Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            Err(BaseMergeContextError::Unavailable(DiagnosticText::new(
                "non-track current branch context is rejected",
            )))
        }
    }

    #[derive(Clone, Copy)]
    enum GitResponse {
        Clean,
        Conflict,
        Failure,
    }

    struct RecordingGit {
        response: GitResponse,
        calls: Arc<Mutex<Vec<MergeCall>>>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct MergeCall {
        workspace_root: PathBuf,
        source: String,
        target: String,
    }

    impl BaseMergeGitPort for RecordingGit {
        fn merge_base(
            &self,
            workspace_root: &Path,
            direction: &BaseMergeDirection,
        ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
            self.calls.lock().unwrap().push(MergeCall {
                workspace_root: workspace_root.to_path_buf(),
                source: direction.source().as_str().to_owned(),
                target: direction.active_track().as_ref().to_owned(),
            });
            match self.response {
                GitResponse::Clean => Ok(BaseMergeAttemptOutcome::Clean),
                GitResponse::Conflict => Ok(BaseMergeAttemptOutcome::Conflicted),
                GitResponse::Failure => {
                    Err(BaseMergeGitError::Execution(DiagnosticText::new("git failed")))
                }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CleanupCall {
        stage: &'static str,
        workspace_root: PathBuf,
        track_id: TrackId,
    }

    struct RecordingCleanup {
        calls: Arc<Mutex<Vec<CleanupCall>>>,
        failure: Option<PostMergeCleanupStage>,
    }

    impl RecordingCleanup {
        fn record(
            &self,
            stage: PostMergeCleanupStage,
            name: &'static str,
            workspace_root: &Path,
            track_id: &TrackId,
        ) -> Result<(), DiagnosticText> {
            self.calls.lock().unwrap().push(CleanupCall {
                stage: name,
                workspace_root: workspace_root.to_path_buf(),
                track_id: track_id.clone(),
            });
            if self.failure == Some(stage) {
                return Err(DiagnosticText::new("cleanup failed"));
            }
            Ok(())
        }
    }

    impl BaseMergeCleanupPort for RecordingCleanup {
        fn regenerate_views(
            &self,
            workspace_root: &Path,
            track_id: &TrackId,
        ) -> Result<(), DiagnosticText> {
            self.record(PostMergeCleanupStage::Views, "views", workspace_root, track_id)
        }

        fn recapture_baselines(
            &self,
            workspace_root: &Path,
            track_id: &TrackId,
        ) -> Result<(), DiagnosticText> {
            self.record(PostMergeCleanupStage::Baseline, "baseline", workspace_root, track_id)
        }

        fn record_sync_base_stamp(
            &self,
            workspace_root: &Path,
            track_id: &TrackId,
        ) -> Result<(), DiagnosticText> {
            self.record(
                PostMergeCleanupStage::SyncBaseStamp,
                "sync-base-stamp",
                workspace_root,
                track_id,
            )
        }
    }

    struct SnapshotDirectionFixture {
        direction: BaseMergeDirection,
        snapshot_base: String,
        active_track: String,
    }

    fn snapshot_direction_fixture() -> SnapshotDirectionFixture {
        let snapshot_base = NonEmptyString::try_new("snapshot-base").unwrap();
        let expected_snapshot_base = snapshot_base.as_ref().to_owned();
        let active_track = TrackBranch::try_new("track/merge-track").unwrap();
        let expected_active_track = active_track.as_ref().to_owned();
        let track = TrackMetadata::with_branch(
            TrackId::try_new("merge-track").unwrap(),
            Some(active_track),
            "Merge track",
            None,
            BranchStrategySnapshot::new(
                snapshot_base,
                NonEmptyString::try_new("develop").unwrap(),
                MergeMethod::Merge,
            ),
        )
        .unwrap();
        SnapshotDirectionFixture {
            direction: domain::derive_base_merge_direction(&track).unwrap(),
            snapshot_base: expected_snapshot_base,
            active_track: expected_active_track,
        }
    }

    fn direction() -> BaseMergeDirection {
        snapshot_direction_fixture().direction
    }

    fn command() -> BaseMergeCommand {
        BaseMergeCommand { workspace_root: PathBuf::from("/workspace") }
    }

    fn cleanup_call(stage: &'static str) -> CleanupCall {
        CleanupCall {
            stage,
            workspace_root: PathBuf::from("/workspace"),
            track_id: TrackId::try_new("merge-track").unwrap(),
        }
    }

    #[test]
    fn test_base_merge_execute_clean_passes_snapshot_direction_and_workspace_to_git_port() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let fixture = snapshot_direction_fixture();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::with_workspace_recording(
                fixture.direction,
                Arc::clone(&context_workspaces),
            )),
            Arc::new(RecordingGit { response: GitResponse::Clean, calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let outcome = interactor.execute(command()).unwrap();

        assert_eq!(outcome, BaseMergeOutcome::Completed);
        let calls = git_calls.lock().unwrap();
        assert_eq!(
            calls.as_slice(),
            &[MergeCall {
                workspace_root: PathBuf::from("/workspace"),
                source: fixture.snapshot_base,
                target: fixture.active_track,
            }]
        );
        assert_eq!(*context_workspaces.lock().unwrap(), vec![PathBuf::from("/workspace")]);
        assert_eq!(
            *cleanup_calls.lock().unwrap(),
            vec![cleanup_call("views"), cleanup_call("baseline"), cleanup_call("sync-base-stamp")]
        );
    }

    #[test]
    fn test_base_merge_execute_conflict_skips_clean_merge_cleanup() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Conflict,
                calls: Arc::clone(&git_calls),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let outcome = interactor.execute(command()).unwrap();

        assert_eq!(outcome, BaseMergeOutcome::Conflicted);
        assert_eq!(git_calls.lock().unwrap().len(), 1);
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_unavailable_context_returns_context_error() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(UnavailableContext),
            Arc::new(RecordingGit { response: GitResponse::Clean, calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(
            matches!(error, BaseMergeError::Context(detail) if detail.as_str() == "metadata unreadable")
        );
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_active_track_mismatch_returns_typed_error() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(MismatchedContext),
            Arc::new(RecordingGit { response: GitResponse::Clean, calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(error, BaseMergeError::ActiveTrackMismatch { current, expected }
            if current.as_ref() == "track/other-track" && expected.as_ref() == "track/merge-track"));
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_snapshot_source_mismatch_context_rejection_skips_git_and_cleanup() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SnapshotSourceMismatchContext),
            Arc::new(RecordingGit { response: GitResponse::Clean, calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(error, BaseMergeError::Context(detail)
            if detail.as_str() == "requested source 'release' differs from snapshot base 'snapshot-base'"));
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_reverse_direction_context_rejection_skips_git_and_cleanup() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(ReverseDirectionContext),
            Arc::new(RecordingGit { response: GitResponse::Clean, calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(error, BaseMergeError::Context(detail)
            if detail.as_str() == "reverse-direction merge context is rejected"));
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_non_track_context_rejection_skips_git_and_cleanup() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(NonTrackContext),
            Arc::new(RecordingGit { response: GitResponse::Clean, calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(error, BaseMergeError::Context(detail)
            if detail.as_str() == "non-track current branch context is rejected"));
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_git_failure_returns_git_error() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Failure,
                calls: Arc::clone(&git_calls),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(error, BaseMergeError::Git(detail) if detail.as_str() == "git failed"));
        assert_eq!(git_calls.lock().unwrap().len(), 1);
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_cleanup_failure_preserves_each_failed_stage() {
        for (failure, expected_calls) in [
            (PostMergeCleanupStage::Views, vec![cleanup_call("views")]),
            (
                PostMergeCleanupStage::Baseline,
                vec![cleanup_call("views"), cleanup_call("baseline")],
            ),
            (
                PostMergeCleanupStage::SyncBaseStamp,
                vec![
                    cleanup_call("views"),
                    cleanup_call("baseline"),
                    cleanup_call("sync-base-stamp"),
                ],
            ),
        ] {
            let git_calls = Arc::new(Mutex::new(Vec::new()));
            let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
            let interactor = BaseMergeInteractor::new(
                Arc::new(SuccessfulContext::new(direction())),
                Arc::new(RecordingGit {
                    response: GitResponse::Clean,
                    calls: Arc::clone(&git_calls),
                }),
                Arc::new(RecordingCleanup {
                    calls: Arc::clone(&cleanup_calls),
                    failure: Some(failure),
                }),
            );

            let error = interactor.execute(command()).unwrap_err();

            assert!(matches!(error, BaseMergeError::PostMergeCleanup { stage, detail }
                if stage == failure && detail.as_str() == "cleanup failed"));
            assert_eq!(git_calls.lock().unwrap().len(), 1);
            assert_eq!(*cleanup_calls.lock().unwrap(), expected_calls);
        }
    }
}
