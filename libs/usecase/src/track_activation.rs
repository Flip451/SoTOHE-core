use std::sync::Arc;

use domain::{
    ImplPlanReader, TrackBranch, TrackId, TrackMetadata, TrackStatus, TrackWriteError, TrackWriter,
    ValidationError, derive_track_status,
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

pub struct ActivateTrackUseCase<S>
where
    S: TrackWriter + ImplPlanReader,
{
    store: Arc<S>,
}

impl<S> ActivateTrackUseCase<S>
where
    S: TrackWriter + ImplPlanReader,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    pub fn execute(
        &self,
        track_id: &TrackId,
        branch: &TrackBranch,
        schema_version: u32,
    ) -> Result<ActivateTrackOutcome, TrackWriteError> {
        // Load impl-plan.json (if present) BEFORE the writer transaction so that
        // `derive_track_status` sees the real task state. A branchless planning
        // track with a populated impl-plan.json whose tasks are already
        // `in_progress` or `done` must NOT be re-materialised; passing `None`
        // here would misclassify such a track as `Planned` and bypass the
        // planning-only activation precondition.
        let impl_plan = self.store.load_impl_plan(track_id).map_err(TrackWriteError::from)?;

        let updated = self.store.update(track_id, |track| {
            if let Some(existing) = track.branch() {
                return Err(ValidationError::TrackAlreadyMaterialized {
                    track_id: track.id().to_string(),
                    branch: existing.to_string(),
                }
                .into());
            }

            // Schema versions 4 and 5 are the identity-only shapes (v4 has status,
            // v5 removes it). Accept 3, 4, or 5. The error variant name is kept
            // as-is for compatibility; the display message covers all versions.
            if !matches!(schema_version, 3..=5) {
                return Err(ValidationError::TrackActivationRequiresSchemaV3 {
                    track_id: track.id().to_string(),
                    schema_version,
                }
                .into());
            }

            // Validate activation precondition against the REAL derived status:
            // derive from impl_plan + status_override (not `None` + override),
            // so in_progress/done tasks in impl-plan.json block activation.
            let derived = derive_track_status(impl_plan.as_ref(), track.status_override());
            if derived != TrackStatus::Planned {
                return Err(ValidationError::TrackActivationRequiresPlanningOnly {
                    track_id: track.id().to_string(),
                    status: derived,
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
        DomainError, ImplPlanDocument, ImplPlanReader, RepositoryError, StatusOverride,
        TrackBranch, TrackId, TrackMetadata, TrackReader, TrackWriteError, TrackWriter,
        ValidationError,
    };

    use super::{ActivateTrackOutcome, ActivateTrackUseCase};

    #[derive(Default)]
    struct StubTrackStore {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
        impl_plans: Mutex<HashMap<TrackId, ImplPlanDocument>>,
    }

    impl StubTrackStore {
        fn set_impl_plan(&self, id: &TrackId, doc: ImplPlanDocument) {
            self.impl_plans.lock().unwrap().insert(id.clone(), doc);
        }
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
        // TrackMetadata is identity-only; status derived from impl-plan + override.
        TrackMetadata::new(TrackId::try_new("activation-track").unwrap(), "Activation Track", None)
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
    fn activation_rejects_branchless_track_with_in_progress_impl_plan() {
        // A branchless planning-track with an already-populated impl-plan.json
        // (e.g. tasks in `in_progress` or `done`) must NOT be activatable.
        // `derive_track_status(Some(impl_plan), None)` returns `InProgress` /
        // `Done`, which breaks the planning-only activation precondition.
        use domain::{ImplPlanDocument, PlanSection, PlanView, TaskId, TaskStatus, TrackTask};

        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        // Construct impl-plan with one in-progress task so derive returns InProgress.
        // The task must be referenced in a PlanSection — ImplPlanDocument::new enforces
        // referential integrity (every task must appear in exactly one section).
        let task = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "work in progress",
            TaskStatus::InProgress,
        )
        .unwrap();
        let section = PlanSection::new(
            "S1",
            "Implementation",
            vec![],
            vec![TaskId::try_new("T001").unwrap()],
        )
        .unwrap();
        let impl_plan =
            ImplPlanDocument::new(vec![task], PlanView::new(vec![], vec![section])).unwrap();

        store.save(&track).unwrap();
        store.set_impl_plan(track.id(), impl_plan);

        let err = usecase.execute(track.id(), &branch, 5).unwrap_err();
        assert!(
            matches!(
                err,
                TrackWriteError::Domain(domain::DomainError::Validation(
                    ValidationError::TrackActivationRequiresPlanningOnly { .. }
                ))
            ),
            "expected TrackActivationRequiresPlanningOnly, got {err:?}"
        );
    }

    #[test]
    fn activation_rejects_non_planning_only_track() {
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let mut track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        // Status is derived; set status_override to Blocked to simulate
        // a track that is not planning-only.
        track.set_status_override(Some(StatusOverride::blocked("testing").unwrap()));
        store.save(&track).unwrap();
        let err = usecase.execute(track.id(), &branch, 5).unwrap_err();

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
        // Schema version 4 is the identity-only shape (without derived-status
        // semantics). Activation must accept v4 tracks so that /track:activate
        // works for newly created tracks.
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        store.save(&track).unwrap();
        let outcome = usecase.execute(track.id(), &branch, 4).unwrap();

        assert!(matches!(outcome, ActivateTrackOutcome::Materialized(_)));
        assert_eq!(outcome.track().branch().unwrap(), &branch);
    }

    #[test]
    fn activation_accepts_schema_version_5_derived_status_track() {
        // Schema version 5 removes the status field; activation must accept
        // v5 tracks (the canonical current format).
        let store = Arc::new(StubTrackStore::default());
        let usecase = ActivateTrackUseCase::new(Arc::clone(&store));
        let track = sample_track();
        let branch = TrackBranch::try_new("track/activation-track").unwrap();

        store.save(&track).unwrap();
        let outcome = usecase.execute(track.id(), &branch, 5).unwrap();

        assert!(matches!(outcome, ActivateTrackOutcome::Materialized(_)));
        assert_eq!(outcome.track().branch().unwrap(), &branch);
    }
}
