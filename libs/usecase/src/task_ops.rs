//! Track task operation and query application services (usecase layer).
//!
//! Wraps `domain::TransitionTaskUseCase`, `domain::AddTaskUseCase`, and
//! `domain::SetOverrideUseCase` behind usecase-owned service traits so the
//! CLI never imports `domain::TrackId`, `domain::TaskId`,
//! `domain::CommitHash`, `domain::StatusOverride`, `domain::DomainError`,
//! `domain::TrackReadError`, or `domain::TrackWriteError` directly (CN-01 / D1).

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use domain::{
    CommitHash, DomainError, ImplPlanReader, ImplPlanWriter, RepositoryError, StatusOverride,
    TaskId, TrackId, TrackReadError, TrackReader, TrackWriteError, TrackWriter, TransitionError,
    derive_track_status,
};

use crate::track_resolution::{BranchReadError, BranchReaderPort};

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
    /// # Errors
    ///
    /// Returns [`TaskOperationError`] on ID validation, not-found, or transition
    /// failures.
    fn transition_task(
        &self,
        cmd: TaskTransitionCommand,
    ) -> Result<TaskOperationOutput, TaskOperationError>;

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
    #[must_use]
    pub fn new(store: Arc<S>, branch_reader: Option<Arc<dyn BranchReaderPort>>) -> Self {
        Self { store, branch_reader }
    }

    /// Test-only constructor with an injected `Option<Arc<dyn BranchReaderPort>>`.
    ///
    /// Intra-crate test helpers use this to share a pre-built `Arc` branch reader;
    /// external callers should use [`Self::new`] instead.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_branch_reader(
        store: Arc<S>,
        branch_reader: Option<Arc<dyn BranchReaderPort>>,
    ) -> Self {
        Self { store, branch_reader }
    }
}

// ── Branch guard logic ────────────────────────────────────────────────────────

/// Enforces the branch guard for track mutation operations.
///
/// When a `branch_reader` port is provided:
/// - Branchless tracks (`branch = None` in metadata) pass the guard unconditionally.
/// - Tracks with a branch require the current git branch (returned by
///   [`BranchReaderPort::current_branch`]) to match the expected branch;
///   detached HEAD state is rejected.
/// - When the current branch does not match, returns
///   [`TaskOperationError::BranchGuardFailed`].
/// - When the HEAD is detached (reader returns `Some("HEAD")`), returns
///   [`TaskOperationError::BranchlessGuardFailed`].
///
/// When no `branch_reader` is provided, the guard is a no-op.
///
/// # Errors
///
/// Returns [`TaskOperationError::BranchGuardFailed`] when the branch does not match.
/// Returns [`TaskOperationError::BranchlessGuardFailed`] for detached HEAD state.
fn enforce_branch_guard<R: TrackReader>(
    store: &R,
    track_id: &TrackId,
    _items_dir: &std::path::Path,
    branch_reader: Option<&Arc<dyn BranchReaderPort>>,
) -> Result<(), TaskOperationError> {
    let Some(reader) = branch_reader else {
        return Ok(()); // no reader injected — skip guard
    };

    // Read track metadata to determine expected branch.
    let track = store
        .find(track_id)
        .map_err(TaskOperationError::from)?
        .ok_or_else(|| TaskOperationError::TrackNotFound(track_id.to_string()))?;

    let expected_branch = match track.branch() {
        None => return Ok(()), // branchless track — skip guard
        Some(b) => b.as_ref().to_owned(),
    };

    // Delegate branch reading to the injected port.
    let actual_branch_opt =
        reader.current_branch().map_err(|BranchReadError::ReadFailed(msg)| {
            TaskOperationError::BranchGuardFailed(format!("branch read failed: {msg}"))
        })?;

    let actual_branch = match actual_branch_opt {
        Some(b) => b,
        None => {
            return Err(TaskOperationError::BranchGuardFailed(
                "branch read returned no branch name".to_owned(),
            ));
        }
    };

    // Detached HEAD → ambiguous branch state.
    if actual_branch == "HEAD" {
        return Err(TaskOperationError::BranchlessGuardFailed(format!(
            "detached HEAD — expected branch '{expected_branch}', cannot verify"
        )));
    }

    // Branch mismatch → guard fails.
    if actual_branch != expected_branch {
        return Err(TaskOperationError::BranchGuardFailed(format!(
            "current branch '{actual_branch}' does not match expected '{expected_branch}'"
        )));
    }

    Ok(())
}

impl<S> TaskOperationService for TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    fn transition_task(
        &self,
        cmd: TaskTransitionCommand,
    ) -> Result<TaskOperationOutput, TaskOperationError> {
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

        Ok(TaskOperationOutput {
            track_id: track.id().as_ref().to_owned(),
            task_id: None,
            derived_status: status,
        })
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
        let interactor = TaskOperationInteractor::new(
            Arc::clone(&store),
            StubBranchReader::ok("track/my-track-2026"),
        );
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let out = interactor.transition_task(cmd).unwrap();
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
        let interactor =
            TaskOperationInteractor::new(Arc::clone(&store), StubBranchReader::ok("main"));
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

        let interactor =
            TaskOperationInteractor::new(Arc::clone(&store), StubBranchReader::ok("HEAD"));
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
        let interactor =
            TaskOperationInteractor::new(Arc::clone(&store), StubBranchReader::ok("any"));
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
        let interactor = TaskOperationInteractor::new(Arc::clone(&store), None);
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let out = interactor.transition_task(cmd).unwrap();
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
        let interactor =
            TaskOperationInteractor::with_branch_reader(Arc::clone(&store), branch_reader);
        let cmd = TaskTransitionCommand {
            items_dir: PathBuf::new(),
            track_id: "my-track-2026".to_owned(),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        };
        let out = interactor.transition_task(cmd).unwrap();
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
        let interactor =
            TaskOperationInteractor::with_branch_reader(Arc::clone(&store), branch_reader);
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
        let interactor =
            TaskOperationInteractor::with_branch_reader(Arc::clone(&store), branch_reader);
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
}
