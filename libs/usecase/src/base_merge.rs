//! Base-to-track merge application service.
//!
//! This module coordinates the guarded merge through injected ports.  It does
//! not inspect the filesystem or invoke git itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::branch_strategy::{BaseBranchName, BaseMergeDirection};
use domain::{CommitHash, TrackBranch, TrackId};
use thiserror::Error;

use crate::git_workflow::DiagnosticText;

/// Pre-cleanup result returned by the git-process port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseMergeAttemptOutcome {
    /// The base branch was merged without conflicts at this exact commit.
    Clean {
        /// The exact base commit incorporated by the successful merge.
        base_commit: CommitHash,
    },
    /// Git reported conflicts at this exact base commit.
    Conflicted {
        /// The exact base commit recorded by `MERGE_HEAD`.
        base_commit: CommitHash,
    },
}

/// Final result of a guarded base merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseMergeOutcome {
    /// The merge and every required cleanup stage completed.
    Completed,
    /// The merge conflicted after completing the ordered cleanup stages.
    Conflicted,
}

/// Command for a guarded base merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseMergeCommand {
    /// Workspace root supplied by the CLI composition boundary.
    pub workspace_root: PathBuf,
}

/// Request passed to every post-merge cleanup stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseMergeCleanupRequest {
    /// Workspace containing the active track.
    pub workspace_root: PathBuf,
    /// Active track identity.
    pub track_id: TrackId,
    /// Base branch incorporated by the guarded merge.
    pub base_branch: BaseBranchName,
    /// Exact base commit incorporated by the guarded merge.
    pub base_commit: CommitHash,
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
    /// The worktree contains tracked or non-ignored untracked changes.
    #[error("base-merge rejected: working tree is not clean: {0}")]
    DirtyWorktree(DiagnosticText),
}

/// Failures while regenerating derived views.
#[derive(Debug, Error)]
pub enum ViewsRegenerationError {
    /// The view-regeneration command failed.
    #[error("view regeneration failed: {0}")]
    Regeneration(DiagnosticText),
}

/// Failures while replacing commit-pinned baselines.
#[derive(Debug, Error)]
pub enum BaselineReplacementError {
    /// The isolated worktree could not be prepared.
    #[error("baseline isolation failed: {0}")]
    Isolation(DiagnosticText),
    /// Baseline generation failed.
    #[error("baseline generation failed: {0}")]
    Generation(DiagnosticText),
    /// Generated baseline validation failed.
    #[error("baseline validation failed: {0}")]
    Validation(DiagnosticText),
    /// Atomic baseline publication failed.
    #[error("baseline publication failed: {0}")]
    Publish(DiagnosticText),
}

/// Error type for the ordered post-merge cleanup stages.
#[derive(Debug, Error)]
pub enum PostMergeCleanupError {
    /// Derived-view regeneration failed.
    #[error("view regeneration failed: {0}")]
    Views(ViewsRegenerationError),
    /// Baseline replacement failed.
    #[error("baseline replacement failed: {0}")]
    Baseline(BaselineReplacementError),
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
    /// The worktree was dirty before the guarded merge could start.
    #[error(
        "base-merge rejected: working tree is not clean ({0}); commit changes or use the guarded stash surface (`bin/sotp git stash`)"
    )]
    DirtyWorktree(DiagnosticText),
    /// A required clean-merge cleanup stage failed.
    #[error("base-merge cleanup failed: {0}")]
    PostMergeCleanup(PostMergeCleanupError),
    /// The merge is already conflicted, but conflict cleanup failed.
    #[error("base-merge conflicted; cleanup failed: {0}")]
    ConflictedCleanupFailed(PostMergeCleanupError),
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
    /// Rejects a worktree that has tracked or non-ignored untracked changes.
    ///
    /// # Errors
    ///
    /// Returns [`BaseMergeGitError::DirtyWorktree`] when the status probe finds
    /// changes, or [`BaseMergeGitError::Execution`] when the probe cannot
    /// complete.
    fn ensure_worktree_clean(&self, workspace_root: &Path) -> Result<(), BaseMergeGitError>;

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

/// Runs the ordered cleanup stages that follow a base merge.
pub trait BaseMergeCleanupPort: Send + Sync {
    /// Regenerates the active track's derived views.
    ///
    /// # Errors
    ///
    /// Returns [`ViewsRegenerationError`] when view regeneration fails.
    fn regenerate_views(
        &self,
        request: &BaseMergeCleanupRequest,
    ) -> Result<(), ViewsRegenerationError>;

    /// Atomically replaces the active track's baselines.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineReplacementError`] when baseline replacement fails.
    fn replace_baselines(
        &self,
        request: &BaseMergeCleanupRequest,
    ) -> Result<(), BaselineReplacementError>;
}

/// Application-service boundary for guarded base merges.
pub trait BaseMergeService: Send + Sync {
    /// Executes a guarded base merge and its post-merge cleanup.
    ///
    /// # Errors
    ///
    /// Returns [`BaseMergeError`] when context loading, the worktree probe, the
    /// git merge, or a required clean-merge cleanup stage fails.
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
        self.git.ensure_worktree_clean(&command.workspace_root).map_err(map_git_error)?;

        let direction =
            self.context.load_direction(&command.workspace_root).map_err(map_context_error)?;

        match self.git.merge_base(&command.workspace_root, &direction).map_err(map_git_error)? {
            BaseMergeAttemptOutcome::Conflicted { base_commit } => {
                let request = BaseMergeCleanupRequest {
                    workspace_root: command.workspace_root.clone(),
                    track_id: direction.track_id().clone(),
                    base_branch: direction.source().clone(),
                    base_commit,
                };
                self.cleanup.replace_baselines(&request).map_err(|error| {
                    BaseMergeError::ConflictedCleanupFailed(PostMergeCleanupError::Baseline(error))
                })?;
                self.cleanup.regenerate_views(&request).map_err(|error| {
                    BaseMergeError::ConflictedCleanupFailed(PostMergeCleanupError::Views(error))
                })?;
                Ok(BaseMergeOutcome::Conflicted)
            }
            BaseMergeAttemptOutcome::Clean { base_commit } => {
                let request = BaseMergeCleanupRequest {
                    workspace_root: command.workspace_root,
                    track_id: direction.track_id().clone(),
                    base_branch: direction.source().clone(),
                    base_commit,
                };
                self.cleanup.replace_baselines(&request).map_err(|error| {
                    BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Baseline(error))
                })?;
                self.cleanup.regenerate_views(&request).map_err(|error| {
                    BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Views(error))
                })?;
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
        BaseMergeGitError::DirtyWorktree(detail) => BaseMergeError::DirtyWorktree(detail),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use domain::{
        BaseBranchName, BranchStrategySnapshot, MergeMethod, NonEmptyString, TrackMetadata,
    };

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CleanupStage {
        Baseline,
        Views,
    }

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
        fn ensure_worktree_clean(&self, _workspace_root: &Path) -> Result<(), BaseMergeGitError> {
            Ok(())
        }

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
                GitResponse::Clean => Ok(BaseMergeAttemptOutcome::Clean {
                    base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
                }),
                GitResponse::Conflict => Ok(BaseMergeAttemptOutcome::Conflicted {
                    base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
                }),
                GitResponse::Failure => {
                    Err(BaseMergeGitError::Execution(DiagnosticText::new("git failed")))
                }
            }
        }
    }

    struct ExactCommitGit {
        base_commit: CommitHash,
        calls: Arc<Mutex<Vec<MergeCall>>>,
    }

    impl BaseMergeGitPort for ExactCommitGit {
        fn ensure_worktree_clean(&self, _workspace_root: &Path) -> Result<(), BaseMergeGitError> {
            Ok(())
        }

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
            Ok(BaseMergeAttemptOutcome::Clean { base_commit: self.base_commit.clone() })
        }
    }

    struct ExactConflictGit {
        base_commit: CommitHash,
        calls: Arc<Mutex<Vec<MergeCall>>>,
    }

    impl BaseMergeGitPort for ExactConflictGit {
        fn ensure_worktree_clean(&self, _workspace_root: &Path) -> Result<(), BaseMergeGitError> {
            Ok(())
        }

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
            Ok(BaseMergeAttemptOutcome::Conflicted { base_commit: self.base_commit.clone() })
        }
    }

    #[derive(Clone, Copy)]
    enum PreflightResponse {
        Dirty,
        Failure,
    }

    struct PreflightGit {
        response: PreflightResponse,
        merge_calls: Arc<Mutex<usize>>,
    }

    impl BaseMergeGitPort for PreflightGit {
        fn ensure_worktree_clean(&self, _workspace_root: &Path) -> Result<(), BaseMergeGitError> {
            match self.response {
                PreflightResponse::Dirty => {
                    Err(BaseMergeGitError::DirtyWorktree(DiagnosticText::new(" M tracked.txt")))
                }
                PreflightResponse::Failure => {
                    Err(BaseMergeGitError::Execution(DiagnosticText::new("status probe failed")))
                }
            }
        }

        fn merge_base(
            &self,
            _workspace_root: &Path,
            _direction: &BaseMergeDirection,
        ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
            *self.merge_calls.lock().unwrap() += 1;
            Err(BaseMergeGitError::Execution(DiagnosticText::new(
                "merge must not be attempted after a failed preflight",
            )))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CleanupCall {
        stage: CleanupStage,
        request: BaseMergeCleanupRequest,
    }

    struct RecordingCleanup {
        calls: Arc<Mutex<Vec<CleanupCall>>>,
        failure: Option<CleanupStage>,
    }

    impl RecordingCleanup {
        fn record(
            &self,
            stage: CleanupStage,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), DiagnosticText> {
            let mut calls = self.calls.lock().unwrap();
            calls.push(CleanupCall { stage, request: request.clone() });
            if self.failure == Some(stage) {
                return Err(DiagnosticText::new("cleanup failed"));
            }
            Ok(())
        }
    }

    impl BaseMergeCleanupPort for RecordingCleanup {
        fn regenerate_views(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), ViewsRegenerationError> {
            self.record(CleanupStage::Views, request).map_err(ViewsRegenerationError::Regeneration)
        }

        fn replace_baselines(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), BaselineReplacementError> {
            self.record(CleanupStage::Baseline, request)
                .map_err(BaselineReplacementError::Generation)
        }
    }

    struct StatefulCleanup {
        state: Arc<Mutex<StatefulCleanupState>>,
        baseline_failure: Option<BaselineFailure>,
    }

    #[derive(Debug, Clone, Copy)]
    enum BaselineFailure {
        Isolation,
        Generation,
        Validation,
        Publish,
    }

    #[derive(Debug)]
    struct StatefulCleanupState {
        calls: Vec<CleanupCall>,
        active_baseline: String,
        type_signals_cache: String,
    }

    impl StatefulCleanup {
        fn record_call(&self, stage: CleanupStage, request: &BaseMergeCleanupRequest) {
            let mut state = self.state.lock().unwrap();
            state.calls.push(CleanupCall { stage, request: request.clone() });
        }
    }

    impl BaseMergeCleanupPort for StatefulCleanup {
        fn regenerate_views(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), ViewsRegenerationError> {
            self.record_call(CleanupStage::Views, request);
            Ok(())
        }

        fn replace_baselines(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), BaselineReplacementError> {
            let mut state = self.state.lock().unwrap();
            state
                .calls
                .push(CleanupCall { stage: CleanupStage::Baseline, request: request.clone() });
            if let Some(failure) = self.baseline_failure {
                return Err(match failure {
                    BaselineFailure::Isolation => BaselineReplacementError::Isolation(
                        DiagnosticText::new("baseline isolation failed"),
                    ),
                    BaselineFailure::Generation => BaselineReplacementError::Generation(
                        DiagnosticText::new("baseline generation failed"),
                    ),
                    BaselineFailure::Validation => BaselineReplacementError::Validation(
                        DiagnosticText::new("baseline validation failed"),
                    ),
                    BaselineFailure::Publish => BaselineReplacementError::Publish(
                        DiagnosticText::new("replacement publication failed"),
                    ),
                });
            }
            state.active_baseline = format!("baseline@{}", request.base_commit.as_ref());
            Ok(())
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

    fn cleanup_request() -> BaseMergeCleanupRequest {
        BaseMergeCleanupRequest {
            workspace_root: PathBuf::from("/workspace"),
            track_id: TrackId::try_new("merge-track").unwrap(),
            base_branch: BaseBranchName::try_new("snapshot-base".to_owned()).unwrap(),
            base_commit: CommitHash::try_new("0123456789abcdef").unwrap(),
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
        assert_eq!(
            git_calls.lock().unwrap().as_slice(),
            &[MergeCall {
                workspace_root: PathBuf::from("/workspace"),
                source: fixture.snapshot_base,
                target: fixture.active_track,
            }]
        );
        assert_eq!(*context_workspaces.lock().unwrap(), vec![PathBuf::from("/workspace")]);
        assert_eq!(
            *cleanup_calls.lock().unwrap(),
            vec![
                CleanupCall { stage: CleanupStage::Baseline, request: cleanup_request() },
                CleanupCall { stage: CleanupStage::Views, request: cleanup_request() },
            ]
        );
    }

    #[test]
    fn test_base_merge_execute_clean_propagates_exact_commit_to_every_cleanup_request() {
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let exact_base_commit = CommitHash::try_new("fedcba9876543210").unwrap();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(ExactCommitGit {
                base_commit: exact_base_commit.clone(),
                calls: Arc::clone(&git_calls),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let outcome = interactor.execute(command()).unwrap();

        assert_eq!(outcome, BaseMergeOutcome::Completed);
        assert_eq!(git_calls.lock().unwrap().len(), 1);
        let cleanup_calls = cleanup_calls.lock().unwrap();
        assert_eq!(cleanup_calls.len(), 2);
        assert_eq!(
            cleanup_calls.iter().map(|call| call.stage).collect::<Vec<_>>(),
            vec![CleanupStage::Baseline, CleanupStage::Views,]
        );
        for call in cleanup_calls.iter() {
            assert_eq!(call.request.workspace_root, PathBuf::from("/workspace"));
            assert_eq!(call.request.track_id.as_ref(), "merge-track");
            assert_eq!(call.request.base_branch.as_str(), "snapshot-base");
            assert_eq!(call.request.base_commit, exact_base_commit);
        }
    }

    #[test]
    fn test_base_merge_execute_clean_regenerates_views_once_after_baseline() {
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Clean,
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        assert_eq!(interactor.execute(command()).unwrap(), BaseMergeOutcome::Completed);

        let stages =
            cleanup_calls.lock().unwrap().iter().map(|call| call.stage).collect::<Vec<_>>();
        assert_eq!(stages, vec![CleanupStage::Baseline, CleanupStage::Views]);
        assert_eq!(stages.iter().filter(|stage| **stage == CleanupStage::Views).count(), 1);
    }

    #[test]
    fn test_base_merge_execute_conflict_replaces_baseline_then_regenerates_views() {
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
        assert_eq!(
            cleanup_calls.lock().unwrap().as_slice(),
            &[
                CleanupCall { stage: CleanupStage::Baseline, request: cleanup_request() },
                CleanupCall { stage: CleanupStage::Views, request: cleanup_request() },
            ]
        );
    }

    #[test]
    fn test_base_merge_execute_conflict_passes_exact_commit_to_baseline_and_views() {
        let exact_base_commit = CommitHash::try_new("fedcba9876543210").unwrap();
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(ExactConflictGit {
                base_commit: exact_base_commit.clone(),
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        assert_eq!(interactor.execute(command()).unwrap(), BaseMergeOutcome::Conflicted);

        let cleanup_calls = cleanup_calls.lock().unwrap();
        assert_eq!(cleanup_calls.len(), 2);
        let baseline_call = cleanup_calls.first().unwrap();
        let views_call = cleanup_calls.get(1).unwrap();
        assert_eq!(baseline_call.stage, CleanupStage::Baseline);
        assert_eq!(views_call.stage, CleanupStage::Views);
        assert!(cleanup_calls.iter().all(|call| call.request.base_commit == exact_base_commit));
        assert!(
            cleanup_calls.iter().all(|call| call.request.base_branch.as_str() == "snapshot-base")
        );
    }

    #[test]
    fn test_base_merge_execute_conflict_baseline_failure_is_reported_without_completion() {
        let exact_base_commit = CommitHash::try_new("fedcba9876543210").unwrap();
        let state = stateful_cleanup_state();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(ExactConflictGit {
                base_commit: exact_base_commit.clone(),
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            stateful_cleanup(Arc::clone(&state), Some(BaselineFailure::Generation)),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(
            error,
            BaseMergeError::ConflictedCleanupFailed(PostMergeCleanupError::Baseline(
                BaselineReplacementError::Generation(detail)
            )) if detail.as_str() == "baseline generation failed"
        ));
        let state = state.lock().unwrap();
        let only_call = state.calls.first().unwrap();
        assert_eq!(state.calls.len(), 1);
        assert_eq!(only_call.stage, CleanupStage::Baseline);
        assert_eq!(only_call.request.base_commit, exact_base_commit);
    }

    #[test]
    fn test_base_merge_execute_conflict_views_failure_is_reported() {
        let exact_base_commit = CommitHash::try_new("fedcba9876543210").unwrap();
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(ExactConflictGit {
                base_commit: exact_base_commit.clone(),
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            Arc::new(RecordingCleanup {
                calls: Arc::clone(&cleanup_calls),
                failure: Some(CleanupStage::Views),
            }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(
            error,
            BaseMergeError::ConflictedCleanupFailed(PostMergeCleanupError::Views(
                ViewsRegenerationError::Regeneration(detail)
            )) if detail.as_str() == "cleanup failed"
        ));
        let cleanup_calls = cleanup_calls.lock().unwrap();
        assert_eq!(
            cleanup_calls.iter().map(|call| call.stage).collect::<Vec<_>>(),
            vec![CleanupStage::Baseline, CleanupStage::Views]
        );
        assert!(cleanup_calls.iter().all(|call| call.request.base_commit == exact_base_commit));
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
    fn test_base_merge_execute_dirty_worktree_rejection_precedes_merge_attempt() {
        let merge_calls = Arc::new(Mutex::new(0));
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::with_workspace_recording(
                direction(),
                Arc::clone(&context_workspaces),
            )),
            Arc::new(PreflightGit {
                response: PreflightResponse::Dirty,
                merge_calls: Arc::clone(&merge_calls),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(&error, BaseMergeError::DirtyWorktree(detail)
            if detail.as_str() == " M tracked.txt"));
        assert!(error.to_string().contains("commit changes"));
        assert!(error.to_string().contains("bin/sotp git stash"));
        assert!(context_workspaces.lock().unwrap().is_empty());
        assert_eq!(*merge_calls.lock().unwrap(), 0);
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_base_merge_execute_worktree_probe_failure_fails_closed_before_merge_attempt() {
        let merge_calls = Arc::new(Mutex::new(0));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(PreflightGit {
                response: PreflightResponse::Failure,
                merge_calls: Arc::clone(&merge_calls),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let error = interactor.execute(command()).unwrap_err();

        assert!(matches!(&error, BaseMergeError::Git(detail)
            if detail.as_str() == "status probe failed"));
        assert_eq!(*merge_calls.lock().unwrap(), 0);
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
        for (failure, expected_stages) in [
            (CleanupStage::Baseline, vec![CleanupStage::Baseline]),
            (CleanupStage::Views, vec![CleanupStage::Baseline, CleanupStage::Views]),
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

            assert!(match (failure, error) {
                (
                    CleanupStage::Views,
                    BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Views(error)),
                ) => {
                    matches!(error, ViewsRegenerationError::Regeneration(detail) if detail.as_str() == "cleanup failed")
                }
                (
                    CleanupStage::Baseline,
                    BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Baseline(error)),
                ) => {
                    matches!(error, BaselineReplacementError::Generation(detail) if detail.as_str() == "cleanup failed")
                }
                _ => false,
            });
            assert_eq!(git_calls.lock().unwrap().len(), 1);
            assert_eq!(
                cleanup_calls.lock().unwrap().iter().map(|call| call.stage).collect::<Vec<_>>(),
                expected_stages
            );
        }
    }

    fn stateful_cleanup_state() -> Arc<Mutex<StatefulCleanupState>> {
        Arc::new(Mutex::new(StatefulCleanupState {
            calls: Vec::new(),
            active_baseline: "prior-valid-baseline".to_owned(),
            type_signals_cache: "current-type-signals-cache".to_owned(),
        }))
    }

    fn stateful_cleanup(
        state: Arc<Mutex<StatefulCleanupState>>,
        baseline_failure: Option<BaselineFailure>,
    ) -> Arc<StatefulCleanup> {
        Arc::new(StatefulCleanup { state, baseline_failure })
    }

    #[test]
    fn test_base_merge_execute_baseline_replacement_preserves_type_signals_cache() {
        let state = stateful_cleanup_state();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Clean,
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            stateful_cleanup(Arc::clone(&state), None),
        );

        assert_eq!(interactor.execute(command()).unwrap(), BaseMergeOutcome::Completed);

        let state = state.lock().unwrap();
        assert_eq!(state.active_baseline, "baseline@0123456789abcdef");
        assert_eq!(state.type_signals_cache, "current-type-signals-cache");
    }

    #[test]
    fn test_base_merge_execute_baseline_isolation_failure_preserves_prior_baseline() {
        let state = stateful_cleanup_state();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Clean,
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            stateful_cleanup(Arc::clone(&state), Some(BaselineFailure::Isolation)),
        );

        let result = interactor.execute(command());

        assert!(matches!(result, Err(BaseMergeError::PostMergeCleanup(
            PostMergeCleanupError::Baseline(BaselineReplacementError::Isolation(detail))
        )) if detail.as_str() == "baseline isolation failed"));
        let state = state.lock().unwrap();
        assert_eq!(state.active_baseline, "prior-valid-baseline");
        assert_eq!(
            state.calls.iter().map(|call| call.stage).collect::<Vec<_>>(),
            vec![CleanupStage::Baseline]
        );
    }

    #[test]
    fn test_base_merge_execute_baseline_generation_validation_and_publish_failures_preserve_prior_baseline()
     {
        for failure in
            [BaselineFailure::Generation, BaselineFailure::Validation, BaselineFailure::Publish]
        {
            let state = stateful_cleanup_state();
            let interactor = BaseMergeInteractor::new(
                Arc::new(SuccessfulContext::new(direction())),
                Arc::new(RecordingGit {
                    response: GitResponse::Clean,
                    calls: Arc::new(Mutex::new(Vec::new())),
                }),
                stateful_cleanup(Arc::clone(&state), Some(failure)),
            );

            let result = interactor.execute(command());

            assert!(match (failure, result) {
                (
                    BaselineFailure::Generation,
                    Err(BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Baseline(
                        BaselineReplacementError::Generation(detail),
                    ))),
                ) => detail.as_str() == "baseline generation failed",
                (
                    BaselineFailure::Validation,
                    Err(BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Baseline(
                        BaselineReplacementError::Validation(detail),
                    ))),
                ) => detail.as_str() == "baseline validation failed",
                (
                    BaselineFailure::Publish,
                    Err(BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Baseline(
                        BaselineReplacementError::Publish(detail),
                    ))),
                ) => detail.as_str() == "replacement publication failed",
                _ => false,
            });
            let state = state.lock().unwrap();
            assert_eq!(state.active_baseline, "prior-valid-baseline");
            assert_eq!(
                state.calls.iter().map(|call| call.stage).collect::<Vec<_>>(),
                vec![CleanupStage::Baseline]
            );
        }
    }

    #[test]
    fn test_base_merge_attempt_outcome_clean_completes_after_all_cleanup_stages() {
        let exact_base_commit = CommitHash::try_new("fedcba9876543210").unwrap();
        let attempt = BaseMergeAttemptOutcome::Clean { base_commit: exact_base_commit.clone() };
        assert!(matches!(attempt, BaseMergeAttemptOutcome::Clean { base_commit }
            if base_commit == exact_base_commit));

        let state = stateful_cleanup_state();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(ExactCommitGit {
                base_commit: exact_base_commit.clone(),
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            stateful_cleanup(Arc::clone(&state), None),
        );

        assert_eq!(interactor.execute(command()).unwrap(), BaseMergeOutcome::Completed);

        let state = state.lock().unwrap();
        assert_eq!(
            state.calls.iter().map(|call| call.stage).collect::<Vec<_>>(),
            vec![CleanupStage::Baseline, CleanupStage::Views,]
        );
        assert!(state.calls.iter().all(|call| call.request.base_commit == exact_base_commit));
    }

    #[test]
    fn test_base_merge_attempt_outcome_conflicted_runs_baseline_then_views() {
        let exact_base_commit = CommitHash::try_new("fedcba9876543210").unwrap();
        let attempt =
            BaseMergeAttemptOutcome::Conflicted { base_commit: exact_base_commit.clone() };
        assert!(matches!(attempt, BaseMergeAttemptOutcome::Conflicted { base_commit }
            if base_commit == exact_base_commit));

        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Conflict,
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        assert_eq!(interactor.execute(command()).unwrap(), BaseMergeOutcome::Conflicted);
        assert_eq!(
            cleanup_calls.lock().unwrap().as_slice(),
            &[
                CleanupCall { stage: CleanupStage::Baseline, request: cleanup_request() },
                CleanupCall { stage: CleanupStage::Views, request: cleanup_request() },
            ]
        );
    }

    #[test]
    fn test_base_merge_cleanup_request_failure_preserves_prior_state_and_never_completes() {
        let state = stateful_cleanup_state();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Clean,
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            stateful_cleanup(Arc::clone(&state), Some(BaselineFailure::Generation)),
        );

        let result = interactor.execute(command());

        assert!(!matches!(result, Ok(BaseMergeOutcome::Completed)));
        assert!(matches!(result, Err(BaseMergeError::PostMergeCleanup(
            PostMergeCleanupError::Baseline(BaselineReplacementError::Generation(detail))
        )) if detail.as_str() == "baseline generation failed"));
        let state = state.lock().unwrap();
        assert_eq!(state.active_baseline, "prior-valid-baseline");
        assert_eq!(
            state.calls.as_slice(),
            [CleanupCall { stage: CleanupStage::Baseline, request: cleanup_request() }]
        );
    }

    #[test]
    fn test_base_merge_error_variants_are_distinct() {
        assert!(matches!(
            BaseMergeError::Context(DiagnosticText::new("context failed")),
            BaseMergeError::Context(detail) if detail.as_str() == "context failed"
        ));
        assert!(matches!(
            BaseMergeError::ActiveTrackMismatch {
                current: TrackBranch::try_new("track/current").unwrap(),
                expected: TrackBranch::try_new("track/expected").unwrap(),
            },
            BaseMergeError::ActiveTrackMismatch { current, expected }
                if current.as_ref() == "track/current" && expected.as_ref() == "track/expected"
        ));
        assert!(matches!(
            BaseMergeError::Git(DiagnosticText::new("git failed")),
            BaseMergeError::Git(detail) if detail.as_str() == "git failed"
        ));
        assert!(matches!(
            BaseMergeError::DirtyWorktree(DiagnosticText::new(" M tracked.txt")),
            BaseMergeError::DirtyWorktree(detail) if detail.as_str() == " M tracked.txt"
        ));
        assert!(matches!(
            BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Views(
                ViewsRegenerationError::Regeneration(DiagnosticText::new("views failed")),
            )),
            BaseMergeError::PostMergeCleanup(PostMergeCleanupError::Views(
                ViewsRegenerationError::Regeneration(detail),
            )) if detail.as_str() == "views failed"
        ));
        assert!(matches!(
            BaseMergeError::ConflictedCleanupFailed(PostMergeCleanupError::Views(
                ViewsRegenerationError::Regeneration(DiagnosticText::new("views failed")),
            )),
            BaseMergeError::ConflictedCleanupFailed(PostMergeCleanupError::Views(
                ViewsRegenerationError::Regeneration(detail),
            )) if detail.as_str() == "views failed"
        ));
    }

    #[test]
    fn test_base_merge_outcome_completed_after_all_ordered_cleanup_successes() {
        let state = stateful_cleanup_state();
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Clean,
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            stateful_cleanup(Arc::clone(&state), None),
        );

        let outcome = interactor.execute(command()).unwrap();

        assert!(matches!(outcome, BaseMergeOutcome::Completed));
        assert_eq!(
            state.lock().unwrap().calls.iter().map(|call| call.stage).collect::<Vec<_>>(),
            vec![CleanupStage::Baseline, CleanupStage::Views,]
        );
    }

    #[test]
    fn test_base_merge_outcome_conflicted_runs_baseline_then_views() {
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = BaseMergeInteractor::new(
            Arc::new(SuccessfulContext::new(direction())),
            Arc::new(RecordingGit {
                response: GitResponse::Conflict,
                calls: Arc::new(Mutex::new(Vec::new())),
            }),
            Arc::new(RecordingCleanup { calls: Arc::clone(&cleanup_calls), failure: None }),
        );

        let outcome = interactor.execute(command()).unwrap();

        assert!(matches!(outcome, BaseMergeOutcome::Conflicted));
        assert_eq!(
            cleanup_calls.lock().unwrap().as_slice(),
            &[
                CleanupCall { stage: CleanupStage::Baseline, request: cleanup_request() },
                CleanupCall { stage: CleanupStage::Views, request: cleanup_request() },
            ]
        );
    }

    #[test]
    fn test_post_merge_cleanup_error_variants_match_declared_nested_errors() {
        assert!(matches!(
            PostMergeCleanupError::Views(ViewsRegenerationError::Regeneration(DiagnosticText::new(
                "views failed",
            ))),
            PostMergeCleanupError::Views(ViewsRegenerationError::Regeneration(detail))
                if detail.as_str() == "views failed"
        ));
        assert!(matches!(
            PostMergeCleanupError::Baseline(BaselineReplacementError::Validation(
                DiagnosticText::new("baseline failed"),
            )),
            PostMergeCleanupError::Baseline(BaselineReplacementError::Validation(detail))
                if detail.as_str() == "baseline failed"
        ));
    }
}
