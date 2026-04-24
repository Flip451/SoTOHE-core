#![forbid(unsafe_code)]
//! Use case layer for the SoTOHE-core track state machine.

pub mod catalogue_spec_refs;
pub mod catalogue_spec_signals;
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
    CommitHash, ImplPlanReader, ImplPlanWriter, RepositoryError, StatusOverride, TaskId,
    TaskTransition, TrackId, TrackMetadata, TrackReadError, TrackReader, TrackWriteError,
    TrackWriter, ValidationError,
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

/// Applies a state transition to a task within a track and persists the result.
///
/// Reads `impl-plan.json` via [`ImplPlanReader`], applies the transition on the
/// `ImplPlanDocument` aggregate, persists via [`ImplPlanWriter`].
/// Returns the (unchanged) [`TrackMetadata`] from the initial read.
///
/// `metadata.json` is **not** written — track status is derived on demand from
/// `impl-plan.json` via `domain::derive_track_status`. This eliminates the
/// two-file non-atomic write (impl-plan.json + metadata.json) that was flagged
/// in PR #107.
pub struct TransitionTaskUseCase<S>
where
    S: TrackReader + ImplPlanReader + ImplPlanWriter,
{
    store: Arc<S>,
}

impl<S> TransitionTaskUseCase<S>
where
    S: TrackReader + ImplPlanReader + ImplPlanWriter,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Transitions a task by an explicit [`TaskTransition`] and persists the result.
    ///
    /// Only `impl-plan.json` is written. `metadata.json` is not touched.
    ///
    /// # Concurrency note
    /// This performs a non-serialized read-modify-write on `impl-plan.json`. Concurrent
    /// callers operating against the same track directory are not supported — the CLI is
    /// designed for sequential single-process execution, consistent with
    /// `FsTrackStore::with_locked_document`'s documented assumption that "concurrent
    /// callers are not supported — parallel access will be handled by worktree isolation".
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
        // Verify the track exists.
        let track = self.store.find(track_id).map_err(TrackWriteError::from)?.ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(track_id.to_string()))
        })?;

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

        // Persist impl-plan.json ONLY (single-file atomic write).
        self.store.save_impl_plan(track_id, &impl_plan).map_err(TrackWriteError::from)?;

        // Return the (unchanged) metadata — callers derive status on demand.
        Ok(track)
    }

    /// Resolves a target status string to the correct transition and applies it.
    ///
    /// Only `impl-plan.json` is written. `metadata.json` is not touched.
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
        // Verify the track exists.
        let track = self.store.find(track_id).map_err(TrackWriteError::from)?.ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(track_id.to_string()))
        })?;

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

        // Persist impl-plan.json ONLY (single-file atomic write).
        self.store.save_impl_plan(track_id, &impl_plan).map_err(TrackWriteError::from)?;

        // Return the (unchanged) metadata — callers derive status on demand.
        Ok(track)
    }
}

/// Adds a new task to a track's `impl-plan.json` and persists the result.
///
/// Reads `impl-plan.json` via [`ImplPlanReader`], delegates to
/// [`domain::ImplPlanDocument::add_task`], persists via [`ImplPlanWriter`].
/// Returns the (unchanged) [`TrackMetadata`] and the newly-allocated [`TaskId`].
///
/// `metadata.json` is **not** written — track status is derived on demand from
/// `impl-plan.json` via `domain::derive_track_status`.
pub struct AddTaskUseCase<S>
where
    S: TrackReader + ImplPlanReader + ImplPlanWriter,
{
    store: Arc<S>,
}

impl<S> AddTaskUseCase<S>
where
    S: TrackReader + ImplPlanReader + ImplPlanWriter,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Adds a task to the track and persists the result.
    ///
    /// Only `impl-plan.json` is written. `metadata.json` is not touched.
    ///
    /// # Concurrency note
    /// This performs a non-serialized read-modify-write on `impl-plan.json`. See
    /// [`TransitionTaskUseCase::execute`] for the documented single-process assumption.
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

        // Verify the track exists.
        let track = self.store.find(track_id).map_err(TrackWriteError::from)?.ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(track_id.to_string()))
        })?;

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

        // Persist impl-plan.json ONLY (single-file atomic write).
        self.store.save_impl_plan(track_id, &impl_plan).map_err(TrackWriteError::from)?;

        // Return the (unchanged) metadata — callers derive status on demand.
        Ok((track, new_task_id))
    }
}

/// Sets or clears a status override on a track and persists the result atomically.
///
/// This is a genuine identity mutation (not a derived-status sync): it updates
/// `status_override` in `metadata.json`. Track status is derived on demand
/// from `impl-plan.json` + `status_override` by callers.
pub struct SetOverrideUseCase<S>
where
    S: TrackWriter,
{
    store: Arc<S>,
}

impl<S> SetOverrideUseCase<S>
where
    S: TrackWriter,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Sets or clears a status override on the track and persists atomically.
    ///
    /// * `status_override = Some(ov)` — sets the override (Blocked/Cancelled + reason).
    /// * `status_override = None` — clears the override.
    ///
    /// Only `metadata.json` is written. `impl-plan.json` is not touched.
    ///
    /// # Errors
    /// Returns `TrackWriteError` if the track is not found or the underlying writer fails.
    pub fn execute(
        &self,
        track_id: &TrackId,
        status_override: Option<StatusOverride>,
    ) -> Result<TrackMetadata, TrackWriteError> {
        self.store.update(track_id, |track| {
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
        TrackWriter, ValidationError, derive_track_status,
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
        // Identity-only TrackMetadata; tasks/plan live in impl-plan.json.
        // Status is derived on demand via derive_track_status.
        TrackMetadata::new(
            TrackId::try_new("track-state-machine").unwrap(),
            "Track state machine",
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
    fn transition_usecase_execute_by_status_transitions_task_and_updates_impl_plan_only() {
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

        // Verify the impl-plan task was updated.
        let updated_plan = store.load_impl_plan(track.id()).unwrap().unwrap();
        assert!(
            matches!(updated_plan.tasks()[0].status(), TaskStatus::InProgress),
            "task must be InProgress after transition"
        );

        // Status is derived on demand — metadata.json is NOT written by transition.
        // Verify derived status reflects the updated impl-plan.
        assert_eq!(
            derive_track_status(Some(&updated_plan), None),
            TrackStatus::InProgress,
            "derived status must be InProgress when a task is in_progress"
        );
    }

    #[test]
    fn transition_usecase_execute_with_explicit_transition_updates_impl_plan_only() {
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

        // metadata.json is NOT written — derive status from updated impl-plan.
        let updated_plan = store.load_impl_plan(track.id()).unwrap().unwrap();
        assert_eq!(derive_track_status(Some(&updated_plan), None), TrackStatus::InProgress);
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

        // Status is derived from status_override.
        assert_eq!(derive_track_status(None, updated.status_override()), TrackStatus::Blocked);
        let loaded = store.find(track.id()).unwrap().unwrap();
        assert!(loaded.status_override().is_some());
        assert_eq!(derive_track_status(None, loaded.status_override()), TrackStatus::Blocked);
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

        // Override cleared → derived status from impl-plan (None) + no override = Planned
        assert_eq!(derive_track_status(None, updated.status_override()), TrackStatus::Planned);
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
