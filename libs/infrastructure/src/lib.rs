#![forbid(unsafe_code)]
//! Infrastructure layer for the SoTOHE-core track state machine.

pub mod agent_profiles;
pub mod code_profile_builder;
pub mod gh_cli;
pub mod git_cli;
pub mod guides_codec;
pub mod impl_plan_codec;
pub mod review_v2;
pub mod schema_export;
pub mod schema_export_codec;
#[cfg(test)]
mod schema_export_tests;
pub mod shell;
pub mod spec;
pub mod task_coverage_codec;
pub mod tddd;
pub mod track;
pub mod type_catalogue_render;
pub mod verify;

/// Returns a `Timestamp` for the current UTC instant, truncated to whole seconds.
///
/// Consolidates `chrono::Utc::now()` into a single infrastructure function so that
/// domain/usecase layers receive timestamps as arguments (hexagonal purity).
///
/// # Errors
///
/// Returns `domain::ValidationError` if chrono produces an unparsable string (should never happen).
pub fn timestamp_now() -> Result<domain::Timestamp, domain::ValidationError> {
    use chrono::Timelike as _;
    let now = chrono::Utc::now();
    let dt = now.with_nanosecond(0).unwrap_or(now);
    let raw = dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    domain::Timestamp::new(raw)
}

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
    use domain::{TrackId, TrackMetadata, TrackReader, TrackStatus, TrackWriter};

    use super::InMemoryTrackStore;

    fn sample_track() -> TrackMetadata {
        // T005: TrackMetadata is identity-only; no tasks/plan fields.
        TrackMetadata::new(
            TrackId::try_new("track-state-machine").unwrap(),
            "Track state machine",
            TrackStatus::Planned,
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

        store.save(&track).unwrap();

        let updated = store
            .update(track.id(), |t| {
                t.set_status(TrackStatus::InProgress);
                Ok(())
            })
            .unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);

        let reloaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(reloaded.status(), TrackStatus::InProgress);
    }
}
