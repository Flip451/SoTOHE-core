//! Use case layer for the SoTOHE-core track state machine.

pub mod git_workflow;
pub mod hook;
pub mod pr_review;
pub mod pr_workflow;
pub mod review_workflow;
pub mod track_activation;
pub mod track_phase;
pub mod track_resolution;
pub mod worktree_guard;

use std::sync::Arc;

use domain::{
    CommitHash, StatusOverride, TaskId, TaskTransition, TrackId, TrackMetadata, TrackReadError,
    TrackReader, TrackWriteError, TrackWriter, TransitionError, ValidationError,
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
/// Uses `TrackWriter::update` for atomic read-modify-write (no find/save race).
pub struct TransitionTaskUseCase<W: TrackWriter> {
    writer: Arc<W>,
}

impl<W: TrackWriter> TransitionTaskUseCase<W> {
    #[must_use]
    pub fn new(writer: Arc<W>) -> Self {
        Self { writer }
    }

    /// Transitions a task and persists the updated track atomically.
    ///
    /// # Errors
    /// Returns `TrackWriteError` if the track is not found, the task is not found,
    /// or the transition is invalid.
    pub fn execute(
        &self,
        track_id: &TrackId,
        task_id: &TaskId,
        transition: TaskTransition,
    ) -> Result<TrackMetadata, TrackWriteError> {
        self.writer.update(track_id, |track| {
            track.transition_task(task_id, transition)?;
            Ok(())
        })
    }

    /// Resolves a target status string to the correct transition and applies it.
    ///
    /// This is a higher-level entry point that encapsulates the
    /// task-lookup → resolve-transition → transition-task flow so that the CLI
    /// only needs to pass the raw `target_status` string and optional commit hash.
    ///
    /// # Errors
    /// Returns `TrackWriteError` if the track/task is not found, the target status
    /// is unrecognised, or the transition is invalid for the current task state.
    pub fn execute_by_status(
        &self,
        track_id: &TrackId,
        task_id: &TaskId,
        target_status: &str,
        commit_hash: Option<CommitHash>,
    ) -> Result<TrackMetadata, TrackWriteError> {
        self.writer.update(track_id, |track| {
            let task =
                track.tasks().iter().find(|t| *t.id() == *task_id).ok_or_else(|| {
                    TransitionError::TaskNotFound { task_id: task_id.to_string() }
                })?;
            let current_kind = task.status().kind();

            let transition =
                track_resolution::resolve_transition(target_status, current_kind, commit_hash)
                    .map_err(|e| match e {
                        track_resolution::TrackResolutionError::UnsupportedTargetStatus(s) => {
                            ValidationError::UnsupportedTargetStatus(s)
                        }
                        other => ValidationError::UnsupportedTargetStatus(other.to_string()),
                    })?;

            track.transition_task(task_id, transition)?;
            Ok(())
        })
    }
}

/// Adds a new task to a track and persists the result atomically.
pub struct AddTaskUseCase<W: TrackWriter> {
    writer: Arc<W>,
}

impl<W: TrackWriter> AddTaskUseCase<W> {
    #[must_use]
    pub fn new(writer: Arc<W>) -> Self {
        Self { writer }
    }

    /// Adds a task to the track and persists atomically.
    ///
    /// # Errors
    /// Returns `TrackWriteError` if the track is not found, description is empty,
    /// or the target section does not exist.
    pub fn execute(
        &self,
        track_id: &TrackId,
        description: &str,
        section_id: Option<&str>,
        after_task_id: Option<&TaskId>,
    ) -> Result<(TrackMetadata, TaskId), TrackWriteError> {
        let captured_id = std::cell::Cell::new(None);
        let track = self.writer.update(track_id, |track| {
            let tid = track.add_task(description, section_id, after_task_id)?;
            captured_id.set(Some(tid));
            Ok(())
        })?;
        let tid = captured_id
            .into_inner()
            .ok_or_else(|| TrackWriteError::Domain(ValidationError::EmptyTaskDescription.into()))?;
        Ok((track, tid))
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
    /// Returns `TrackWriteError` if the track is not found or override is incompatible.
    pub fn execute(
        &self,
        track_id: &TrackId,
        status_override: Option<StatusOverride>,
    ) -> Result<TrackMetadata, TrackWriteError> {
        self.writer.update(track_id, |track| {
            track.set_status_override(status_override)?;
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
        DomainError, PlanSection, PlanView, RepositoryError, TaskId, TaskTransition, TrackId,
        TrackMetadata, TrackReadError, TrackReader, TrackStatus, TrackTask, TrackWriteError,
        TrackWriter,
    };

    use domain::StatusOverride;

    use super::{
        AddTaskUseCase, LoadTrackUseCase, SaveTrackUseCase, SetOverrideUseCase,
        TransitionTaskUseCase,
    };

    #[derive(Default)]
    struct StubTrackStore {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
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

    fn sample_track() -> TrackMetadata {
        let task_id = TaskId::new("T1").unwrap();
        let task = TrackTask::new(task_id.clone(), "Implement the domain aggregate").unwrap();
        let section = PlanSection::new("S1", "Domain", Vec::new(), vec![task_id]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);

        TrackMetadata::new(
            TrackId::new("track-state-machine").unwrap(),
            "Track state machine",
            vec![task],
            plan,
            None,
        )
        .unwrap()
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

    #[test]
    fn transition_usecase_persists_updated_track() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        let updated = transition.execute(track.id(), &task_id, TaskTransition::Start).unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);
        assert_eq!(store.find(track.id()).unwrap().unwrap().status(), TrackStatus::InProgress);
    }

    #[test]
    fn transition_usecase_returns_error_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let transition = TransitionTaskUseCase::new(store);
        let track_id = TrackId::new("nonexistent-track").unwrap();
        let task_id = TaskId::new("T1").unwrap();

        let result = transition.execute(&track_id, &task_id, TaskTransition::Start);

        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn execute_by_status_resolves_and_transitions() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        let updated =
            transition.execute_by_status(track.id(), &task_id, "in_progress", None).unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);
    }

    #[test]
    fn execute_by_status_rejects_unsupported_status() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        let err = transition.execute_by_status(track.id(), &task_id, "invalid", None).unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("unsupported target status"),
            "expected 'unsupported target status' in: {msg}"
        );
        assert!(msg.contains("invalid"), "expected 'invalid' in: {msg}");
        // Verify no double-prefixing
        assert_eq!(msg.matches("unsupported target status").count(), 1, "double-prefix in: {msg}");
    }

    #[test]
    fn execute_by_status_reopens_done_task() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        transition.execute_by_status(track.id(), &task_id, "in_progress", None).unwrap();
        transition.execute_by_status(track.id(), &task_id, "done", None).unwrap();
        let updated =
            transition.execute_by_status(track.id(), &task_id, "in_progress", None).unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);
    }

    // --- AddTaskUseCase tests ---

    #[test]
    fn add_task_usecase_appends_task_and_persists() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let add_task = AddTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();

        save.execute(&track).unwrap();
        let (updated, new_id) =
            add_task.execute(track.id(), "New task from usecase", None, None).unwrap();

        assert_eq!(new_id.as_str(), "T002");
        assert_eq!(updated.tasks().len(), 2);
        assert_eq!(updated.tasks()[1].description(), "New task from usecase");
        // Verify persistence
        let loaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded.tasks().len(), 2);
    }

    #[test]
    fn add_task_usecase_returns_error_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let add_task = AddTaskUseCase::new(store);
        let track_id = TrackId::new("nonexistent-track").unwrap();

        let result = add_task.execute(&track_id, "Some task", None, None);
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
            .execute(track.id(), Some(StatusOverride::blocked("blocker reason")))
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
        set_override.execute(track.id(), Some(StatusOverride::blocked("reason"))).unwrap();
        let updated = set_override.execute(track.id(), None).unwrap();

        assert_eq!(updated.status(), TrackStatus::Planned);
    }

    #[test]
    fn set_override_usecase_returns_error_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let set_override = SetOverrideUseCase::new(store);
        let track_id = TrackId::new("nonexistent-track").unwrap();

        let result = set_override.execute(&track_id, Some(StatusOverride::blocked("reason")));
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn add_task_usecase_returns_error_for_unknown_section() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let add_task = AddTaskUseCase::new(Arc::clone(&store));
        let track = sample_track();

        save.execute(&track).unwrap();
        let result = add_task.execute(track.id(), "Some task", Some("NONEXISTENT"), None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not found"), "expected 'not found' in: {msg}");
    }

    #[test]
    fn set_override_usecase_rejects_override_on_resolved_track() {
        let store = Arc::new(StubTrackStore::default());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let set_override = SetOverrideUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        // Move task to done to make all tasks resolved
        transition.execute(track.id(), &task_id, TaskTransition::Start).unwrap();
        transition
            .execute(track.id(), &task_id, TaskTransition::Complete { commit_hash: None })
            .unwrap();

        let result = set_override.execute(track.id(), Some(StatusOverride::blocked("reason")));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("incompatible"), "expected 'incompatible' in: {msg}");
    }

    #[test]
    fn load_usecase_returns_none_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let load = LoadTrackUseCase::new(store);
        let track_id = TrackId::new("nonexistent-track").unwrap();

        let result = load.execute(&track_id).unwrap();

        assert!(result.is_none());
    }
}
