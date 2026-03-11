//! Infrastructure layer for the SoTOHE-core track state machine.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

pub mod lock;

use std::collections::HashMap;
use std::sync::Mutex;

use domain::{RepositoryError, TrackId, TrackMetadata, TrackRepository};

/// In-memory implementation of `TrackRepository` backed by `Mutex<HashMap>`.
#[derive(Default)]
pub struct InMemoryTrackRepository {
    tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
}

impl InMemoryTrackRepository {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TrackRepository for InMemoryTrackRepository {
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

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::{
        PlanSection, PlanView, TaskId, TrackId, TrackMetadata, TrackRepository, TrackTask,
    };

    use super::InMemoryTrackRepository;

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
    fn repository_returns_saved_track() {
        let repo = InMemoryTrackRepository::new();
        let track = sample_track();

        repo.save(&track).unwrap();

        let loaded = repo.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded, track);
    }
}
