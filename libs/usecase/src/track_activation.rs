use std::sync::Arc;

use domain::{
    TrackBranch, TrackId, TrackMetadata, TrackStatus, TrackWriteError, TrackWriter, ValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivateTrackOutcome {
    Materialized(TrackMetadata),
}

impl ActivateTrackOutcome {
    #[must_use]
    pub fn track(&self) -> &TrackMetadata {
        match self {
            Self::Materialized(track) => track,
        }
    }
}

pub struct ActivateTrackUseCase<W: TrackWriter> {
    writer: Arc<W>,
}

impl<W: TrackWriter> ActivateTrackUseCase<W> {
    #[must_use]
    pub fn new(writer: Arc<W>) -> Self {
        Self { writer }
    }

    pub fn execute(
        &self,
        track_id: &TrackId,
        branch: &TrackBranch,
        schema_version: u32,
    ) -> Result<ActivateTrackOutcome, TrackWriteError> {
        let updated = self.writer.update(track_id, |track| {
            if let Some(existing) = track.branch() {
                return Err(ValidationError::TrackAlreadyMaterialized {
                    track_id: track.id().to_string(),
                    branch: existing.to_string(),
                }
                .into());
            }

            // T005: schema_version 4 is the new identity-only shape. Accept both 3 and 4.
            // The error variant name is kept as-is for compatibility; the display message
            // covers both versions.
            if !matches!(schema_version, 3 | 4) {
                return Err(ValidationError::TrackActivationRequiresSchemaV3 {
                    track_id: track.id().to_string(),
                    schema_version,
                }
                .into());
            }

            if track.status() != TrackStatus::Planned {
                return Err(ValidationError::TrackActivationRequiresPlanningOnly {
                    track_id: track.id().to_string(),
                    status: track.status(),
                }
                .into());
            }

            track.set_branch(Some(branch.clone()))?;
            Ok(())
        })?;

        Ok(ActivateTrackOutcome::Materialized(updated))
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use domain::{
        DomainError, RepositoryError, TrackBranch, TrackId, TrackMetadata, TrackReader,
        TrackStatus, TrackWriteError, TrackWriter, ValidationError,
    };

    use super::{ActivateTrackOutcome, ActivateTrackUseCase};

    #[derive(Default)]
    struct StubTrackStore {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
    }

    impl TrackReader for StubTrackStore {
        fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, domain::TrackReadError> {
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
        // T005: TrackMetadata is identity-only; tasks/plan live in impl-plan.json.
        TrackMetadata::new(
            TrackId::try_new("activation-track").unwrap(),
            "Activation Track",
            TrackStatus::Planned,
            None,
        )
        .unwrap()
    }

    #[test]
    fn activation_materializes_branch_for_planning_only_track() {
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        store.save(&track).unwrap();
        let outcome = usecase.execute(track.id(), &branch, 3).unwrap();

        assert!(matches!(outcome, ActivateTrackOutcome::Materialized(_)));
        assert_eq!(outcome.track().branch().unwrap(), &branch);
    }

    #[test]
    fn activation_rejects_already_materialized_track() {
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let branch = TrackBranch::try_new("track/activation-track").unwrap();
        let mut track = sample_track();
        track.set_branch(Some(branch.clone())).unwrap();

        store.save(&track).unwrap();
        let err = usecase.execute(track.id(), &branch, 3).unwrap_err();

        assert!(matches!(
            err,
            TrackWriteError::Domain(domain::DomainError::Validation(
                ValidationError::TrackAlreadyMaterialized { .. }
            ))
        ));
        assert!(err.to_string().contains("already materialized"));
    }

    #[test]
    fn activation_rejects_non_planning_only_track() {
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let mut track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        // T005: status is now explicitly stored; set it to InProgress to simulate
        // a track that is not planning-only.
        track.set_status(TrackStatus::InProgress);
        store.save(&track).unwrap();
        let err = usecase.execute(track.id(), &branch, 3).unwrap_err();

        assert!(matches!(
            err,
            TrackWriteError::Domain(domain::DomainError::Validation(
                ValidationError::TrackActivationRequiresPlanningOnly { .. }
            ))
        ));
        assert!(err.to_string().contains("not planning-only"));
    }

    #[test]
    fn activation_rejects_legacy_v2_branchless_track() {
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        store.save(&track).unwrap();
        let err = usecase.execute(track.id(), &branch, 2).unwrap_err();

        assert!(matches!(
            err,
            TrackWriteError::Domain(domain::DomainError::Validation(
                ValidationError::TrackActivationRequiresSchemaV3 { .. }
            ))
        ));
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn activation_accepts_schema_version_4_identity_only_track() {
        // T005: schema_version 4 is the new identity-only shape. Activation must
        // accept v4 tracks so that /track:activate works for newly created tracks.
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        store.save(&track).unwrap();
        let outcome = usecase.execute(track.id(), &branch, 4).unwrap();

        assert!(matches!(outcome, ActivateTrackOutcome::Materialized(_)));
        assert_eq!(outcome.track().branch().unwrap(), &branch);
    }
}
