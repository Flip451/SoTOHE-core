#![forbid(unsafe_code)]
//! Use case layer for the SoTOHE-core track state machine.

pub mod contract_map_workflow;
pub mod git_workflow;
pub mod hook;
pub mod merge_gate;
pub mod pr_review;
pub mod pr_workflow;
pub mod review_v2;
pub mod review_workflow;
pub mod task_completion;
pub mod track_activation;
pub mod track_phase;
pub mod track_resolution;
pub mod worktree_guard;

use std::sync::Arc;

use domain::{
    CommitHash, DomainError, ImplPlanDocument, ImplPlanReader, ImplPlanWriter, RepositoryError,
    StatusOverride, TaskId, TaskStatusKind, TaskTransition, TrackId, TrackMetadata, TrackReadError,
    TrackReader, TrackStatus, TrackWriteError, TrackWriter, ValidationError,
};

/// Persists a track aggregate.
pub struct SaveTrackUseCase<W: TrackWriter> {
    writer: Arc<W>,
}

impl<W: TrackWriter> SaveTrackUseCase<W> {
    #[must_use]
    pub fn new(writer: Arc<W>) -> Self {
        Self { writer }
    }

    /// Saves the given track.
    ///
    /// # Errors
    /// Returns `TrackWriteError` on persistence failure.
    pub fn execute(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
        self.writer.save(track)
    }
}

/// Loads a track aggregate by ID.
pub struct LoadTrackUseCase<R: TrackReader> {
    reader: Arc<R>,
}

impl<R: TrackReader> LoadTrackUseCase<R> {
    #[must_use]
    pub fn new(reader: Arc<R>) -> Self {
        Self { reader }
    }

    /// Loads a track by ID, returning `None` if not found.
    ///
    /// # Errors
    /// Returns `TrackReadError` on persistence failure.
    pub fn execute(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
        self.reader.find(id)
    }
}

/// Derives `TrackStatus` from the current state of an `ImplPlanDocument`.
///
/// Rules (mirrors the pre-T005 `TrackMetadata::derive_status_from_tasks`):
/// - Empty plan → `TrackStatus::Planned`
/// - All tasks resolved (done/skipped) → `TrackStatus::Done`
/// - Any task `InProgress`, or any mix of at least one resolved + at least one
///   unresolved (todo) task → `TrackStatus::InProgress` (partial progress must
///   not regress to `Planned` after the first commit)
/// - All tasks `Todo` → `TrackStatus::Planned`
fn derive_track_status_from_impl_plan(plan: &ImplPlanDocument) -> TrackStatus {
    if plan.tasks().is_empty() {
        return TrackStatus::Planned;
    }
    if plan.all_tasks_resolved() {
        return TrackStatus::Done;
    }
    let any_in_progress =
        plan.tasks().iter().any(|t| matches!(t.status().kind(), TaskStatusKind::InProgress));
    let any_resolved = plan
        .tasks()
        .iter()
        .any(|t| matches!(t.status().kind(), TaskStatusKind::Done | TaskStatusKind::Skipped));
    if any_in_progress || any_resolved { TrackStatus::InProgress } else { TrackStatus::Planned }
}

/// Applies a state transition to a task within a track and persists the result.
///
/// Reads `impl-plan.json` via [`ImplPlanReader`], applies the transition on the
/// `ImplPlanDocument` aggregate, persists via [`ImplPlanWriter`], then synchronizes
/// `metadata.json` status via [`TrackWriter`] (unless a status override is active).
/// Returns the updated [`TrackMetadata`].
pub struct TransitionTaskUseCase<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter,
{
    store: Arc<S>,
}

impl<S> TransitionTaskUseCase<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Transitions a task by an explicit [`TaskTransition`] and persists the result.
    ///
    /// # Concurrency note
    /// This performs a non-serialized read-modify-write on `impl-plan.json`. Concurrent
    /// callers operating against the same track directory are not supported — the CLI is
    /// designed for sequential single-process execution, consistent with
    /// `FsTrackStore::with_locked_document`'s documented assumption that "concurrent
    /// callers are not supported — parallel access will be handled by worktree isolation".
    /// A future `ImplPlanWriter::update_impl_plan` port method would allow adapters to
    /// provide serialized R-M-W (tracked as a follow-up to T007).
    ///
    /// # Errors
    /// Returns `TrackWriteError::Repository(TrackNotFound)` if the track does not exist.
    /// Returns `TrackWriteError::Repository(Message)` if `impl-plan.json` is missing or
    /// cannot be read/written.
    /// Returns `TrackWriteError::Domain` if the transition is invalid or the task is not found.
    pub fn execute(
        &self,
        track_id: &TrackId,
        task_id: &TaskId,
        transition: TaskTransition,
    ) -> Result<TrackMetadata, TrackWriteError> {
        // Verify the track exists (check for override before any mutation).
        let track = self.store.find(track_id).map_err(TrackWriteError::from)?.ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(track_id.to_string()))
        })?;
        let has_override = track.status_override().is_some();

        // Load impl-plan.json (required for transition).
        let mut impl_plan =
            self.store.load_impl_plan(track_id).map_err(TrackWriteError::from)?.ok_or_else(
                || {
                    TrackWriteError::Repository(RepositoryError::Message(format!(
                        "impl-plan.json not found for track '{track_id}'"
                    )))
                },
            )?;

        // Apply transition on the domain aggregate.
        impl_plan.apply_transition(task_id, transition).map_err(TrackWriteError::from)?;

        // Persist impl-plan.json.
        self.store.save_impl_plan(track_id, &impl_plan).map_err(TrackWriteError::from)?;

        // Sync metadata.json status from impl-plan state (skip if override is active).
        if !has_override {
            let derived = derive_track_status_from_impl_plan(&impl_plan);
            let updated = self.store.update(track_id, |t| {
                t.set_status(derived);
                Ok::<(), DomainError>(())
            })?;
            return Ok(updated);
        }

        Ok(track)
    }

    /// Resolves a target status string to the correct transition and applies it.
    ///
    /// # Errors
    /// Returns `TrackWriteError::Repository(TrackNotFound)` if the track does not exist.
    /// Returns `TrackWriteError::Repository(Message)` if `impl-plan.json` is missing or
    /// cannot be read/written.
    /// Returns `TrackWriteError::Domain` if the target status is unrecognised, the task
    /// is not found, or the transition is invalid for the current state.
    pub fn execute_by_status(
        &self,
        track_id: &TrackId,
        task_id: &TaskId,
        target_status: &str,
        commit_hash: Option<CommitHash>,
    ) -> Result<TrackMetadata, TrackWriteError> {
        // Verify the track exists (check for override before any mutation).
        let track = self.store.find(track_id).map_err(TrackWriteError::from)?.ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(track_id.to_string()))
        })?;
        let has_override = track.status_override().is_some();

        // Load impl-plan.json (required for transition).
        let mut impl_plan =
            self.store.load_impl_plan(track_id).map_err(TrackWriteError::from)?.ok_or_else(
                || {
                    TrackWriteError::Repository(RepositoryError::Message(format!(
                        "impl-plan.json not found for track '{track_id}'"
                    )))
                },
            )?;

        // Apply transition by status string on the domain aggregate.
        impl_plan
            .apply_transition_by_status(task_id, target_status, commit_hash)
            .map_err(TrackWriteError::from)?;

        // Persist impl-plan.json.
        self.store.save_impl_plan(track_id, &impl_plan).map_err(TrackWriteError::from)?;

        // Sync metadata.json status from impl-plan state (skip if override is active).
        if !has_override {
            let derived = derive_track_status_from_impl_plan(&impl_plan);
            let updated = self.store.update(track_id, |t| {
                t.set_status(derived);
                Ok::<(), DomainError>(())
            })?;
            return Ok(updated);
        }

        Ok(track)
    }
}

/// Adds a new task to a track's `impl-plan.json` and persists the result.
///
/// Reads `impl-plan.json` via [`ImplPlanReader`], delegates to
/// [`domain::ImplPlanDocument::add_task`], persists via [`ImplPlanWriter`], then
/// re-derives and syncs `metadata.json` status via [`TrackWriter`] (a new Todo task
/// on a previously-Done track moves it back to `InProgress`/`Planned`).
/// Returns the updated [`TrackMetadata`] and the newly-allocated [`TaskId`].
pub struct AddTaskUseCase<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter,
{
    store: Arc<S>,
}

impl<S> AddTaskUseCase<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Adds a task to the track and persists the result.
    ///
    /// # Concurrency note
    /// This performs a non-serialized read-modify-write on `impl-plan.json`. See
    /// [`TransitionTaskUseCase::execute`] for the documented single-process assumption
    /// and the follow-up T007 note for `ImplPlanWriter::update_impl_plan`.
    ///
    /// # Errors
    /// Returns `TrackWriteError::Repository(TrackNotFound)` if the track does not exist.
    /// Returns `TrackWriteError::Repository(Message)` if `impl-plan.json` is missing or
    /// cannot be read/written.
    /// Returns `TrackWriteError::Domain` if `description` is empty or the target section
    /// does not exist.
    pub fn execute(
        &self,
        track_id: &TrackId,
        description: &str,
        section_id: Option<&str>,
        after_task_id: Option<&TaskId>,
    ) -> Result<(TrackMetadata, TaskId), TrackWriteError> {
        // Validate description early (mirrors domain validation for clear early error).
        if description.trim().is_empty() {
            return Err(TrackWriteError::Domain(ValidationError::EmptyTaskDescription.into()));
        }

        // Verify the track exists (check for override before any mutation).
        let track = self.store.find(track_id).map_err(TrackWriteError::from)?.ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(track_id.to_string()))
        })?;
        let has_override = track.status_override().is_some();

        // Load impl-plan.json (required for task management).
        let mut impl_plan =
            self.store.load_impl_plan(track_id).map_err(TrackWriteError::from)?.ok_or_else(
                || {
                    TrackWriteError::Repository(RepositoryError::Message(format!(
                        "impl-plan.json not found for track '{track_id}'"
                    )))
                },
            )?;

        // Add the task on the domain aggregate.
        let new_task_id = impl_plan
            .add_task(description, section_id, after_task_id)
            .map_err(TrackWriteError::from)?;

        // Persist impl-plan.json.
        self.store.save_impl_plan(track_id, &impl_plan).map_err(TrackWriteError::from)?;

        // Sync metadata.json status (skip if override is active).
        // A new Todo task added to a Done track must move it back to Planned/InProgress.
        if !has_override {
            let derived = derive_track_status_from_impl_plan(&impl_plan);
            let updated = self.store.update(track_id, |t| {
                t.set_status(derived);
                Ok::<(), DomainError>(())
            })?;
            return Ok((updated, new_task_id));
        }

        Ok((track, new_task_id))
    }
}

/// Sets or clears a status override on a track and persists the result atomically.
pub struct SetOverrideUseCase<W: TrackWriter> {
    writer: Arc<W>,
}

impl<W: TrackWriter> SetOverrideUseCase<W> {
    #[must_use]
    pub fn new(writer: Arc<W>) -> Self {
        Self { writer }
    }

    /// Sets a status override on the track and persists atomically.
    ///
    /// # Errors
    /// Returns `TrackWriteError` if the track is not found.
    pub fn execute(
        &self,
        track_id: &TrackId,
        status_override: Option<StatusOverride>,
    ) -> Result<TrackMetadata, TrackWriteError> {
        self.writer.update(track_id, |track| {
            // T005: set_status_override no longer returns Result (it is infallible).
            // Mirror the status so track.status() stays consistent with the override.
            if let Some(ref ov) = status_override {
                track.set_status(ov.track_status());
            } else {
                // Clearing override: revert to Planned.
                // T005: task transitions are stubbed (T007 pending), so tracks cannot
                // organically reach InProgress/Done. Reverting to Planned is correct for
                // the T005 stub phase.
                // TODO T007: once TransitionTaskUseCase operates on impl-plan.json, the
                // pre-override status must be preserved (e.g. via a stored field or an
                // explicit restore_status parameter) so that clearing an override on an
                // active track restores the correct phase rather than resetting to Planned.
                track.set_status(domain::TrackStatus::Planned);
            }
            track.set_status_override(status_override);
            Ok(())
        })
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use domain::{
        DomainError, ImplPlanDocument, ImplPlanReader, ImplPlanWriter, PlanSection, PlanView,
        RepositoryError, StatusOverride, TaskId, TaskStatus, TaskTransition, TrackId,
        TrackMetadata, TrackReadError, TrackReader, TrackStatus, TrackTask, TrackWriteError,
        TrackWriter, ValidationError,
    };

    use super::{
        AddTaskUseCase, LoadTrackUseCase, SaveTrackUseCase, SetOverrideUseCase,
        TransitionTaskUseCase,
    };

    #[derive(Default)]
    struct StubTrackStore {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
        impl_plans: Mutex<HashMap<TrackId, ImplPlanDocument>>,
    }

    impl TrackReader for StubTrackStore {
        fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
            let tracks = self
                .tracks
                .lock()
                .map_err(|_| RepositoryError::Message("lock error".to_owned()))?;
            Ok(tracks.get(id).cloned())
        }
    }

    impl TrackWriter for StubTrackStore {
        fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
            let mut tracks = self
                .tracks
                .lock()
                .map_err(|_| RepositoryError::Message("lock error".to_owned()))?;
            tracks.insert(track.id().clone(), track.clone());
            Ok(())
        }

        fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
        where
            F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>,
        {
            let mut tracks = self.tracks.lock().map_err(|_| {
                TrackWriteError::Repository(RepositoryError::Message("lock error".to_owned()))
            })?;
            let track = tracks.get_mut(id).ok_or_else(|| {
                TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
            })?;
            mutate(track).map_err(TrackWriteError::from)?;
            Ok(track.clone())
        }
    }

    impl ImplPlanReader for StubTrackStore {
        fn load_impl_plan(
            &self,
            id: &TrackId,
        ) -> Result<Option<ImplPlanDocument>, RepositoryError> {
            let plans = self
                .impl_plans
                .lock()
                .map_err(|_| RepositoryError::Message("lock error".to_owned()))?;
            Ok(plans.get(id).cloned())
        }
    }

    impl ImplPlanWriter for StubTrackStore {
        fn save_impl_plan(
            &self,
            id: &TrackId,
            doc: &ImplPlanDocument,
        ) -> Result<(), RepositoryError> {
            let mut plans = self
                .impl_plans
                .lock()
                .map_err(|_| RepositoryError::Message("lock error".to_owned()))?;
            plans.insert(id.clone(), doc.clone());
            Ok(())
        }
    }

    fn sample_track() -> TrackMetadata {
        // T005: identity-only TrackMetadata; tasks/plan live in impl-plan.json.
        TrackMetadata::new(
            TrackId::try_new("track-state-machine").unwrap(),
            "Track state machine",
            TrackStatus::Planned,
            None,
        )
        .unwrap()
    }

    fn sample_impl_plan() -> ImplPlanDocument {
        let task = TrackTask::new(TaskId::try_new("T001").unwrap(), "First task").unwrap();
        let section =
            PlanSection::new("S1", "Impl", vec![], vec![TaskId::try_new("T001").unwrap()]).unwrap();
        ImplPlanDocument::new(vec![task], PlanView::new(vec![], vec![section])).unwrap()
    }

    #[test]
    fn transition_usecase_missing_impl_plan_returns_repository_error() {
        // When impl-plan.json is not present, TransitionTaskUseCase returns a
        // RepositoryError (not a Domain error) explaining the missing document.
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let task_id = TaskId::try_new("T001").unwrap();

        save.execute(&track).unwrap();
        let result = transition.execute(track.id(), &task_id, TaskTransition::Start);
        assert!(result.is_err());
        assert!(
            matches!(result, Err(TrackWriteError::Repository(_))),
            "expected RepositoryError when impl-plan.json is missing"
        );
    }

    #[test]
    fn transition_usecase_execute_by_status_transitions_task_and_syncs_metadata_status() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let impl_plan = sample_impl_plan();
        let task_id = TaskId::try_new("T001").unwrap();

        save.execute(&track).unwrap();
        store.save_impl_plan(track.id(), &impl_plan).unwrap();

        let result = transition.execute_by_status(track.id(), &task_id, "in_progress", None);
        assert!(result.is_ok(), "transition to in_progress must succeed: {result:?}");
        let returned_track = result.unwrap();

        // Verify the impl-plan task was updated.
        let updated_plan = store.load_impl_plan(track.id()).unwrap().unwrap();
        assert!(
            matches!(updated_plan.tasks()[0].status(), TaskStatus::InProgress),
            "task must be InProgress after transition"
        );
        // Verify metadata.status was synced.
        assert_eq!(
            returned_track.status(),
            TrackStatus::InProgress,
            "metadata.status must be synced to InProgress when a task is in_progress"
        );
        let stored_track = store.find(track.id()).unwrap().unwrap();
        assert_eq!(
            stored_track.status(),
            TrackStatus::InProgress,
            "persisted metadata.status must also be InProgress"
        );
    }

    #[test]
    fn transition_usecase_execute_with_explicit_transition_transitions_task_and_syncs_status() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let impl_plan = sample_impl_plan();
        let task_id = TaskId::try_new("T001").unwrap();

        save.execute(&track).unwrap();
        store.save_impl_plan(track.id(), &impl_plan).unwrap();

        let result = transition.execute(track.id(), &task_id, TaskTransition::Start);
        assert!(result.is_ok(), "explicit Start transition must succeed: {result:?}");
        // Metadata status should be synced to InProgress.
        assert_eq!(result.unwrap().status(), TrackStatus::InProgress);
    }

    #[test]
    fn transition_usecase_returns_error_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track_id = TrackId::try_new("nonexistent").unwrap();
        let task_id = TaskId::try_new("T001").unwrap();

        let result = transition.execute(&track_id, &task_id, TaskTransition::Start);
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn add_task_usecase_returns_error_for_empty_description() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let add_task = AddTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();

        save.execute(&track).unwrap();
        let result = add_task.execute(track.id(), "", None, None);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(TrackWriteError::Domain(domain::DomainError::Validation(
                ValidationError::EmptyTaskDescription
            )))
        ));
    }

    #[test]
    fn add_task_usecase_adds_task_to_impl_plan() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let add_task = AddTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let impl_plan = sample_impl_plan();

        save.execute(&track).unwrap();
        store.save_impl_plan(track.id(), &impl_plan).unwrap();

        let result = add_task.execute(track.id(), "New task description", None, None);
        assert!(result.is_ok(), "add_task must succeed: {result:?}");
        let (_, new_id) = result.unwrap();
        assert_eq!(new_id.as_ref(), "T002", "new task must get next ID T002");

        // Verify the impl-plan was updated.
        let updated_plan = store.load_impl_plan(track.id()).unwrap().unwrap();
        assert_eq!(updated_plan.tasks().len(), 2, "impl-plan must now have 2 tasks");
    }

    #[test]
    fn add_task_usecase_returns_error_for_missing_impl_plan() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let add_task = AddTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();

        save.execute(&track).unwrap();
        // No impl_plan saved → should return RepositoryError
        let result = add_task.execute(track.id(), "New task", None, None);
        assert!(matches!(result, Err(TrackWriteError::Repository(_))));
    }

    // --- SetOverrideUseCase tests ---

    #[test]
    fn set_override_usecase_blocks_track() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let set_override = SetOverrideUseCase::new(Arc::clone(&store));
        let track = sample_track();

        save.execute(&track).unwrap();
        let updated = set_override
            .execute(track.id(), Some(StatusOverride::blocked("blocker reason").unwrap()))
            .unwrap();

        assert_eq!(updated.status(), TrackStatus::Blocked);
        let loaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded.status(), TrackStatus::Blocked);
    }

    #[test]
    fn set_override_usecase_clears_override() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let set_override = SetOverrideUseCase::new(Arc::clone(&store));
        let track = sample_track();

        save.execute(&track).unwrap();
        set_override.execute(track.id(), Some(StatusOverride::blocked("reason").unwrap())).unwrap();
        let updated = set_override.execute(track.id(), None).unwrap();

        assert_eq!(updated.status(), TrackStatus::Planned);
    }

    #[test]
    fn set_override_usecase_returns_error_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let set_override = SetOverrideUseCase::new(store);
        let track_id = TrackId::try_new("nonexistent-track").unwrap();

        let result =
            set_override.execute(&track_id, Some(StatusOverride::blocked("reason").unwrap()));
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn load_usecase_returns_none_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let load = LoadTrackUseCase::new(store);
        let track_id = TrackId::try_new("nonexistent-track").unwrap();

        let result = load.execute(&track_id).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn save_and_load_round_trip_track() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let load = LoadTrackUseCase::new(Arc::clone(&store));
        let track = sample_track();

        save.execute(&track).unwrap();
        let loaded = load.execute(track.id()).unwrap().unwrap();

        assert_eq!(loaded, track);
    }
}
