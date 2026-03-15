//! Use case layer for the SoTOHE-core track state machine.

pub mod git_workflow;
pub mod hook;
pub mod pr_workflow;
pub mod review_workflow;
pub mod track_activation;
pub mod track_phase;
pub mod track_resolution;
pub mod worktree_guard;

use std::sync::Arc;

use domain::{
    CommitHash, TaskId, TaskTransition, TrackId, TrackMetadata, TrackReadError, TrackReader,
    TrackWriteError, TrackWriter, TransitionError, ValidationError,
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

    use super::{LoadTrackUseCase, SaveTrackUseCase, TransitionTaskUseCase};

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

    #[test]
    fn load_usecase_returns_none_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let load = LoadTrackUseCase::new(store);
        let track_id = TrackId::new("nonexistent-track").unwrap();

        let result = load.execute(&track_id).unwrap();

        assert!(result.is_none());
    }
}
