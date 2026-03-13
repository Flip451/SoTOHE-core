//! Infrastructure layer for the SoTOHE-core track state machine.

pub mod gh_cli;
pub mod git_cli;
pub mod lock;
pub mod track;

use std::collections::HashMap;
use std::sync::Mutex;

use domain::{
    DomainError, RepositoryError, TrackId, TrackMetadata, TrackReadError, TrackReader,
    TrackWriteError, TrackWriter,
};

/// In-memory implementation of `TrackReader` + `TrackWriter` for testing.
#[derive(Default)]
pub struct InMemoryTrackStore {
    tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
}

impl InMemoryTrackStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TrackReader for InMemoryTrackStore {
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
        let tracks = self
            .tracks
            .lock()
            .map_err(|_| RepositoryError::Message("internal repository error".to_owned()))?;
        Ok(tracks.get(id).cloned())
    }
}

impl TrackWriter for InMemoryTrackStore {
    fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
        let mut tracks = self
            .tracks
            .lock()
            .map_err(|_| RepositoryError::Message("internal repository error".to_owned()))?;
        tracks.insert(track.id().clone(), track.clone());
        Ok(())
    }

    fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>,
    {
        let mut tracks = self.tracks.lock().map_err(|_| {
            TrackWriteError::Repository(RepositoryError::Message(
                "internal repository error".to_owned(),
            ))
        })?;
        let track = tracks.get_mut(id).ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
        })?;
        mutate(track).map_err(TrackWriteError::from)?;
        Ok(track.clone())
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::{
        PlanSection, PlanView, TaskId, TaskTransition, TrackId, TrackMetadata, TrackReader,
        TrackStatus, TrackTask, TrackWriter,
    };

    use super::InMemoryTrackStore;

    fn sample_track() -> TrackMetadata {
        let task_id = TaskId::new("T1").unwrap();
        let task = TrackTask::new(task_id.clone(), "Persist the track aggregate").unwrap();
        let section = PlanSection::new("S1", "Persistence", Vec::new(), vec![task_id]).unwrap();
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
    fn store_returns_saved_track() {
        let store = InMemoryTrackStore::new();
        let track = sample_track();

        store.save(&track).unwrap();

        let loaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded, track);
    }

    #[test]
    fn update_atomically_mutates_and_persists() {
        let store = InMemoryTrackStore::new();
        let track = sample_track();
        let task_id = TaskId::new("T1").unwrap();

        store.save(&track).unwrap();

        let updated = store
            .update(track.id(), |t| {
                t.transition_task(&task_id, TaskTransition::Start)?;
                Ok(())
            })
            .unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);

        let reloaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(reloaded.status(), TrackStatus::InProgress);
    }
}
