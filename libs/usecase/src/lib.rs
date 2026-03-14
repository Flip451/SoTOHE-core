//! Use case layer for the SoTOHE-core track state machine.

pub mod git_workflow;
pub mod hook;
pub mod pr_workflow;
pub mod review_workflow;
pub mod track_activation;

use std::sync::Arc;

use domain::{
    TaskId, TaskTransition, TrackId, TrackMetadata, TrackReadError, TrackReader, TrackWriteError,
    TrackWriter,
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
    fn load_usecase_returns_none_for_missing_track() {
        let store = Arc::new(StubTrackStore::default());
        let load = LoadTrackUseCase::new(store);
        let track_id = TrackId::new("nonexistent-track").unwrap();

        let result = load.execute(&track_id).unwrap();

        assert!(result.is_none());
    }
}
