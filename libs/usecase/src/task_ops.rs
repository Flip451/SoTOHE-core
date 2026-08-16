//! Track task operation and query application services (usecase layer).
//!
//! Wraps `domain::TransitionTaskUseCase`, `domain::AddTaskUseCase`, and
//! `domain::SetOverrideUseCase` behind usecase-owned service traits so the
//! CLI never imports `domain::TrackId`, `domain::TaskId`,
//! `domain::CommitHash`, `domain::StatusOverride`, `domain::DomainError`,
//! `domain::TrackReadError`, or `domain::TrackWriteError` directly (CN-01 / D1).

mod branch_guard;
mod commit_record;
mod ports;

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use domain::{
    CommitHash, DomainError, ImplPlanReader, ImplPlanWriter, RepositoryError, StatusOverride,
    TaskId, TrackId, TrackReadError, TrackReader, TrackWriteError, TrackWriter, TransitionError,
    derive_track_status,
};

use crate::batch_plan::{BatchPlanReaderPort, ScopeConfigReaderPort, ScopeDiffMeasurePort};
use crate::task_admission::{AdmissionPorts, judge_admission};
use crate::track_resolution::BranchReaderPort;
use branch_guard::enforce_branch_guard;

pub use crate::task_admission::{AdmissionRejectionOutput, TaskTransitionOutcome};
// The record-time verification contract is part of this module's type
// contract; it lives in its own file only for module-size reasons.
pub use ports::{CommitRecordVerifierPort, CommitRecordVerifyError};

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// DTO returned by track task operations (transition, add-task, set-override,
/// clear-override).
///
/// Contains the derived track status string so the CLI can print status without
/// calling `domain::derive_track_status` directly. The `task_id` field is
/// present for add-task; `None` for others.
#[derive(Debug)]
pub struct TaskOperationOutput {
    pub track_id: String,
    pub task_id: Option<String>,
    pub derived_status: String,
}

/// DTO returned by `TaskQueryService::next_task`.
///
/// Contains the `task_id` and `description` of the next open task so the CLI
/// can print them without importing domain task types.
#[derive(Debug)]
pub struct NextTaskOutput {
    pub task_id: String,
    pub description: String,
}

/// DTO returned by `TaskQueryService::task_counts`.
///
/// Contains per-status counts (todo, in_progress, done, skipped) so the CLI
/// can print counts without importing `domain::TaskStatusKind`.
pub struct TaskCountsOutput {
    pub todo: usize,
    pub in_progress: usize,
    pub done: usize,
    pub skipped: usize,
}

/// DTO carrying overall track status (used for `TaskOperationOutput.track_id`).
pub struct TrackStatusOutput {
    pub track_id: String,
    pub status: String,
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Error type for track task operation and query use cases.
///
/// Wraps `domain::DomainError`, `domain::TrackReadError`,
/// `domain::TrackWriteError`, `domain::RepositoryError`, and
/// `domain::WorktreeError` so that `cli/src/error.rs` can remove the five
/// `#[from]` domain error variants from `CliError`. The CLI converts
/// `TaskOperationError` to `CliError::Message`, eliminating the direct domain
/// error coupling at the CLI boundary.
#[derive(Debug, Error)]
pub enum TaskOperationError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("invalid task ID: {0}")]
    InvalidTaskId(String),
    #[error("invalid commit hash: {0}")]
    InvalidCommitHash(String),
    #[error("track not found: {0}")]
    TrackNotFound(String),
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("transition failed: {0}")]
    TransitionFailed(String),
    #[error("store failed: {0}")]
    StoreFailed(String),
    #[error("branch guard failed: {0}")]
    BranchGuardFailed(String),
    #[error("branchless guard failed: {0}")]
    BranchlessGuardFailed(String),
}

impl From<TrackWriteError> for TaskOperationError {
    fn from(e: TrackWriteError) -> Self {
        match e {
            TrackWriteError::Domain(de) => match de {
                DomainError::Transition(TransitionError::TaskNotFound { task_id }) => {
                    Self::TaskNotFound(task_id)
                }
                other => Self::TransitionFailed(other.to_string()),
            },
            TrackWriteError::Repository(re) => match re {
                RepositoryError::TrackNotFound(id) => Self::TrackNotFound(id),
                other => Self::StoreFailed(other.to_string()),
            },
        }
    }
}

impl From<TrackReadError> for TaskOperationError {
    fn from(e: TrackReadError) -> Self {
        match e {
            TrackReadError::Repository(re) => match re {
                RepositoryError::TrackNotFound(id) => Self::TrackNotFound(id),
                other => Self::StoreFailed(other.to_string()),
            },
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// CQRS command object for the task transition use case.
pub struct TaskTransitionCommand {
    pub items_dir: PathBuf,
    pub track_id: String,
    pub task_id: String,
    pub target_status: String,
    pub commit_hash: Option<String>,
}

/// CQRS command object for the add-task use case.
pub struct AddTaskCommand {
    pub items_dir: PathBuf,
    pub track_id: String,
    pub description: String,
    pub section: Option<String>,
    pub after_task_id: Option<String>,
}

/// CQRS command object for the set-override use case.
pub struct SetOverrideCommand {
    pub items_dir: PathBuf,
    pub track_id: String,
    pub status: String,
    pub reason: String,
}

/// CQRS command object for the clear-override use case.
pub struct ClearOverrideCommand {
    pub items_dir: PathBuf,
    pub track_id: String,
}

// ── TaskOperationService ──────────────────────────────────────────────────────

/// Application service trait for track task mutation operations.
///
/// Driven by the CLI. Accepts command objects that carry only string/primitive
/// fields, so the CLI never imports `domain::TrackId`, `domain::TaskId`,
/// `domain::CommitHash`, `domain::StatusOverride`, or `domain::DomainError`
/// directly.
pub trait TaskOperationService: Send + Sync {
    /// Transitions a task to the target status.
    ///
    /// A `todo -> in_progress` transition is judged first: a refusal comes back
    /// as [`TaskTransitionOutcome::Rejected`] with its figures intact, and no
    /// state is written.
    ///
    /// # Errors
    ///
    /// Returns [`TaskOperationError`] on ID validation, not-found, or transition
    /// failures.
    fn transition_task(
        &self,
        cmd: TaskTransitionCommand,
    ) -> Result<TaskTransitionOutcome, TaskOperationError>;

    /// Adds a new task to the track's impl-plan.
    ///
    /// # Errors
    ///
    /// Returns [`TaskOperationError`] on ID validation, not-found, or domain
    /// failures.
    fn add_task(&self, cmd: AddTaskCommand) -> Result<TaskOperationOutput, TaskOperationError>;

    /// Sets a status override on the track.
    ///
    /// # Errors
    ///
    /// Returns [`TaskOperationError`] on ID validation, not-found, or domain
    /// failures.
    fn set_override(
        &self,
        cmd: SetOverrideCommand,
    ) -> Result<TaskOperationOutput, TaskOperationError>;

    /// Clears any status override on the track.
    ///
    /// # Errors
    ///
    /// Returns [`TaskOperationError`] on ID validation, not-found, or domain
    /// failures.
    fn clear_override(
        &self,
        cmd: ClearOverrideCommand,
    ) -> Result<TaskOperationOutput, TaskOperationError>;
}

// ── Branch guard port type alias (removed) ───────────────────────────────────
// BranchReaderFn closure type replaced by BranchReaderPort from track_resolution
// (T003: IN-07, CN-05).

// ── TaskOperationInteractor ───────────────────────────────────────────────────

/// Concrete struct implementing [`TaskOperationService`].
///
/// Follows the same internal generic-storage pattern as the existing
/// `TransitionTaskUseCase`: the struct holds a private `Arc<S>` field where
/// `S` satisfies domain storage traits (`TrackReader + TrackWriter +
/// ImplPlanReader + ImplPlanWriter`) as an implementation detail.
///
/// CLI composition root wires `FsTrackStore` as `S` and injects the result as
/// `Arc<dyn TaskOperationService>`, so the generic bound never crosses the
/// usecase→CLI boundary; CLI commands only see the `dyn TaskOperationService`
/// trait object (CN-01 satisfied).
///
/// Branch guard enforcement is always active when a `branch_reader` port is
/// injected (supplied at construction time). Passing `None` for `branch_reader`
/// disables the guard entirely (e.g. in test environments without a git
/// repository). The injected port reads the current branch via the
/// [`BranchReaderPort`] secondary port; branch detection stays outside the
/// usecase layer (hexagonal boundary preserved, IN-07, CN-05).
pub struct TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    store: Arc<S>,
    branch_reader: Option<Arc<dyn BranchReaderPort>>,
    batch_plan_reader: Arc<dyn BatchPlanReaderPort>,
    scope_diff_measurer: Arc<dyn ScopeDiffMeasurePort>,
    scope_config_reader: Arc<dyn ScopeConfigReaderPort>,
    commit_record_verifier: Arc<dyn CommitRecordVerifierPort>,
}

impl<S> TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    /// Creates a new interactor with an optional injected [`BranchReaderPort`] port.
    ///
    /// The `branch_reader` port supplies the current git branch name without
    /// introducing an infrastructure dependency into the usecase layer.  When
    /// `branch_reader` is `Some`, the interactor calls
    /// `branch_reader.current_branch()` and compares the result with the track's
    /// expected branch, returning [`TaskOperationError::BranchGuardFailed`] or
    /// [`TaskOperationError::BranchlessGuardFailed`] on mismatch.
    ///
    /// Pass `None` to disable the branch guard entirely (e.g. in test
    /// environments without a git repository); the guard is a no-op in that case.
    ///
    /// The CLI composition root supplies `Arc<SystemGitRepo>` (wrapped in `Some`)
    /// as the real adapter; tests pass `None` (or a stub) to control guard
    /// behaviour.
    ///
    /// # Errors
    ///
    /// The branch reader port should return `Err(BranchReadError)` when the
    /// branch cannot be determined; the error is surfaced as
    /// [`TaskOperationError::BranchGuardFailed`].
    /// The three admission collaborators are required rather than optional, so
    /// no constructible interactor can transition a task without judging it
    /// first (`CN-10`). The commit-record verifier is required for the same
    /// reason: no constructible interactor can record a hash unverified
    /// (`IN-22`).
    #[must_use]
    pub fn new(
        store: Arc<S>,
        branch_reader: Option<Arc<dyn BranchReaderPort>>,
        batch_plan_reader: Arc<dyn BatchPlanReaderPort>,
        scope_diff_measurer: Arc<dyn ScopeDiffMeasurePort>,
        scope_config_reader: Arc<dyn ScopeConfigReaderPort>,
        commit_record_verifier: Arc<dyn CommitRecordVerifierPort>,
    ) -> Self {
        Self {
            store,
            branch_reader,
            batch_plan_reader,
            scope_diff_measurer,
            scope_config_reader,
            commit_record_verifier,
        }
    }

    /// The ports the admission judgement reads its inputs through.
    fn admission_ports(&self) -> AdmissionPorts<'_> {
        AdmissionPorts {
            batch_plan_reader: &self.batch_plan_reader,
            scope_diff_measurer: &self.scope_diff_measurer,
            scope_config_reader: &self.scope_config_reader,
        }
    }

    /// Judges every transition that enters `in_progress` — a start from `todo`
    /// and a reopen from done alike — returning the refusal when there is one
    /// (`IN-09`, `AC-26`, `AC-27`).
    ///
    /// Every other transition passes through untouched: the guard is about
    /// entering work, not about finishing or skipping it. Widening it to reopens
    /// closes the route by which an undeclared settled task could become
    /// unsettled unjudged; the judgement itself is unchanged.
    fn judge_entry_into_work(
        &self,
        cmd: &TaskTransitionCommand,
        track_id: &TrackId,
        task_id: &TaskId,
        commit_hash: Option<CommitHash>,
    ) -> Result<Option<AdmissionRejectionOutput>, TaskOperationError> {
        let plan = self
            .store
            .load_impl_plan(track_id)
            .map_err(|e| TaskOperationError::StoreFailed(e.to_string()))?;
        let Some(plan) = plan else { return Ok(None) };

        // Which transition the request names is the plan's to resolve. An
        // unresolvable one is left to the applier to report, so this guard neither
        // duplicates the transition table nor pre-empts its errors.
        let Ok(transition) = plan.transition_for(task_id, &cmd.target_status, commit_hash) else {
            return Ok(None);
        };
        if !crate::task_admission::enters_in_progress(&transition) {
            return Ok(None);
        }
        judge_admission(&self.admission_ports(), &cmd.items_dir, track_id, task_id, &plan)
    }
}

impl<S> TaskOperationService for TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    fn transition_task(
        &self,
        cmd: TaskTransitionCommand,
    ) -> Result<TaskTransitionOutcome, TaskOperationError> {
        let track_id = TrackId::try_new(&cmd.track_id)
            .map_err(|e| TaskOperationError::InvalidTrackId(e.to_string()))?;
        let task_id = TaskId::try_new(&cmd.task_id)
            .map_err(|e| TaskOperationError::InvalidTaskId(e.to_string()))?;
        let commit_hash = cmd
            .commit_hash
            .as_deref()
            .map(CommitHash::try_new)
            .transpose()
            .map_err(|e| TaskOperationError::InvalidCommitHash(e.to_string()))?;

        // Enforce branch guard before mutating state.
        enforce_branch_guard(&*self.store, &track_id, &cmd.items_dir, self.branch_reader.as_ref())?;

        // Judge the entry into work before anything is written: the guard sits on
        // the one transition path, so no workflow reaches `in_progress` around
        // it (CN-10).
        if let Some(rejection) =
            self.judge_entry_into_work(&cmd, &track_id, &task_id, commit_hash.clone())?
        {
            return Ok(TaskTransitionOutcome::Rejected(rejection));
        }

        // A hash is checked against the repository before the transition that
        // would record it is applied, so a refused hash leaves the plan exactly
        // as it was (IN-22, AC-28).
        commit_record::verify_recorded_hash(
            &*self.commit_record_verifier,
            &*self.store,
            &cmd,
            &track_id,
            &task_id,
            commit_hash.as_ref(),
        )?;

        let uc = crate::TransitionTaskUseCase::new(Arc::clone(&self.store));
        let track = uc
            .execute_by_status(&track_id, &task_id, &cmd.target_status, commit_hash)
            .map_err(TaskOperationError::from)?;

        // Derive status from impl-plan (loaded separately after the write).
        let impl_plan = self
            .store
            .load_impl_plan(&track_id)
            .map_err(|e| TaskOperationError::StoreFailed(e.to_string()))?;
        let status = derive_track_status(impl_plan.as_ref(), track.status_override()).to_string();

        Ok(TaskTransitionOutcome::Transitioned(TaskOperationOutput {
            track_id: track.id().as_ref().to_owned(),
            task_id: None,
            derived_status: status,
        }))
    }

    fn add_task(&self, cmd: AddTaskCommand) -> Result<TaskOperationOutput, TaskOperationError> {
        let track_id = TrackId::try_new(&cmd.track_id)
            .map_err(|e| TaskOperationError::InvalidTrackId(e.to_string()))?;
        let after_task_id = cmd
            .after_task_id
            .as_deref()
            .map(TaskId::try_new)
            .transpose()
            .map_err(|e| TaskOperationError::InvalidTaskId(e.to_string()))?;

        // Enforce branch guard before mutating state.
        enforce_branch_guard(&*self.store, &track_id, &cmd.items_dir, self.branch_reader.as_ref())?;

        let uc = crate::AddTaskUseCase::new(Arc::clone(&self.store));
        let (track, new_id) = uc
            .execute(&track_id, &cmd.description, cmd.section.as_deref(), after_task_id.as_ref())
            .map_err(TaskOperationError::from)?;

        let impl_plan = self
            .store
            .load_impl_plan(&track_id)
            .map_err(|e| TaskOperationError::StoreFailed(e.to_string()))?;
        let status = derive_track_status(impl_plan.as_ref(), track.status_override()).to_string();

        Ok(TaskOperationOutput {
            track_id: track.id().as_ref().to_owned(),
            task_id: Some(new_id.as_ref().to_owned()),
            derived_status: status,
        })
    }

    fn set_override(
        &self,
        cmd: SetOverrideCommand,
    ) -> Result<TaskOperationOutput, TaskOperationError> {
        let track_id = TrackId::try_new(&cmd.track_id)
            .map_err(|e| TaskOperationError::InvalidTrackId(e.to_string()))?;

        let status_override = match cmd.status.as_str() {
            "blocked" => StatusOverride::blocked(&cmd.reason)
                .map_err(|e| TaskOperationError::TransitionFailed(e.to_string()))?,
            "cancelled" => StatusOverride::cancelled(&cmd.reason)
                .map_err(|e| TaskOperationError::TransitionFailed(e.to_string()))?,
            other => {
                return Err(TaskOperationError::TransitionFailed(format!(
                    "unknown status override kind: '{other}' (expected 'blocked' or 'cancelled')"
                )));
            }
        };

        // Enforce branch guard before mutating state.
        enforce_branch_guard(&*self.store, &track_id, &cmd.items_dir, self.branch_reader.as_ref())?;

        let uc = crate::SetOverrideUseCase::new(Arc::clone(&self.store));
        let track =
            uc.execute(&track_id, Some(status_override)).map_err(TaskOperationError::from)?;

        // For set-override, the status override dominates: pass `None` for impl-plan
        // so `derive_track_status` returns the override-driven status (Blocked/Cancelled)
        // without reading impl-plan.json. This matches the existing CLI behavior in
        // `execute_set_override`, which also uses `derive_track_status(None, override)`.
        let status = derive_track_status(None, track.status_override()).to_string();

        Ok(TaskOperationOutput {
            track_id: track.id().as_ref().to_owned(),
            task_id: None,
            derived_status: status,
        })
    }

    fn clear_override(
        &self,
        cmd: ClearOverrideCommand,
    ) -> Result<TaskOperationOutput, TaskOperationError> {
        let track_id = TrackId::try_new(&cmd.track_id)
            .map_err(|e| TaskOperationError::InvalidTrackId(e.to_string()))?;

        // Enforce branch guard before mutating state.
        enforce_branch_guard(&*self.store, &track_id, &cmd.items_dir, self.branch_reader.as_ref())?;

        let uc = crate::SetOverrideUseCase::new(Arc::clone(&self.store));
        let track = uc.execute(&track_id, None).map_err(TaskOperationError::from)?;

        let impl_plan = self
            .store
            .load_impl_plan(&track_id)
            .map_err(|e| TaskOperationError::StoreFailed(e.to_string()))?;
        let status = derive_track_status(impl_plan.as_ref(), track.status_override()).to_string();

        Ok(TaskOperationOutput {
            track_id: track.id().as_ref().to_owned(),
            task_id: None,
            derived_status: status,
        })
    }
}

// ── TaskQueryService ──────────────────────────────────────────────────────────

/// Application service trait for next-task and task-counts queries.
///
/// Driven by the CLI. Accepts string `track_id` and `items_dir` so the CLI
/// never imports `domain::TrackId`, `domain::ImplPlanReader`, or
/// `domain::TaskStatusKind` directly. Returns simple serializable DTOs that
/// the CLI formats for stdout.
pub trait TaskQueryService: Send + Sync {
    /// Returns the next open task for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`TaskOperationError`] on ID validation or store failures.
    fn next_task(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<Option<NextTaskOutput>, TaskOperationError>;

    /// Returns per-status task counts for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`TaskOperationError`] on ID validation or store failures.
    fn task_counts(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<TaskCountsOutput, TaskOperationError>;
}

// ── TaskQueryInteractor ───────────────────────────────────────────────────────

/// Concrete struct implementing [`TaskQueryService`].
///
/// Constructs domain types internally and converts results to
/// [`NextTaskOutput`] or [`TaskCountsOutput`] before returning to the CLI.
pub struct TaskQueryInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    store: Arc<S>,
}

impl<S> TaskQueryInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    /// Creates a new interactor bound to the given store.
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }
}

impl<S> TaskQueryService for TaskQueryInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    fn next_task(
        &self,
        track_id: String,
        _items_dir: PathBuf,
    ) -> Result<Option<NextTaskOutput>, TaskOperationError> {
        let id = TrackId::try_new(&track_id)
            .map_err(|e| TaskOperationError::InvalidTrackId(e.to_string()))?;

        // Verify track exists.
        self.store
            .find(&id)
            .map_err(TaskOperationError::from)?
            .ok_or_else(|| TaskOperationError::TrackNotFound(track_id.clone()))?;

        let impl_plan = self
            .store
            .load_impl_plan(&id)
            .map_err(|e| TaskOperationError::StoreFailed(e.to_string()))?;

        let Some(plan) = impl_plan else {
            return Ok(None);
        };

        Ok(plan.next_open_task().map(|t| NextTaskOutput {
            task_id: t.id().as_ref().to_owned(),
            description: t.description().to_owned(),
        }))
    }

    fn task_counts(
        &self,
        track_id: String,
        _items_dir: PathBuf,
    ) -> Result<TaskCountsOutput, TaskOperationError> {
        use domain::TaskStatusKind;

        let id = TrackId::try_new(&track_id)
            .map_err(|e| TaskOperationError::InvalidTrackId(e.to_string()))?;

        // Verify track exists.
        self.store
            .find(&id)
            .map_err(TaskOperationError::from)?
            .ok_or_else(|| TaskOperationError::TrackNotFound(track_id.clone()))?;

        let impl_plan = self
            .store
            .load_impl_plan(&id)
            .map_err(|e| TaskOperationError::StoreFailed(e.to_string()))?;

        let Some(plan) = impl_plan else {
            return Ok(TaskCountsOutput { todo: 0, in_progress: 0, done: 0, skipped: 0 });
        };

        let mut todo = 0usize;
        let mut in_progress = 0usize;
        let mut done = 0usize;
        let mut skipped = 0usize;

        for task in plan.tasks() {
            match task.status().kind() {
                TaskStatusKind::Todo => todo += 1,
                TaskStatusKind::InProgress => in_progress += 1,
                TaskStatusKind::Done => done += 1,
                TaskStatusKind::Skipped => skipped += 1,
            }
        }

        Ok(TaskCountsOutput { todo, in_progress, done, skipped })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use domain::{
        DomainError, ImplPlanDocument, ImplPlanReader, ImplPlanWriter, PlanSection, PlanView,
        RepositoryError, TaskId, TrackId, TrackMetadata, TrackReadError, TrackReader,
        TrackWriteError, TrackWriter,
    };

    use crate::track_lifecycle::TrackItemsDirectory;
    use crate::track_lifecycle::TrackTaskTransition;
    use crate::track_resolution::{BranchReadError, BranchReaderPort};

    use super::*;

    /// Stub BranchReaderPort that returns a fixed branch or error.
    struct StubBranchReader {
        value: Result<Option<String>, String>,
    }

    impl StubBranchReader {
        /// Returns `Some(Arc<StubBranchReader>)` that yields the given branch name.
        fn ok(branch: impl Into<String>) -> Option<Arc<dyn BranchReaderPort>> {
            Some(Arc::new(Self { value: Ok(Some(branch.into())) }))
        }

        /// Returns `Some(Arc<StubBranchReader>)` that yields a `ReadFailed` error.
        fn err(msg: impl Into<String>) -> Option<Arc<dyn BranchReaderPort>> {
            Some(Arc::new(Self { value: Err(msg.into()) }))
        }
    }

    impl BranchReaderPort for StubBranchReader {
        fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
            match &self.value {
                Ok(v) => Ok(v.clone()),
                Err(msg) => Err(BranchReadError::ReadFailed(msg.clone())),
            }
        }
    }

    /// Batch plan stub: `None` is the shape of a track authored before the gate
    /// existed, which the reader reports as `NotFound`.
    struct StubBatchPlanReader(Option<domain::batch_plan::BatchPlanDocument>);

    impl BatchPlanReaderPort for StubBatchPlanReader {
        fn read(
            &self,
            _items_dir: &std::path::Path,
            _track_id: &TrackId,
        ) -> Result<domain::batch_plan::BatchPlanDocument, crate::batch_plan::PlanArtifactReadError>
        {
            self.0.clone().ok_or(crate::batch_plan::PlanArtifactReadError::NotFound)
        }
    }

    struct StubScopeDiffMeasurer(Vec<domain::batch_plan::MeasuredScopeDiff>);

    impl ScopeDiffMeasurePort for StubScopeDiffMeasurer {
        fn measure_scope_diff(
            &self,
            _items_dir: &std::path::Path,
            _track_id: &TrackId,
        ) -> Result<
            Vec<domain::batch_plan::MeasuredScopeDiff>,
            crate::batch_plan::ScopeDiffMeasureError,
        > {
            Ok(self.0.clone())
        }
    }

    /// Scope configuration stub carrying one ceiling per named scope.
    struct StubScopeConfigReader(Vec<(&'static str, Option<u32>)>);

    impl ScopeConfigReaderPort for StubScopeConfigReader {
        fn read(
            &self,
            _items_dir: &std::path::Path,
            track_id: &TrackId,
        ) -> Result<domain::review_v2::ReviewScopeConfig, crate::batch_plan::ScopeConfigReadError>
        {
            let groups = self
                .0
                .iter()
                .map(|(scope, ceiling)| {
                    ((*scope).to_owned(), vec![format!("libs/{scope}/**")], None, *ceiling)
                })
                .collect();
            domain::review_v2::ReviewScopeConfig::new(
                track_id,
                groups,
                Vec::new(),
                Vec::new(),
                None,
            )
            .map_err(|error| crate::batch_plan::ScopeConfigReadError::ReadFailed {
                message: domain::FreeText::new(error.to_string()),
            })
        }
    }

    /// Commit-record verifier stub yielding one fixed verdict.
    ///
    /// [`StubVerifier::accepting`] is what every test that is not about record
    /// verification wires: those tests are about another guard, and an accepted
    /// hash leaves the transition they observe unchanged.
    struct StubVerifier(Option<Box<dyn Fn() -> CommitRecordVerifyError + Send + Sync>>);

    impl StubVerifier {
        fn accepting() -> Arc<dyn CommitRecordVerifierPort> {
            Arc::new(Self(None))
        }

        fn refusing(
            error: impl Fn() -> CommitRecordVerifyError + Send + Sync + 'static,
        ) -> Arc<dyn CommitRecordVerifierPort> {
            Arc::new(Self(Some(Box::new(error))))
        }
    }

    impl CommitRecordVerifierPort for StubVerifier {
        fn verify_commit_record(
            &self,
            _items_dir: &std::path::Path,
            _commit_hash: &CommitHash,
        ) -> Result<(), CommitRecordVerifyError> {
            match &self.0 {
                None => Ok(()),
                Some(error) => Err(error()),
            }
        }
    }

    /// The interactor the guard-focused tests use: a batch plan that admits
    /// `T001`, so those tests observe the branch guard rather than admission.
    fn task_ops_interactor(
        store: Arc<StubStore>,
        branch_reader: Option<Arc<dyn BranchReaderPort>>,
    ) -> TaskOperationInteractor<StubStore> {
        let batch_plan = domain::batch_plan::BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5)],
            vec![batch("B1", &["T001"])],
        )
        .unwrap();
        TaskOperationInteractor::new(
            store,
            branch_reader,
            Arc::new(StubBatchPlanReader(Some(batch_plan))),
            Arc::new(StubScopeDiffMeasurer(Vec::new())),
            Arc::new(StubScopeConfigReader(vec![("domain", Some(500))])),
            StubVerifier::accepting(),
        )
    }

    fn assert_task_operation_ports<T>()
    where
        T: crate::track_lifecycle::TrackTaskTransitionPort
            + crate::track_lifecycle::TrackTaskAddPort
            + crate::track_lifecycle::TrackOverrideSetPort
            + crate::track_lifecycle::TrackOverrideClearPort,
    {
    }

    fn assert_task_query_ports<T>()
    where
        T: crate::track_lifecycle::TrackNextTaskQueryPort
            + crate::track_lifecycle::TrackTaskCountsQueryPort,
    {
    }

    #[test]
    fn test_task_interactors_implement_track_lifecycle_ports() {
        assert_task_operation_ports::<TaskOperationInteractor<StubStore>>();
        assert_task_query_ports::<TaskQueryInteractor<StubStore>>();
    }

    fn lifecycle_items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("track/items")).unwrap()
    }

    #[test]
    fn test_track_lifecycle_operation_ports_execute_resolved_arguments() {
        use crate::track_lifecycle::{
            TrackOverrideClearPort, TrackOverrideSetPort, TrackTaskAddPort, TrackTaskTransitionPort,
        };
        use domain::{NonEmptyString, StatusOverrideKind, TaskStatus};

        let store = store_with(plan_of(&[("T001", TaskStatus::Todo)]));
        let interactor = task_ops_interactor(Arc::clone(&store), None);
        let track_id = TrackId::try_new("my-track-2026").unwrap();

        let transition =
            <TaskOperationInteractor<StubStore> as TrackTaskTransitionPort>::transition_task(
                &interactor,
                track_id.clone(),
                lifecycle_items_dir(),
                TaskId::try_new("T001").unwrap(),
                TrackTaskTransition::InProgress,
            )
            .unwrap();
        assert_eq!(transitioned(transition).derived_status, "in_progress");

        let added = <TaskOperationInteractor<StubStore> as TrackTaskAddPort>::add_task(
            &interactor,
            track_id.clone(),
            lifecycle_items_dir(),
            NonEmptyString::try_new("second task").unwrap(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(added.track_id, "my-track-2026");
        assert_eq!(added.task_id.as_deref(), Some("T002"));

        let overridden =
            <TaskOperationInteractor<StubStore> as TrackOverrideSetPort>::set_override(
                &interactor,
                track_id.clone(),
                lifecycle_items_dir(),
                StatusOverrideKind::Blocked,
                crate::git_workflow::DiagnosticText::new("waiting on a decision"),
            )
            .unwrap();
        assert_eq!(overridden.derived_status, "blocked");

        let cleared =
            <TaskOperationInteractor<StubStore> as TrackOverrideClearPort>::clear_override(
                &interactor,
                track_id,
                lifecycle_items_dir(),
            )
            .unwrap();
        assert_eq!(cleared.derived_status, "in_progress");
        assert_eq!(stored_status(&store, "T001"), TaskStatus::InProgress);
    }

    #[test]
    fn test_track_lifecycle_query_ports_use_resolved_track_id() {
        use crate::track_lifecycle::{TrackNextTaskQueryPort, TrackTaskCountsQueryPort};
        use domain::TaskStatus;

        let store =
            store_with(plan_of(&[("T001", TaskStatus::Todo), ("T002", TaskStatus::DonePending)]));
        let interactor = TaskQueryInteractor::new(Arc::clone(&store));

        let next = <TaskQueryInteractor<StubStore> as TrackNextTaskQueryPort>::next_task(
            &interactor,
            TrackId::try_new("my-track-2026").unwrap(),
            lifecycle_items_dir(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(next.task_id, "T001");
        assert_eq!(next.description, "task T001");

        let counts = <TaskQueryInteractor<StubStore> as TrackTaskCountsQueryPort>::task_counts(
            &interactor,
            TrackId::try_new("my-track-2026").unwrap(),
            lifecycle_items_dir(),
        )
        .unwrap();
        assert_eq!(counts.todo, 1);
        assert_eq!(counts.done, 1);
        assert_eq!(counts.in_progress, 0);
        assert_eq!(counts.skipped, 0);
    }

    /// The output of a transition that must have happened. A refusal is carried
    /// into the derived status instead, so the assertion that follows fails with
    /// the reason in its message.
    fn transitioned(outcome: TaskTransitionOutcome) -> TaskOperationOutput {
        match outcome {
            TaskTransitionOutcome::Transitioned(output) => output,
            TaskTransitionOutcome::Rejected(rejection) => TaskOperationOutput {
                track_id: format!("<refused: {rejection}>"),
                task_id: None,
                derived_status: format!("<refused: {rejection}>"),
            },
        }
    }

    #[derive(Default)]
    struct StubStore {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
        impl_plans: Mutex<HashMap<TrackId, ImplPlanDocument>>,
    }

    impl TrackReader for StubStore {
        fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
            Ok(self.tracks.lock().unwrap().get(id).cloned())
        }
    }

    impl TrackWriter for StubStore {
        fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
            self.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
            Ok(())
        }

        fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
        where
            F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>,
        {
            let mut tracks = self.tracks.lock().unwrap();
            let track = tracks.get_mut(id).ok_or_else(|| {
                TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
            })?;
            mutate(track).map_err(TrackWriteError::from)?;
            Ok(track.clone())
        }
    }

    impl ImplPlanReader for StubStore {
        fn load_impl_plan(
            &self,
            id: &TrackId,
        ) -> Result<Option<ImplPlanDocument>, RepositoryError> {
            Ok(self.impl_plans.lock().unwrap().get(id).cloned())
        }
    }

    impl ImplPlanWriter for StubStore {
        fn save_impl_plan(
            &self,
            id: &TrackId,
            doc: &ImplPlanDocument,
        ) -> Result<(), RepositoryError> {
            self.impl_plans.lock().unwrap().insert(id.clone(), doc.clone());
            Ok(())
        }
    }

    fn test_snapshot() -> domain::branch_strategy::BranchStrategySnapshot {
        domain::branch_strategy::BranchStrategySnapshot::new(
            domain::NonEmptyString::try_new("main").unwrap(),
            domain::NonEmptyString::try_new("main").unwrap(),
            domain::branch_strategy::MergeMethod::Squash,
        )
    }

    fn sample_track() -> TrackMetadata {
        TrackMetadata::new(
            TrackId::try_new("my-track-2026").unwrap(),
            "My Track",
            None,
            test_snapshot(),
        )
        .unwrap()
    }

    fn sample_plan() -> ImplPlanDocument {
        use domain::TrackTask;
        let task = TrackTask::new(TaskId::try_new("T001").unwrap(), "first task").unwrap();
        let section =
            PlanSection::new("S1", "Section", vec![], vec![TaskId::try_new("T001").unwrap()])
                .unwrap();
        ImplPlanDocument::new(vec![task], PlanView::new(vec![], vec![section])).unwrap()
    }

    #[test]
    fn task_query_next_task_returns_next_todo_task() {
        let store = Arc::new(StubStore::default());
        let track = sample_track();
        let plan = sample_plan();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        store.impl_plans.lock().unwrap().insert(track.id().clone(), plan);

        let interactor = TaskQueryInteractor::new(Arc::clone(&store));
        let result = interactor.next_task("my-track-2026".to_owned(), PathBuf::new()).unwrap();
        assert!(result.is_some());
        let out = result.unwrap();
        assert_eq!(out.task_id, "T001");
        assert_eq!(out.description, "first task");
    }

    #[test]
    fn task_query_next_task_returns_none_when_no_plan() {
        let store = Arc::new(StubStore::default());
        let track = sample_track();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());

        let interactor = TaskQueryInteractor::new(Arc::clone(&store));
        let result = interactor.next_task("my-track-2026".to_owned(), PathBuf::new()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn task_query_task_counts_returns_correct_counts() {
        use domain::{TaskStatus, TrackTask};
        let store = Arc::new(StubStore::default());
        let track = sample_track();

        let tasks = vec![
            TrackTask::with_status(
                TaskId::try_new("T001").unwrap(),
                "done task",
                TaskStatus::DonePending,
            )
            .unwrap(),
            TrackTask::new(TaskId::try_new("T002").unwrap(), "todo task").unwrap(),
        ];
        let section = PlanSection::new(
            "S1",
            "Section",
            vec![],
            vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
        )
        .unwrap();
        let plan = ImplPlanDocument::new(tasks, PlanView::new(vec![], vec![section])).unwrap();

        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        store.impl_plans.lock().unwrap().insert(track.id().clone(), plan);

        let interactor = TaskQueryInteractor::new(Arc::clone(&store));
        let counts = interactor.task_counts("my-track-2026".to_owned(), PathBuf::new()).unwrap();
        assert_eq!(counts.todo, 1);
        assert_eq!(counts.in_progress, 0);
        assert_eq!(counts.done, 1);
        assert_eq!(counts.skipped, 0);
    }

    #[test]
    fn task_query_invalid_track_id_returns_error() {
        let store = Arc::new(StubStore::default());
        let interactor = TaskQueryInteractor::new(Arc::clone(&store));
        let err = interactor.next_task("".to_owned(), PathBuf::new()).unwrap_err();
        assert!(matches!(err, TaskOperationError::InvalidTrackId(_)));
    }

    #[test]
    fn task_operation_transition_task_succeeds() {
        // Schema v5 with a materialized track (has branch): branchless guard passes
        // regardless of schema version when the track has an explicit branch.
        // The injected branch reader returns the expected branch so the guard passes.
        use domain::TrackBranch;
        let store = Arc::new(StubStore::default());
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track-2026").unwrap(),
            Some(TrackBranch::try_new("track/my-track-2026").unwrap()),
            "My Track",
            None,
            test_snapshot(),
        )
        .unwrap();
        let plan = sample_plan();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        store.impl_plans.lock().unwrap().insert(track.id().clone(), plan);

        // Inject a stub that returns the expected branch — guard passes.
        let interactor =
            task_ops_interactor(Arc::clone(&store), StubBranchReader::ok("track/my-track-2026"));
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let out = transitioned(interactor.transition_task(cmd).unwrap());
        assert_eq!(out.track_id, "my-track-2026");
        assert!(out.task_id.is_none());
        assert_eq!(out.derived_status, "in_progress");
    }

    #[test]
    fn task_operation_branch_guard_enforced_via_injected_reader() {
        // Verify that new() wires the injected branch reader and enforces the guard.
        use domain::TrackBranch;
        let store = Arc::new(StubStore::default());
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track-2026").unwrap(),
            Some(TrackBranch::try_new("track/my-track-2026").unwrap()),
            "My Track",
            None,
            test_snapshot(),
        )
        .unwrap();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        // No impl-plan: branch guard fires before the domain call.

        // Inject a reader that returns a mismatched branch.
        let interactor = task_ops_interactor(Arc::clone(&store), StubBranchReader::ok("main"));
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let err = interactor.transition_task(cmd).unwrap_err();
        assert!(
            matches!(err, TaskOperationError::BranchGuardFailed(_)),
            "expected BranchGuardFailed, got: {err}"
        );
    }

    #[test]
    fn task_operation_new_detached_head_returns_branchless_guard_failed() {
        // Verify the production constructor new() wires the reader so that
        // detached HEAD (reader returns "HEAD") is rejected as BranchlessGuardFailed.
        use domain::TrackBranch;
        let store = Arc::new(StubStore::default());
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track-2026").unwrap(),
            Some(TrackBranch::try_new("track/my-track-2026").unwrap()),
            "My Track",
            None,
            test_snapshot(),
        )
        .unwrap();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        // No impl-plan: branch guard fires before the domain call.

        let interactor = task_ops_interactor(Arc::clone(&store), StubBranchReader::ok("HEAD"));
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let err = interactor.transition_task(cmd).unwrap_err();
        assert!(
            matches!(err, TaskOperationError::BranchlessGuardFailed(_)),
            "expected BranchlessGuardFailed for detached HEAD, got: {err}"
        );
    }

    #[test]
    fn task_operation_error_invalid_track_id_returns_error() {
        let store = Arc::new(StubStore::default());
        // Branch reader is never called: track_id validation fires first.
        let interactor = task_ops_interactor(Arc::clone(&store), StubBranchReader::ok("any"));
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: String::new(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let err = interactor.transition_task(cmd).unwrap_err();
        assert!(matches!(err, TaskOperationError::InvalidTrackId(_)));
    }

    #[test]
    fn task_operation_transition_with_none_branch_reader_bypasses_guard() {
        // When branch_reader is None, the guard is a no-op (replaces the old
        // skip_branch_check = true test).
        use domain::TrackBranch;
        let store = Arc::new(StubStore::default());
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track-2026").unwrap(),
            Some(TrackBranch::try_new("track/my-track-2026").unwrap()),
            "My Track",
            None,
            test_snapshot(),
        )
        .unwrap();
        let plan = sample_plan();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        store.impl_plans.lock().unwrap().insert(track.id().clone(), plan);

        // No branch reader — guard is a no-op regardless of the track's branch.
        let interactor = task_ops_interactor(Arc::clone(&store), None);
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let out = transitioned(interactor.transition_task(cmd).unwrap());
        assert_eq!(out.track_id, "my-track-2026");
    }

    #[test]
    fn task_operation_transition_branchless_track_skips_branch_guard() {
        // A branchless track (branch = None) always passes the guard.
        let store = Arc::new(StubStore::default());
        let track = sample_track(); // branchless by construction
        let plan = sample_plan();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        store.impl_plans.lock().unwrap().insert(track.id().clone(), plan);

        // Inject a branch reader that would fail — it must not be called for
        // branchless tracks.
        let branch_reader =
            StubBranchReader::err("branch reader must not be called for branchless tracks");
        let interactor = task_ops_interactor(Arc::clone(&store), branch_reader);
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let out = transitioned(interactor.transition_task(cmd).unwrap());
        assert_eq!(out.track_id, "my-track-2026");
    }

    #[test]
    fn task_operation_branch_guard_rejects_wrong_branch() {
        // A track with a branch; the injected reader returns a mismatched branch.
        use domain::TrackBranch;
        let store = Arc::new(StubStore::default());
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track-2026").unwrap(),
            Some(TrackBranch::try_new("track/my-track-2026").unwrap()),
            "My Track",
            None,
            test_snapshot(),
        )
        .unwrap();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        // Note: no impl-plan needed since branch guard fires before domain calls.

        // Inject a reader that returns "main" — mismatches "track/my-track-2026".
        let branch_reader = StubBranchReader::ok("main");
        let interactor = task_ops_interactor(Arc::clone(&store), branch_reader);
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let err = interactor.transition_task(cmd).unwrap_err();
        assert!(
            matches!(err, TaskOperationError::BranchGuardFailed(_)),
            "expected BranchGuardFailed, got: {err}"
        );
    }

    #[test]
    fn task_operation_branch_guard_rejects_detached_head() {
        // A track with a branch; the injected reader returns "HEAD" (detached).
        use domain::TrackBranch;
        let store = Arc::new(StubStore::default());
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track-2026").unwrap(),
            Some(TrackBranch::try_new("track/my-track-2026").unwrap()),
            "My Track",
            None,
            test_snapshot(),
        )
        .unwrap();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());

        let branch_reader = StubBranchReader::ok("HEAD");
        let interactor = task_ops_interactor(Arc::clone(&store), branch_reader);
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let err = interactor.transition_task(cmd).unwrap_err();
        assert!(
            matches!(err, TaskOperationError::BranchlessGuardFailed(_)),
            "expected BranchlessGuardFailed, got: {err}"
        );
    }

    // ── admission on todo -> in_progress ──────────────────────────────────────

    /// An implementation plan holding the named tasks in the given states.
    fn plan_of(tasks: &[(&str, domain::TaskStatus)]) -> ImplPlanDocument {
        use domain::{NonEmptyString, TrackTask};
        let ids: Vec<TaskId> = tasks.iter().map(|(id, _)| TaskId::try_new(*id).unwrap()).collect();
        let tasks: Vec<TrackTask> = tasks
            .iter()
            .map(|(id, status)| {
                TrackTask::with_dependencies(
                    TaskId::try_new(*id).unwrap(),
                    NonEmptyString::try_new(format!("task {id}")).unwrap(),
                    status.clone(),
                    Vec::new(),
                )
            })
            .collect();
        let section = PlanSection::new("S1", "Section", vec![], ids).unwrap();
        ImplPlanDocument::new(tasks, PlanView::new(vec![], vec![section])).unwrap()
    }

    /// One task's estimate for the `domain` scope, split as production + test.
    fn estimate(id: &str, production: u32, test: u32) -> domain::batch_plan::TaskEstimate {
        estimate_in(main_scope("domain"), id, production, test)
    }

    /// The same estimate against any scope, so a test can declare one the
    /// configuration does not hold or the reserved implicit scope.
    fn estimate_in(
        scope: domain::review_v2::ScopeName,
        id: &str,
        production: u32,
        test: u32,
    ) -> domain::batch_plan::TaskEstimate {
        use domain::batch_plan::{LineCount, ScopeLineEstimate, TaskDecomposition, TaskEstimate};
        TaskEstimate::new(
            TaskId::try_new(id).unwrap(),
            vec![ScopeLineEstimate::new(scope, LineCount::new(production), LineCount::new(test))],
            TaskDecomposition::Decomposable,
        )
        .unwrap()
    }

    fn main_scope(name: &str) -> domain::review_v2::ScopeName {
        domain::review_v2::ScopeName::Main(domain::review_v2::MainScopeName::new(name).unwrap())
    }

    /// A measured diff of `lines` on the `domain` scope.
    fn measured_domain(lines: u32) -> Vec<domain::batch_plan::MeasuredScopeDiff> {
        use domain::batch_plan::{LineCount, MeasuredScopeDiff};
        vec![MeasuredScopeDiff::new(main_scope("domain"), LineCount::new(lines))]
    }

    fn batch(id: &str, members: &[&str]) -> domain::batch_plan::BatchDeclaration {
        use domain::batch_plan::{BatchDeclaration, BatchId};
        BatchDeclaration::new(
            BatchId::try_new(id).unwrap(),
            members.iter().map(|member| TaskId::try_new(*member).unwrap()).collect(),
        )
        .unwrap()
    }

    /// The interactor with a declared batch plan, a measured diff and one
    /// ceiling for the `domain` scope.
    fn admitting_interactor(
        store: Arc<StubStore>,
        batch_plan: domain::batch_plan::BatchPlanDocument,
        measured: Vec<domain::batch_plan::MeasuredScopeDiff>,
        ceiling: Option<u32>,
    ) -> TaskOperationInteractor<StubStore> {
        TaskOperationInteractor::new(
            store,
            None,
            Arc::new(StubBatchPlanReader(Some(batch_plan))),
            Arc::new(StubScopeDiffMeasurer(measured)),
            Arc::new(StubScopeConfigReader(vec![("domain", ceiling)])),
            StubVerifier::accepting(),
        )
    }

    fn start_work_on(task_id: &str) -> TaskTransitionCommand {
        TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: task_id.to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        }
    }

    fn commit_hash() -> CommitHash {
        CommitHash::try_new("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678").unwrap()
    }

    fn store_with(plan: ImplPlanDocument) -> Arc<StubStore> {
        let store = Arc::new(StubStore::default());
        let track = sample_track();
        store.tracks.lock().unwrap().insert(track.id().clone(), track);
        store.impl_plans.lock().unwrap().insert(TrackId::try_new("my-track-2026").unwrap(), plan);
        store
    }

    /// The same command as a start: entering `in_progress` from done is a reopen,
    /// and both are judged by the one guard.
    fn reopen(task_id: &str) -> TaskTransitionCommand {
        start_work_on(task_id)
    }

    /// The status the store currently holds for `task_id`.
    fn stored_status(store: &StubStore, task_id: &str) -> domain::TaskStatus {
        let plans = store.impl_plans.lock().unwrap();
        let plan = plans.get(&TrackId::try_new("my-track-2026").unwrap()).unwrap();
        let wanted = TaskId::try_new(task_id).unwrap();
        plan.tasks().iter().find(|task| *task.id() == wanted).unwrap().status().clone()
    }

    /// B1 holds the settled T001 — declared with `t001` as its estimate — beside
    /// the todo T002, so B1 still has an uncommitted member and is the batch being
    /// consumed: T001's reopen is a current-batch candidate and reaches the
    /// measured guard rather than stopping at membership.
    fn reopen_fixture(
        t001: domain::batch_plan::TaskEstimate,
    ) -> (Arc<StubStore>, domain::batch_plan::BatchPlanDocument) {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![t001, estimate("T002", 10, 5)],
            vec![batch("B1", &["T001", "T002"])],
        )
        .unwrap();
        let store = store_with(plan_of(&[
            ("T001", TaskStatus::DoneTraced { commit_hash: commit_hash() }),
            ("T002", TaskStatus::Todo),
        ]));
        (store, batch_plan)
    }

    #[test]
    fn test_reopening_a_candidate_whose_estimate_would_pass_the_ceiling_is_refused() {
        // 400 measured domain lines plus the reopened task's declared 200 would
        // reach 600 against a ceiling of 500: the cumulative rejection applies to a
        // reopen exactly as it does to a start.
        let (store, batch_plan) = reopen_fixture(estimate("T001", 150, 50));

        let outcome = admitting_interactor(store, batch_plan, measured_domain(400), Some(500))
            .transition_task(reopen("T001"))
            .unwrap();

        assert_eq!(
            outcome.rejection(),
            Some(&AdmissionRejectionOutput::ScopeCeilingWouldBeExceeded {
                scope: "domain".to_owned(),
                prior_contribution: std::num::NonZeroU32::new(400).unwrap(),
                candidate_estimate: 200,
                ceiling: 500,
            })
        );
    }

    #[test]
    fn test_reopening_a_candidate_whose_estimate_names_an_unconfigured_scope_is_an_error() {
        // The same fail-closed error a start gets: an unresolvable ceiling must not
        // admit the reopen on the grounds that nothing constrains it.
        let (store, batch_plan) = reopen_fixture(estimate_in(main_scope("domian"), "T001", 10, 5));

        let error = admitting_interactor(store, batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap_err();

        assert!(matches!(error, TaskOperationError::TransitionFailed(_)), "unexpected: {error}");
        assert!(error.to_string().contains("T001"), "the failure names the task: {error}");
        assert!(error.to_string().contains("domian"), "the failure names the scope: {error}");
    }

    #[test]
    fn test_reopening_the_first_contributor_to_a_scope_is_admitted_however_large_its_estimate() {
        // Nothing measured and no member in progress, so the scope carries nothing:
        // the first contribution gets in on a reopen too, and the guard cannot
        // deadlock a reopen it would have to refuse forever.
        let (store, batch_plan) = reopen_fixture(estimate("T001", 700, 300));

        let outcome = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap();

        assert!(matches!(outcome, TaskTransitionOutcome::Transitioned(_)));
        assert_eq!(stored_status(&store, "T001"), domain::TaskStatus::InProgress);
    }

    #[test]
    fn test_reopening_a_candidate_that_declares_only_the_reserved_other_scope_is_admitted() {
        // `other` resolves no ceiling, so its total is never compared — on a reopen
        // as on a start, and even with the scope already carrying more than the
        // configured limit for a named scope.
        let (store, batch_plan) =
            reopen_fixture(estimate_in(domain::review_v2::ScopeName::Other, "T001", 900, 200));

        let outcome = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap();

        assert!(matches!(outcome, TaskTransitionOutcome::Transitioned(_)));
        assert_eq!(stored_status(&store, "T001"), domain::TaskStatus::InProgress);
    }

    #[test]
    fn test_reopening_a_member_of_a_fully_settled_declaration_is_refused_for_want_of_a_batch() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // The declaration holds one batch and its only member is delivered, so no
        // batch is being consumed. The reopen is refused rather than admitted: the
        // declaration has to be updated before the work can be picked up again.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5)],
            vec![batch("B1", &["T001"])],
        )
        .unwrap();
        let store =
            store_with(plan_of(&[("T001", TaskStatus::DoneTraced { commit_hash: commit_hash() })]));

        let outcome = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap();

        assert_eq!(
            outcome.rejection(),
            Some(&AdmissionRejectionOutput::NoCurrentBatch {
                task_id: "T001".to_owned(),
                task_batch: "B1".to_owned(),
            })
        );
        let rendered = outcome.rejection().unwrap().to_string();
        assert!(rendered.contains("T001") && rendered.contains("B1"), "rendered as: {rendered}");
        assert!(rendered.contains("settled"), "rendered as: {rendered}");
        // Refused before any write: the task is still delivered.
        assert!(matches!(stored_status(&store, "T001"), TaskStatus::DoneTraced { .. }));
    }

    #[test]
    fn test_reopening_a_candidate_with_no_declared_estimate_is_the_missing_estimate_error() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // The missing-estimate failure on a reopen, isolated from the membership
        // one. B1 = [T002] is a well-formed current batch, so membership is
        // decidable here; the settled T001 is simply not declared, and the
        // evaluation asks for the candidate's estimate before it asks about
        // membership, so the missing-estimate error is what fires.
        //
        // The neighbouring state — a *declared* member carrying no estimate — is
        // unrepresentable rather than untested: `BatchPlanDocument::new` refuses to
        // build such a plan, as
        // `domain::batch_plan::tests::test_the_plan_rejects_a_batch_member_without_a_declared_estimate`
        // and, for a settled member,
        // `test_a_declared_settled_task_is_not_exempt_from_the_file_internal_invariants`
        // establish. An undeclared candidate is therefore the only input that
        // reaches this error at all.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T002", 10, 5)],
            vec![batch("B1", &["T002"])],
        )
        .unwrap();
        let store = store_with(plan_of(&[
            ("T001", TaskStatus::DoneTraced { commit_hash: commit_hash() }),
            ("T002", TaskStatus::Todo),
        ]));

        let error = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap_err();

        let rendered = error.to_string();
        assert!(matches!(error, TaskOperationError::TransitionFailed(_)), "unexpected: {error}");
        assert!(rendered.contains("no estimate in the batch plan"), "unexpected: {rendered}");
        assert!(rendered.contains("T001"), "the failure names the candidate: {rendered}");
        // Specifically not the membership refusal, which renders the batch names.
        assert!(!rendered.contains("being consumed"), "not a membership refusal: {rendered}");
        assert!(matches!(stored_status(&store, "T001"), TaskStatus::DoneTraced { .. }));
    }

    #[test]
    fn test_reopening_a_settled_task_the_batch_plan_does_not_declare_is_refused_without_a_write() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // The plan declares only the remaining work, so the settled T001 has no
        // estimate and no batch. Reopening it would make it unsettled with neither,
        // which is exactly what the judged set is widened to prevent (AC-26).
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T002", 10, 5)],
            vec![batch("B2", &["T002"])],
        )
        .unwrap();
        let store = store_with(plan_of(&[
            ("T001", TaskStatus::DoneTraced { commit_hash: commit_hash() }),
            ("T002", TaskStatus::Todo),
        ]));

        let error = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap_err();

        assert!(matches!(error, TaskOperationError::TransitionFailed(_)), "unexpected: {error}");
        assert!(error.to_string().contains("T001"), "the failure names the task: {error}");
        // Fail-closed and before any write: the task is still settled.
        assert!(matches!(stored_status(&store, "T001"), TaskStatus::DoneTraced { .. }));
    }

    #[test]
    fn test_reopening_a_declared_member_of_the_current_batch_is_judged_like_a_start() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // B1 holds the settled T001 and the todo T002, so B1 still has an
        // uncommitted member and is the batch being consumed. T001 is declared, so
        // the same formula that admits a start admits its reopen (AC-27).
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5), estimate("T002", 10, 5)],
            vec![batch("B1", &["T001", "T002"])],
        )
        .unwrap();
        let store = store_with(plan_of(&[
            ("T001", TaskStatus::DoneTraced { commit_hash: commit_hash() }),
            ("T002", TaskStatus::Todo),
        ]));

        let outcome = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap();

        assert_eq!(transitioned(outcome).derived_status, "in_progress");
        assert_eq!(stored_status(&store, "T001"), TaskStatus::InProgress);
    }

    #[test]
    fn test_reopening_a_declared_member_of_a_closed_batch_is_refused_as_a_non_member() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // B1 is fully committed, so B2 is the batch being consumed and B1's member
        // is no longer part of it: the reopen needs the declaration updated first.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5), estimate("T002", 10, 5)],
            vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
        )
        .unwrap();
        let store = store_with(plan_of(&[
            ("T001", TaskStatus::DoneTraced { commit_hash: commit_hash() }),
            ("T002", TaskStatus::Todo),
        ]));

        let outcome = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(reopen("T001"))
            .unwrap();

        assert_eq!(
            outcome.rejection(),
            Some(&AdmissionRejectionOutput::NotCurrentBatchMember {
                task_id: "T001".to_owned(),
                task_batch: "B1".to_owned(),
                current_batch: "B2".to_owned(),
            })
        );
        assert!(matches!(stored_status(&store, "T001"), TaskStatus::DoneTraced { .. }));
    }

    #[test]
    fn test_widening_the_judged_set_leaves_starts_and_other_transitions_as_they_were() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // A declared todo member of the current batch still starts, unchanged.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5)],
            vec![batch("B1", &["T001"])],
        )
        .unwrap();
        let started = admitting_interactor(
            store_with(plan_of(&[("T001", TaskStatus::Todo)])),
            batch_plan.clone(),
            Vec::new(),
            Some(500),
        )
        .transition_task(start_work_on("T001"))
        .unwrap();
        assert_eq!(transitioned(started).derived_status, "in_progress");

        // A transition that does not enter `in_progress` is still not judged: the
        // undeclared T002 completes even though the plan holds no estimate for it.
        let store =
            store_with(plan_of(&[("T001", TaskStatus::Todo), ("T002", TaskStatus::InProgress)]));
        let finished = admitting_interactor(Arc::clone(&store), batch_plan, Vec::new(), Some(500))
            .transition_task(TaskTransitionCommand {
                items_dir: PathBuf::new(),
                track_id: "my-track-2026".to_owned(),
                task_id: "T002".to_owned(),
                target_status: "done".to_owned(),
                commit_hash: Some(commit_hash().as_ref().to_owned()),
            })
            .unwrap();

        assert!(matches!(finished, TaskTransitionOutcome::Transitioned(_)));
        assert!(matches!(stored_status(&store, "T002"), TaskStatus::DoneTraced { .. }));

        // Nor is a reset judged, though it too is now resolved before the guard
        // runs: what decides is where the resolved transition leads, not the target
        // string, and only `in_progress` is entered.
        let store =
            store_with(plan_of(&[("T001", TaskStatus::Todo), ("T002", TaskStatus::InProgress)]));
        let reset = admitting_interactor(
            Arc::clone(&store),
            BatchPlanDocument::new(
                TrackId::try_new("my-track-2026").unwrap(),
                vec![estimate("T001", 10, 5)],
                vec![batch("B1", &["T001"])],
            )
            .unwrap(),
            Vec::new(),
            Some(500),
        )
        .transition_task(TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T002".to_owned(),
            target_status: "todo".to_owned(),
            commit_hash: None,
        })
        .unwrap();

        assert!(matches!(reset, TaskTransitionOutcome::Transitioned(_)));
        assert_eq!(stored_status(&store, "T002"), TaskStatus::Todo);
    }

    #[test]
    fn test_a_task_from_a_later_batch_cannot_start_until_the_current_batch_is_settled() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // B1 = T001, B2 = T002, and nothing is settled yet, so B1 is the batch
        // being consumed.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5), estimate("T002", 10, 5)],
            vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
        )
        .unwrap();

        let waiting =
            store_with(plan_of(&[("T001", TaskStatus::Todo), ("T002", TaskStatus::Todo)]));
        let outcome = admitting_interactor(waiting, batch_plan.clone(), Vec::new(), Some(500))
            .transition_task(start_work_on("T002"))
            .unwrap();

        assert_eq!(
            outcome.rejection(),
            Some(&AdmissionRejectionOutput::NotCurrentBatchMember {
                task_id: "T002".to_owned(),
                task_batch: "B2".to_owned(),
                current_batch: "B1".to_owned(),
            })
        );

        // With B1's only member committed, B2 becomes the batch being consumed
        // and the same task starts.
        let settled = store_with(plan_of(&[
            ("T001", TaskStatus::DoneTraced { commit_hash: commit_hash() }),
            ("T002", TaskStatus::Todo),
        ]));
        let outcome = admitting_interactor(settled, batch_plan, Vec::new(), Some(500))
            .transition_task(start_work_on("T002"))
            .unwrap();

        assert_eq!(transitioned(outcome).derived_status, "in_progress");
    }

    #[test]
    fn test_a_batch_advances_on_a_recorded_commit_and_not_on_a_done_mark_alone() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // B1 = T001, B2 = T002 again.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5), estimate("T002", 10, 5)],
            vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
        )
        .unwrap();

        // T001 is marked done with no commit recorded: its work is not on the
        // branch yet, so B1 is still the batch being consumed.
        let done_without_commit =
            store_with(plan_of(&[("T001", TaskStatus::DonePending), ("T002", TaskStatus::Todo)]));
        let outcome =
            admitting_interactor(done_without_commit, batch_plan.clone(), Vec::new(), Some(500))
                .transition_task(start_work_on("T002"))
                .unwrap();

        assert_eq!(
            outcome.rejection(),
            Some(&AdmissionRejectionOutput::NotCurrentBatchMember {
                task_id: "T002".to_owned(),
                task_batch: "B2".to_owned(),
                current_batch: "B1".to_owned(),
            })
        );

        // A member that will never be committed does not hold the batch open.
        let skipped =
            store_with(plan_of(&[("T001", TaskStatus::Skipped), ("T002", TaskStatus::Todo)]));
        let outcome = admitting_interactor(skipped, batch_plan, Vec::new(), Some(500))
            .transition_task(start_work_on("T002"))
            .unwrap();

        assert!(matches!(outcome, TaskTransitionOutcome::Transitioned(_)));
    }

    #[test]
    fn test_the_first_contribution_to_a_scope_starts_however_large_its_estimate_is() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // 1000 declared domain lines against a ceiling of 500, with nothing
        // measured and no member in progress: the scope carries nothing yet.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 700, 300)],
            vec![batch("B1", &["T001"])],
        )
        .unwrap();

        let outcome = admitting_interactor(
            store_with(plan_of(&[("T001", TaskStatus::Todo)])),
            batch_plan,
            Vec::new(),
            Some(500),
        )
        .transition_task(start_work_on("T001"))
        .unwrap();

        assert_eq!(transitioned(outcome).derived_status, "in_progress");
    }

    #[test]
    fn test_a_scope_that_already_carries_work_refuses_a_candidate_that_would_pass_its_ceiling() {
        use domain::TaskStatus;
        use domain::batch_plan::{BatchPlanDocument, LineCount, MeasuredScopeDiff};
        use domain::review_v2::{MainScopeName, ScopeName};

        // B1 holds both tasks; T001 is already in progress with 300 declared
        // domain lines, and 100 more are measured on disk. The candidate's own
        // 200 would take the scope to 600 against a ceiling of 500.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 200, 100), estimate("T002", 150, 50)],
            vec![batch("B1", &["T001", "T002"])],
        )
        .unwrap();

        let outcome = admitting_interactor(
            store_with(plan_of(&[("T001", TaskStatus::InProgress), ("T002", TaskStatus::Todo)])),
            batch_plan,
            vec![MeasuredScopeDiff::new(
                ScopeName::Main(MainScopeName::new("domain").unwrap()),
                LineCount::new(100),
            )],
            Some(500),
        )
        .transition_task(start_work_on("T002"))
        .unwrap();

        assert_eq!(
            outcome.rejection(),
            Some(&AdmissionRejectionOutput::ScopeCeilingWouldBeExceeded {
                scope: "domain".to_owned(),
                // 100 measured + 300 declared by the member in progress.
                prior_contribution: std::num::NonZeroU32::new(400).unwrap(),
                candidate_estimate: 200,
                ceiling: 500,
            })
        );
    }

    #[test]
    fn test_a_candidate_without_an_estimate_makes_the_transition_an_error() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // The batch plan knows T001 only; T002 is planned work the plan never
        // estimated, so there is nothing to compare and the start is refused as
        // an error rather than passing silently.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5)],
            vec![batch("B1", &["T001"])],
        )
        .unwrap();

        let error = admitting_interactor(
            store_with(plan_of(&[("T001", TaskStatus::DonePending), ("T002", TaskStatus::Todo)])),
            batch_plan,
            Vec::new(),
            Some(500),
        )
        .transition_task(start_work_on("T002"))
        .unwrap_err();

        assert!(matches!(error, TaskOperationError::TransitionFailed(_)), "unexpected: {error}");
        assert!(error.to_string().contains("T002"), "the failure names the task: {error}");
        assert!(error.to_string().contains("no estimate"), "unexpected: {error}");
    }

    #[test]
    fn test_an_estimate_naming_an_unconfigured_scope_makes_the_transition_an_error() {
        use domain::TaskStatus;
        use domain::batch_plan::{
            BatchPlanDocument, LineCount, ScopeLineEstimate, TaskDecomposition, TaskEstimate,
        };
        use domain::review_v2::{MainScopeName, ScopeName};

        // The estimate names a misspelled scope, so no ceiling can be resolved
        // for it: the start fails rather than degrading into an unconstrained
        // comparison that admits any figure.
        let scope = ScopeName::Main(MainScopeName::new("domian").unwrap());
        let misspelled = TaskEstimate::new(
            TaskId::try_new("T001").unwrap(),
            vec![ScopeLineEstimate::new(scope, LineCount::new(10), LineCount::new(5))],
            TaskDecomposition::Decomposable,
        )
        .unwrap();
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![misspelled],
            vec![batch("B1", &["T001"])],
        )
        .unwrap();

        let error = admitting_interactor(
            store_with(plan_of(&[("T001", TaskStatus::Todo)])),
            batch_plan,
            Vec::new(),
            Some(500),
        )
        .transition_task(start_work_on("T001"))
        .unwrap_err();

        assert!(matches!(error, TaskOperationError::TransitionFailed(_)), "unexpected: {error}");
        assert!(error.to_string().contains("T001"), "the failure names the task: {error}");
        assert!(error.to_string().contains("domian"), "the failure names the scope: {error}");
    }

    #[test]
    fn test_a_start_whose_estimate_declares_only_the_reserved_other_scope_is_admitted() {
        use domain::TaskStatus;
        use domain::batch_plan::{BatchPlanDocument, LineCount, MeasuredScopeDiff};
        use domain::review_v2::ScopeName;

        // `other` is reserved rather than unconfigured: naming it is not the
        // unknown-scope error, and it resolves no ceiling for its total to be
        // compared against. T001 already contributes 500 declared lines and
        // another 100 are measured, so this is not admitted by the
        // first-contributor exception; T002's 1,100 lines still start because
        // the reserved scope is exempt.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![
                estimate_in(ScopeName::Other, "T001", 400, 100),
                estimate_in(ScopeName::Other, "T002", 900, 200),
            ],
            vec![batch("B1", &["T001", "T002"])],
        )
        .unwrap();

        let store =
            store_with(plan_of(&[("T001", TaskStatus::InProgress), ("T002", TaskStatus::Todo)]));
        let outcome = admitting_interactor(
            Arc::clone(&store),
            batch_plan,
            vec![MeasuredScopeDiff::new(ScopeName::Other, LineCount::new(100))],
            Some(500),
        )
        .transition_task(start_work_on("T002"))
        .unwrap();

        assert!(matches!(outcome, TaskTransitionOutcome::Transitioned(_)));
        assert_eq!(stored_status(&store, "T002"), TaskStatus::InProgress);
    }

    #[test]
    fn test_a_track_without_a_batch_plan_cannot_start_work_but_still_finishes_it() {
        use domain::TaskStatus;

        // Starting work is judged against the batch plan, so its absence is an
        // error: the start never passes unjudged.
        let error = TaskOperationInteractor::new(
            store_with(plan_of(&[("T001", TaskStatus::Todo)])),
            None,
            Arc::new(StubBatchPlanReader(None)),
            Arc::new(StubScopeDiffMeasurer(Vec::new())),
            Arc::new(StubScopeConfigReader(vec![("domain", Some(500))])),
            StubVerifier::accepting(),
        )
        .transition_task(start_work_on("T001"))
        .unwrap_err();

        assert!(matches!(error, TaskOperationError::TransitionFailed(_)), "unexpected: {error}");
        assert!(error.to_string().contains("no batch plan"), "unexpected: {error}");

        // Finishing work already under way is a different transition and is not
        // judged, so a track without the artifact still closes its tasks.
        let outcome = TaskOperationInteractor::new(
            store_with(plan_of(&[("T001", TaskStatus::InProgress)])),
            None,
            Arc::new(StubBatchPlanReader(None)),
            Arc::new(StubScopeDiffMeasurer(Vec::new())),
            Arc::new(StubScopeConfigReader(vec![("domain", Some(500))])),
            StubVerifier::accepting(),
        )
        .transition_task(TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "done".to_owned(),
            commit_hash: None,
        })
        .unwrap();

        assert!(matches!(outcome, TaskTransitionOutcome::Transitioned(_)));
    }

    #[test]
    fn test_adding_a_task_names_the_id_it_created_and_the_status_that_follows() {
        use domain::TaskStatus;

        let store = store_with(plan_of(&[("T001", TaskStatus::Todo)]));
        let output = task_ops_interactor(Arc::clone(&store), None)
            .add_task(AddTaskCommand {
                items_dir: PathBuf::new(),
                track_id: "my-track-2026".to_owned(),
                description: "second task".to_owned(),
                section: None,
                after_task_id: None,
            })
            .unwrap();

        assert_eq!(output.track_id, "my-track-2026");
        assert_eq!(output.task_id.as_deref(), Some("T002"));
        // Two tasks, neither started: the track is still planned.
        assert_eq!(output.derived_status, "planned");

        let stored = store.impl_plans.lock().unwrap();
        let plan = stored.get(&TrackId::try_new("my-track-2026").unwrap()).unwrap();
        assert_eq!(plan.tasks().len(), 2);
        assert_eq!(plan.tasks()[1].id().as_ref(), "T002");
    }

    #[test]
    fn test_an_override_dominates_the_derived_status_until_it_is_cleared() {
        use domain::TaskStatus;

        let store = store_with(plan_of(&[("T001", TaskStatus::Todo)]));
        let interactor = task_ops_interactor(Arc::clone(&store), None);

        let blocked = interactor
            .set_override(SetOverrideCommand {
                items_dir: PathBuf::new(),
                track_id: "my-track-2026".to_owned(),
                status: "blocked".to_owned(),
                reason: "waiting on a decision".to_owned(),
            })
            .unwrap();
        assert_eq!(blocked.derived_status, "blocked");

        // Cleared, the status is derived from the plan again.
        let cleared = interactor
            .clear_override(ClearOverrideCommand {
                items_dir: PathBuf::new(),
                track_id: "my-track-2026".to_owned(),
            })
            .unwrap();
        assert_eq!(cleared.derived_status, "planned");

        // An override kind the domain does not define is refused rather than
        // written.
        let error = interactor
            .set_override(SetOverrideCommand {
                items_dir: PathBuf::new(),
                track_id: "my-track-2026".to_owned(),
                status: "paused".to_owned(),
                reason: "no such kind".to_owned(),
            })
            .unwrap_err();
        assert!(matches!(error, TaskOperationError::TransitionFailed(_)), "unexpected: {error}");
    }

    #[test]
    fn test_finishing_a_task_is_not_judged_by_the_admission_guard() {
        use domain::TaskStatus;
        use domain::batch_plan::BatchPlanDocument;

        // A plan whose membership rule would refuse T002 if it were starting:
        // B1 still has an unsettled member. Finishing work already in progress
        // is a different transition and goes through.
        let batch_plan = BatchPlanDocument::new(
            TrackId::try_new("my-track-2026").unwrap(),
            vec![estimate("T001", 10, 5), estimate("T002", 10, 5)],
            vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
        )
        .unwrap();

        let outcome = admitting_interactor(
            store_with(plan_of(&[("T001", TaskStatus::Todo), ("T002", TaskStatus::InProgress)])),
            batch_plan,
            Vec::new(),
            Some(500),
        )
        .transition_task(TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T002".to_owned(),
            target_status: "done".to_owned(),
            commit_hash: None,
        })
        .unwrap();

        assert!(matches!(outcome, TaskTransitionOutcome::Transitioned(_)));
    }

    // ── record-time verification of a commit hash ─────────────────────────────

    /// The two recording lanes side by side: `T001` is in progress, so
    /// completing it records the hash, and `T002` is done without one, so the
    /// same request against it is a backfill.
    fn recording_plan() -> ImplPlanDocument {
        use domain::TaskStatus;
        plan_of(&[("T001", TaskStatus::InProgress), ("T002", TaskStatus::DonePending)])
    }

    /// A request to record `commit_hash()` against `task_id`.
    fn record_hash(task_id: &str) -> TaskTransitionCommand {
        TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: task_id.to_owned(),
            target_status: "done".to_owned(),
            commit_hash: Some(commit_hash().as_ref().to_owned()),
        }
    }

    /// The interactor with the given verifier. The batch plan is absent because
    /// no recording transition is judged against one: what these tests observe
    /// is the verification, and a reader that is never consulted cannot supply
    /// the outcome.
    fn recording_interactor(
        store: Arc<StubStore>,
        verifier: Arc<dyn CommitRecordVerifierPort>,
    ) -> TaskOperationInteractor<StubStore> {
        TaskOperationInteractor::new(
            store,
            None,
            Arc::new(StubBatchPlanReader(None)),
            Arc::new(StubScopeDiffMeasurer(Vec::new())),
            Arc::new(StubScopeConfigReader(vec![("domain", Some(500))])),
            verifier,
        )
    }

    /// The persisted plan as a byte string.
    ///
    /// The stub store holds the very document `impl-plan.json` would be written
    /// from, so comparing this rendering across an attempt is the byte
    /// comparison of the file AC-28 requires: a partial application to any task
    /// or any field outside the task list changes it.
    fn persisted(store: &StubStore) -> String {
        let plans = store.impl_plans.lock().unwrap();
        format!("{:?}", plans.get(&TrackId::try_new("my-track-2026").unwrap()).unwrap())
    }

    /// Attempts to record a refused hash against `task_id`, returning the error
    /// and whether the persisted plan survived the attempt unchanged.
    fn refused_recording(
        task_id: &str,
        verifier: Arc<dyn CommitRecordVerifierPort>,
    ) -> (TaskOperationError, bool) {
        let store = store_with(recording_plan());
        let before = persisted(&store);
        let error = recording_interactor(Arc::clone(&store), verifier)
            .transition_task(record_hash(task_id))
            .unwrap_err();
        let unchanged = persisted(&store) == before;
        (error, unchanged)
    }

    /// `T001` is the completion lane and `T002` the backfill lane: the same
    /// request reaches a different transition on each, and neither is exempt.
    const RECORDING_LANES: [(&str, &str); 2] =
        [("T001", "the completion lane"), ("T002", "the backfill lane")];

    #[test]
    fn test_a_hash_the_repository_does_not_hold_is_recorded_on_neither_lane() {
        for (task_id, lane) in RECORDING_LANES {
            let (error, unchanged) = refused_recording(
                task_id,
                StubVerifier::refusing(|| CommitRecordVerifyError::CommitNotFound {
                    commit_hash: commit_hash(),
                }),
            );

            assert!(
                matches!(error, TaskOperationError::TransitionFailed(_)),
                "{lane} must refuse a hash naming no commit: {error}"
            );
            assert!(unchanged, "{lane} must leave the persisted plan byte-identical");
        }
    }

    #[test]
    fn test_a_hash_that_is_not_an_ancestor_of_head_is_recorded_on_neither_lane() {
        for (task_id, lane) in RECORDING_LANES {
            let (error, unchanged) = refused_recording(
                task_id,
                StubVerifier::refusing(|| CommitRecordVerifyError::NotAncestorOfHead {
                    commit_hash: commit_hash(),
                }),
            );

            assert!(
                matches!(error, TaskOperationError::TransitionFailed(_)),
                "{lane} must refuse a commit unreachable from HEAD: {error}"
            );
            assert!(unchanged, "{lane} must leave the persisted plan byte-identical");
        }
    }

    #[test]
    fn test_a_repository_that_cannot_be_read_records_on_neither_lane() {
        // No verdict was produced, and the absence of a refusal is not an
        // acceptance: there is no graceful skip that records the hash unchecked.
        for (task_id, lane) in RECORDING_LANES {
            let (error, unchanged) = refused_recording(
                task_id,
                StubVerifier::refusing(|| CommitRecordVerifyError::RepositoryUnreadable {
                    message: domain::FreeText::new("not a git repository"),
                }),
            );

            assert!(
                matches!(error, TaskOperationError::StoreFailed(_)),
                "{lane} must report an unperformed check as a store failure: {error}"
            );
            assert!(unchanged, "{lane} must leave the persisted plan byte-identical");
        }
    }

    #[test]
    fn test_a_hash_the_repository_accepts_is_recorded_on_both_lanes() {
        use domain::TaskStatus;

        for (task_id, lane) in RECORDING_LANES {
            let store = store_with(recording_plan());
            let outcome = recording_interactor(Arc::clone(&store), StubVerifier::accepting())
                .transition_task(record_hash(task_id))
                .unwrap();

            assert!(
                matches!(outcome, TaskTransitionOutcome::Transitioned(_)),
                "{lane} must record an accepted hash"
            );
            assert_eq!(
                stored_status(&store, task_id),
                TaskStatus::DoneTraced { commit_hash: commit_hash() },
                "{lane} must persist the accepted hash"
            );
        }
    }

    #[test]
    fn test_a_completion_carrying_no_hash_is_recorded_without_being_verified() {
        use domain::TaskStatus;

        // Nothing would be written, so there is nothing to verify: the verifier
        // refuses everything it is asked about, and the completion still goes
        // through — as an unsettled `done`, exactly as before (AC-29).
        let store = store_with(recording_plan());
        let outcome = recording_interactor(
            Arc::clone(&store),
            StubVerifier::refusing(|| CommitRecordVerifyError::CommitNotFound {
                commit_hash: commit_hash(),
            }),
        )
        .transition_task(TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "done".to_owned(),
            commit_hash: None,
        })
        .unwrap();

        assert!(matches!(outcome, TaskTransitionOutcome::Transitioned(_)));
        assert_eq!(stored_status(&store, "T001"), TaskStatus::DonePending);
    }
}
