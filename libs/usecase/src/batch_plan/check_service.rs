//! The Phase 3 gate as an application service (spec `IN-06`, `IN-07`, `AC-05`,
//! `AC-06`, `AC-07`, `AC-08`, `CN-09`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the
//! workspace module-size limit.

use std::sync::Arc;

use domain::batch_plan::check_batch_plan;
use domain::{FreeText, TrackId};
use thiserror::Error;

use super::output::map_outcome;
use super::{
    BatchPlanCheckCommand, BatchPlanCheckOutput, BatchPlanReaderPort, PlanArtifactReadError,
    PlannedTaskReaderPort, ScopeConfigReadError, ScopeConfigReaderPort,
};
use crate::track_resolution::ActiveTrackResolveService;

// ── BatchPlanCheckError ───────────────────────────────────────────────────────

/// A failure that prevented the Phase 3 gate from running (`AC-05`, `AC-06`,
/// `CN-09`).
///
/// Kept apart from the verdict: structural findings are data inside
/// [`BatchPlanCheckOutput::Blocked`], so an adapter can tell "the plan does not
/// conform" from "the check could not be made".
#[derive(Debug, Error)]
pub enum BatchPlanCheckError {
    /// The active track declares no batch plan — an error, not a pass.
    #[error("the track declares no batch plan")]
    BatchPlanNotFound,
    /// The batch plan could not be read.
    #[error("the batch plan could not be read: {message}")]
    BatchPlanReadFailed {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
    /// The track declares no implementation plan.
    #[error("the track declares no implementation plan")]
    ImplPlanNotFound,
    /// The implementation plan could not be read.
    #[error("the implementation plan could not be read: {message}")]
    ImplPlanReadFailed {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
    /// The review scope configuration could not be read.
    #[error("the review scope configuration could not be read: {message}")]
    ScopeConfigReadFailed {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
    /// No track could be resolved to run the gate for.
    #[error("no active track could be resolved: {message}")]
    TrackResolutionFailed {
        /// Opaque diagnostic from the resolution.
        message: FreeText,
    },
}

// ── BatchPlanCheckService ─────────────────────────────────────────────────────

/// Primary port for the Phase 3 gate, driven by the CLI batch-plan check
/// command (`IN-06`, `IN-07`, `AC-06`, `AC-07`).
pub trait BatchPlanCheckService: Send + Sync {
    /// Runs the gate for the track `cmd` names.
    ///
    /// A non-conforming plan is a [`BatchPlanCheckOutput::Blocked`] value, not
    /// an error.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPlanCheckError`] when an input could not be read, so the
    /// gate could not be run at all.
    fn check(
        &self,
        cmd: BatchPlanCheckCommand,
    ) -> Result<BatchPlanCheckOutput, BatchPlanCheckError>;
}

// ── BatchPlanCheckInteractor ──────────────────────────────────────────────────

/// Resolves the track context, reads the batch plan, the planned tasks and the
/// scope configuration through its three ports, and delegates the judgement to
/// the domain gate.
///
/// Resolution is absorbed here rather than performed by the adapter beforehand,
/// so one request drives one interactor, and it reuses the shared resolve
/// service instead of a second mechanism. Holds no ceiling arithmetic of its
/// own. An absent batch plan is mapped to an error rather than a pass: on a
/// track this gate applies to, the artifact is required (`CN-09`).
pub struct BatchPlanCheckInteractor {
    batch_plan_reader: Arc<dyn BatchPlanReaderPort>,
    planned_task_reader: Arc<dyn PlannedTaskReaderPort>,
    scope_config_reader: Arc<dyn ScopeConfigReaderPort>,
    track_resolver: Arc<dyn ActiveTrackResolveService>,
}

impl BatchPlanCheckInteractor {
    /// Wires the interactor to the three reads it runs the gate from and to the
    /// service it resolves its track context through.
    #[must_use]
    pub fn new(
        batch_plan_reader: Arc<dyn BatchPlanReaderPort>,
        planned_task_reader: Arc<dyn PlannedTaskReaderPort>,
        scope_config_reader: Arc<dyn ScopeConfigReaderPort>,
        track_resolver: Arc<dyn ActiveTrackResolveService>,
    ) -> BatchPlanCheckInteractor {
        BatchPlanCheckInteractor {
            batch_plan_reader,
            planned_task_reader,
            scope_config_reader,
            track_resolver,
        }
    }

    /// Resolves the command's track context with READ semantics: an explicit id
    /// wins, an omitted one is self-resolved from the current branch.
    fn resolve_track(&self, track_id: Option<String>) -> Result<TrackId, BatchPlanCheckError> {
        let resolved = self.track_resolver.resolve_for_read(track_id).map_err(|error| {
            BatchPlanCheckError::TrackResolutionFailed { message: FreeText::new(error.to_string()) }
        })?;
        TrackId::try_new(resolved.clone()).map_err(|error| {
            BatchPlanCheckError::TrackResolutionFailed {
                message: FreeText::new(format!("invalid track id '{resolved}': {error}")),
            }
        })
    }
}

impl BatchPlanCheckService for BatchPlanCheckInteractor {
    fn check(
        &self,
        cmd: BatchPlanCheckCommand,
    ) -> Result<BatchPlanCheckOutput, BatchPlanCheckError> {
        let track_id = self.resolve_track(cmd.track_id)?;

        let plan =
            self.batch_plan_reader.read(&cmd.items_dir, &track_id).map_err(
                |error| match error {
                    PlanArtifactReadError::NotFound => BatchPlanCheckError::BatchPlanNotFound,
                    PlanArtifactReadError::ReadFailed { message } => {
                        BatchPlanCheckError::BatchPlanReadFailed { message }
                    }
                },
            )?;
        let planned_tasks = self
            .planned_task_reader
            .read_planned_tasks(&cmd.items_dir, &track_id)
            .map_err(|error| match error {
                PlanArtifactReadError::NotFound => BatchPlanCheckError::ImplPlanNotFound,
                PlanArtifactReadError::ReadFailed { message } => {
                    BatchPlanCheckError::ImplPlanReadFailed { message }
                }
            })?;
        let scope_config =
            self.scope_config_reader.read(&cmd.items_dir, &track_id).map_err(|error| {
                let ScopeConfigReadError::ReadFailed { message } = error;
                BatchPlanCheckError::ScopeConfigReadFailed { message }
            })?;

        Ok(map_outcome(check_batch_plan(&plan, &scope_config, &planned_tasks)))
    }
}
