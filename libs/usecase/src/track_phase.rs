//! Track phase resolution application service types
//! (TrackPhaseService / TrackPhaseInteractor).
//!
//! Adds the usecase-owned service trait / interactor required by T007 so the
//! CLI never imports `domain::track_phase::TrackPhaseInfo` or
//! `domain::ImplPlanReader` directly (CN-01 / D1).

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use domain::{
    ImplPlanReader, RepositoryError, TrackId, TrackReadError, TrackReader,
    track_phase::resolve_phase,
};

// ── TrackPhaseOutput ──────────────────────────────────────────────────────────

/// DTO returned by the track phase resolution use case.
///
/// Wraps the current phase name, reason, recommended next command, and optional
/// blocker string so that the CLI never imports `domain::track_phase::TrackPhaseInfo`
/// or `domain::ImplPlanReader` directly.
#[derive(Debug)]
pub struct TrackPhaseOutput {
    pub phase: String,
    pub reason: String,
    pub next_command: String,
    pub blocker: Option<String>,
}

// ── TrackPhaseError ───────────────────────────────────────────────────────────

/// Error type for [`TrackPhaseService`].
///
/// Wraps invalid track ID, missing track, and implementation plan load failures
/// without leaking `domain::RepositoryError` across the usecase boundary.
#[derive(Debug, Error)]
pub enum TrackPhaseError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("track not found: {0}")]
    TrackNotFound(String),
    #[error("impl-plan load failed: {0}")]
    ImplPlanLoadFailed(String),
}

impl From<TrackReadError> for TrackPhaseError {
    fn from(e: TrackReadError) -> Self {
        match e {
            TrackReadError::Repository(re) => match re {
                RepositoryError::TrackNotFound(id) => Self::TrackNotFound(id),
                other => Self::ImplPlanLoadFailed(other.to_string()),
            },
        }
    }
}

// ── TrackPhaseService ─────────────────────────────────────────────────────────

/// Application service trait for the track phase resolution use case
/// (`sotp track resolve`).
///
/// Driven by the CLI layer. Takes string `track_id` so the CLI does not need to
/// construct `domain::TrackId`. Returns [`TrackPhaseOutput`] which contains all
/// display information for the resolve command.
pub trait TrackPhaseService: Send + Sync {
    /// Resolves the track phase for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`TrackPhaseError`] on ID validation, not-found, or load failures.
    fn resolve(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<TrackPhaseOutput, TrackPhaseError>;
}

// ── TrackPhaseInteractor ──────────────────────────────────────────────────────

/// Concrete struct implementing [`TrackPhaseService`].
///
/// Follows the same internal generic-storage pattern as the existing
/// `TransitionTaskUseCase`: holds a private `Arc<S>` field where `S` satisfies
/// domain storage traits (`TrackReader + ImplPlanReader`) as an implementation
/// detail. CLI composition root wires `FsTrackStore` as `S` and injects the
/// result as `Arc<dyn TrackPhaseService>`, so the generic bound never crosses
/// the usecase→CLI boundary (CN-01 satisfied).
///
/// Constructs `domain::TrackId` internally, calls `domain::track_phase::resolve_phase`,
/// and returns [`TrackPhaseOutput`].
pub struct TrackPhaseInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    store: Arc<S>,
}

impl<S> TrackPhaseInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    /// Creates a new interactor.
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }
}

impl<S> TrackPhaseService for TrackPhaseInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    fn resolve(
        &self,
        track_id: String,
        _items_dir: PathBuf,
    ) -> Result<TrackPhaseOutput, TrackPhaseError> {
        let id = TrackId::try_new(&track_id)
            .map_err(|e| TrackPhaseError::InvalidTrackId(e.to_string()))?;

        let track = self
            .store
            .find(&id)
            .map_err(TrackPhaseError::from)?
            .ok_or_else(|| TrackPhaseError::TrackNotFound(track_id.clone()))?;

        let impl_plan = self
            .store
            .load_impl_plan(&id)
            .map_err(|e| TrackPhaseError::ImplPlanLoadFailed(e.to_string()))?;

        let info = resolve_phase(&track, impl_plan.as_ref());

        Ok(TrackPhaseOutput {
            phase: info.phase.to_string(),
            reason: info.reason.clone(),
            next_command: info.next_command.to_string(),
            blocker: info.blocker.clone(),
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use domain::{
        ImplPlanDocument, ImplPlanReader, PlanSection, PlanView, RepositoryError, TaskId, TrackId,
        TrackMetadata, TrackReadError, TrackReader,
    };

    use super::*;

    #[derive(Default)]
    struct StubStore {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
        impl_plans: Mutex<HashMap<TrackId, ImplPlanDocument>>,
    }

    impl TrackReader for StubStore {
        fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
            Ok(self.tracks.lock().unwrap().get(id).cloned())
        }
    }

    impl ImplPlanReader for StubStore {
        fn load_impl_plan(
            &self,
            id: &TrackId,
        ) -> Result<Option<ImplPlanDocument>, RepositoryError> {
            Ok(self.impl_plans.lock().unwrap().get(id).cloned())
        }
    }

    const TRACK_ID: &str = "phase-test-track";

    fn sample_track() -> TrackMetadata {
        TrackMetadata::new(TrackId::try_new(TRACK_ID).unwrap(), "Phase Test", None).unwrap()
    }

    fn sample_plan_all_done() -> ImplPlanDocument {
        use domain::{TaskStatus, TrackTask};
        let task = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "done",
            TaskStatus::DonePending,
        )
        .unwrap();
        let section =
            PlanSection::new("S1", "Section", vec![], vec![TaskId::try_new("T001").unwrap()])
                .unwrap();
        ImplPlanDocument::new(vec![task], PlanView::new(vec![], vec![section])).unwrap()
    }

    #[test]
    fn track_phase_interactor_resolve_returns_output_for_known_track() {
        let store = Arc::new(StubStore::default());
        let track = sample_track();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());

        let interactor = TrackPhaseInteractor::new(Arc::clone(&store));
        let out = interactor.resolve(TRACK_ID.to_owned(), PathBuf::new()).unwrap();
        // No impl-plan → Planned phase
        assert!(!out.phase.is_empty());
        assert!(!out.next_command.is_empty());
    }

    #[test]
    fn track_phase_interactor_resolve_returns_ready_to_ship_when_all_done() {
        let store = Arc::new(StubStore::default());
        let track = sample_track();
        let plan = sample_plan_all_done();
        store.tracks.lock().unwrap().insert(track.id().clone(), track.clone());
        store.impl_plans.lock().unwrap().insert(track.id().clone(), plan);

        let interactor = TrackPhaseInteractor::new(Arc::clone(&store));
        let out = interactor.resolve(TRACK_ID.to_owned(), PathBuf::new()).unwrap();
        assert_eq!(out.phase, "Ready to Ship");
    }

    #[test]
    fn track_phase_interactor_resolve_returns_not_found_for_unknown_track() {
        let store = Arc::new(StubStore::default());
        let interactor = TrackPhaseInteractor::new(Arc::clone(&store));
        let err = interactor.resolve("nonexistent".to_owned(), PathBuf::new()).unwrap_err();
        assert!(matches!(err, TrackPhaseError::TrackNotFound(_)));
    }

    #[test]
    fn track_phase_interactor_resolve_returns_invalid_id_for_empty_string() {
        let store = Arc::new(StubStore::default());
        let interactor = TrackPhaseInteractor::new(Arc::clone(&store));
        let err = interactor.resolve(String::new(), PathBuf::new()).unwrap_err();
        assert!(matches!(err, TrackPhaseError::InvalidTrackId(_)));
    }
}
