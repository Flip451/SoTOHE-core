//! The Phase 3 gate as an application service (spec `IN-06`, `IN-07`, `AC-05`,
//! `AC-06`, `AC-07`, `AC-08`, `CN-09`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the
//! workspace module-size limit.

use std::sync::Arc;

use domain::FreeText;
use domain::batch_plan::{BatchPlanGateOutcome, check_batch_plan};
use thiserror::Error;

use super::{
    BatchPlanCheckCommand, BatchPlanReadError, BatchPlanReaderPort, PlannedTaskReadError,
    PlannedTaskReaderPort, ScopeConfigReadError, ScopeConfigReaderPort,
};

// ── BatchPlanCheckError ───────────────────────────────────────────────────────

/// A failure that prevented the Phase 3 gate from running (`AC-05`, `AC-06`,
/// `CN-09`).
///
/// Kept apart from the verdict: structural findings are data inside
/// [`BatchPlanGateOutcome::Blocked`], so an adapter can tell "the plan does not
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
}

// ── BatchPlanCheckService ─────────────────────────────────────────────────────

/// Primary port for the Phase 3 gate, driven by the CLI batch-plan check
/// command (`IN-06`, `IN-07`, `AC-06`, `AC-07`).
pub trait BatchPlanCheckService: Send + Sync {
    /// Runs the gate for the track `cmd` names.
    ///
    /// A non-conforming plan is a [`BatchPlanGateOutcome::Blocked`] value, not
    /// an error.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPlanCheckError`] when an input could not be read, so the
    /// gate could not be run at all.
    fn check(
        &self,
        cmd: BatchPlanCheckCommand,
    ) -> Result<BatchPlanGateOutcome, BatchPlanCheckError>;
}

// ── BatchPlanCheckInteractor ──────────────────────────────────────────────────

/// Reads the batch plan, the planned tasks and the scope configuration through
/// its three ports and delegates the judgement to the domain gate.
///
/// Holds no ceiling arithmetic of its own. An absent batch plan is mapped to an
/// error rather than a pass: on a track this gate applies to, the artifact is
/// required (`CN-09`).
pub struct BatchPlanCheckInteractor {
    batch_plan_reader: Arc<dyn BatchPlanReaderPort>,
    planned_task_reader: Arc<dyn PlannedTaskReaderPort>,
    scope_config_reader: Arc<dyn ScopeConfigReaderPort>,
}

impl BatchPlanCheckInteractor {
    /// Wires the interactor to the three reads it runs the gate from.
    #[must_use]
    pub fn new(
        batch_plan_reader: Arc<dyn BatchPlanReaderPort>,
        planned_task_reader: Arc<dyn PlannedTaskReaderPort>,
        scope_config_reader: Arc<dyn ScopeConfigReaderPort>,
    ) -> BatchPlanCheckInteractor {
        BatchPlanCheckInteractor { batch_plan_reader, planned_task_reader, scope_config_reader }
    }
}

impl BatchPlanCheckService for BatchPlanCheckInteractor {
    fn check(
        &self,
        cmd: BatchPlanCheckCommand,
    ) -> Result<BatchPlanGateOutcome, BatchPlanCheckError> {
        let plan =
            self.batch_plan_reader.read(&cmd.items_dir, &cmd.track_id).map_err(
                |error| match error {
                    BatchPlanReadError::NotFound => BatchPlanCheckError::BatchPlanNotFound,
                    BatchPlanReadError::ReadFailed { message } => {
                        BatchPlanCheckError::BatchPlanReadFailed { message }
                    }
                },
            )?;
        let planned_tasks = self
            .planned_task_reader
            .read_planned_tasks(&cmd.items_dir, &cmd.track_id)
            .map_err(|error| match error {
                PlannedTaskReadError::NotFound => BatchPlanCheckError::ImplPlanNotFound,
                PlannedTaskReadError::ReadFailed { message } => {
                    BatchPlanCheckError::ImplPlanReadFailed { message }
                }
            })?;
        let scope_config =
            self.scope_config_reader.read(&cmd.items_dir, &cmd.track_id).map_err(|error| {
                let ScopeConfigReadError::ReadFailed { message } = error;
                BatchPlanCheckError::ScopeConfigReadFailed { message }
            })?;

        Ok(check_batch_plan(&plan, &scope_config, &planned_tasks))
    }
}
