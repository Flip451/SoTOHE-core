//! Use case layer for the SoTOHE-core track state machine.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

use std::sync::Arc;

use domain::{
    DomainError, RepositoryError, TaskId, TaskTransition, TrackId, TrackMetadata, TrackRepository,
};

/// Persists a track aggregate to the repository.
pub struct SaveTrackUseCase<R: TrackRepository> {
    repo: Arc<R>,
}

impl<R: TrackRepository> SaveTrackUseCase<R> {
    #[must_use]
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    /// Saves the given track.
    ///
    /// # Errors
    /// Returns `DomainError::Repository` on persistence failure.
    pub fn execute(&self, track: &TrackMetadata) -> Result<(), DomainError> {
        self.repo.save(track).map_err(DomainError::from)
    }
}

/// Loads a track aggregate from the repository by ID.
pub struct LoadTrackUseCase<R: TrackRepository> {
    repo: Arc<R>,
}

impl<R: TrackRepository> LoadTrackUseCase<R> {
    #[must_use]
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    /// Loads a track by ID, returning `None` if not found.
    ///
    /// # Errors
    /// Returns `DomainError::Repository` on persistence failure.
    pub fn execute(&self, id: &TrackId) -> Result<Option<TrackMetadata>, DomainError> {
        self.repo.find(id).map_err(DomainError::from)
    }
}

/// Applies a state transition to a task within a track and persists the result.
pub struct TransitionTaskUseCase<R: TrackRepository> {
    repo: Arc<R>,
}

impl<R: TrackRepository> TransitionTaskUseCase<R> {
    #[must_use]
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    /// Transitions a task and persists the updated track.
    ///
    /// # Errors
    /// Returns `DomainError` if the track is not found, the task is not found,
    /// or the transition is invalid.
    pub fn execute(
        &self,
        track_id: &TrackId,
        task_id: &TaskId,
        transition: TaskTransition,
    ) -> Result<TrackMetadata, DomainError> {
        let mut track = self.repo.find(track_id).map_err(DomainError::from)?.ok_or_else(|| {
            DomainError::from(RepositoryError::TrackNotFound(track_id.to_string()))
        })?;

        track.transition_task(task_id, transition)?;
        self.repo.save(&track).map_err(DomainError::from)?;

        Ok(track)
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use domain::{
        DomainError, PlanSection, PlanView, RepositoryError, TaskId, TaskTransition, TrackId,
        TrackMetadata, TrackRepository, TrackStatus, TrackTask,
    };

    use super::{LoadTrackUseCase, SaveTrackUseCase, TransitionTaskUseCase};

    #[derive(Default)]
    struct StubTrackRepository {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
    }

    impl TrackRepository for StubTrackRepository {
        fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, RepositoryError> {
            let tracks = self
                .tracks
                .lock()
                .map_err(|_| RepositoryError::Message("internal repository error".to_owned()))?;
            Ok(tracks.get(id).cloned())
        }

        fn save(&self, track: &TrackMetadata) -> Result<(), RepositoryError> {
            let mut tracks = self
                .tracks
                .lock()
                .map_err(|_| RepositoryError::Message("internal repository error".to_owned()))?;
            tracks.insert(track.id().clone(), track.clone());
            Ok(())
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
        let repo = Arc::new(StubTrackRepository::default());
        let save = SaveTrackUseCase::new(Arc::clone(&repo));
        let load = LoadTrackUseCase::new(Arc::clone(&repo));
        let track = sample_track();

        save.execute(&track).unwrap();
        let loaded = load.execute(track.id()).unwrap().unwrap();

        assert_eq!(loaded, track);
    }

    #[test]
    fn transition_usecase_persists_updated_track() {
        let repo = Arc::new(StubTrackRepository::default());
        let save = SaveTrackUseCase::new(Arc::clone(&repo));
        let transition = TransitionTaskUseCase::new(Arc::clone(&repo));
        let track = sample_track();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        let updated = transition.execute(track.id(), &task_id, TaskTransition::Start).unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);
        assert_eq!(repo.find(track.id()).unwrap().unwrap().status(), TrackStatus::InProgress);
    }

    #[test]
    fn transition_usecase_returns_error_for_missing_track() {
        let repo = Arc::new(StubTrackRepository::default());
        let transition = TransitionTaskUseCase::new(repo);
        let track_id = TrackId::new("nonexistent-track").unwrap();
        let task_id = TaskId::new("T1").unwrap();

        let result = transition.execute(&track_id, &task_id, TaskTransition::Start);

        assert!(matches!(result, Err(DomainError::Repository(RepositoryError::TrackNotFound(_)))));
    }

    #[test]
    fn load_usecase_returns_none_for_missing_track() {
        let repo = Arc::new(StubTrackRepository::default());
        let load = LoadTrackUseCase::new(repo);
        let track_id = TrackId::new("nonexistent-track").unwrap();

        let result = load.execute(&track_id).unwrap();

        assert!(result.is_none());
    }
}
