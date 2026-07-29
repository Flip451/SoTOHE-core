//! Driven ports the batch-plan gates read the outside world through
//! (spec `IN-01`, `IN-06`, `IN-07`, `IN-10`, `AC-01`, `AC-05`, `AC-07`,
//! `AC-08`, `AC-17`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the
//! workspace module-size limit.
//!
//! Every port here is synchronous: the callers are single-shot CLI gates with
//! no concurrency and no ambient async runtime on this path, so the blocking
//! call is the deliberate boundary choice rather than an unstated default.

use std::path::Path;

use domain::batch_plan::{BatchPlanDocument, MeasuredScopeDiff};
use domain::review_v2::ReviewScopeConfig;
use domain::{FreeText, TrackId, TrackTask};
use thiserror::Error;

// ── BatchPlanReaderPort ───────────────────────────────────────────────────────

/// Failure of the batch-plan reading port (`IN-01`, `AC-05`).
///
/// An absent plan is its own variant because the callers act on it distinctly
/// rather than folding it into a generic read failure.
#[derive(Debug, Error)]
pub enum BatchPlanReadError {
    /// The track declares no batch plan.
    #[error("the track declares no batch plan")]
    NotFound,
    /// The batch plan exists but could not be read.
    #[error("the batch plan could not be read: {message}")]
    ReadFailed {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
}

/// Reads a track's declared batch plan as a validated domain document.
///
/// One read operation and no write: `batch-plan.json` is the impl-planner's to
/// author, and every consumer here only looks. Decoding stays in the adapter,
/// so the domain never sees the wire format.
pub trait BatchPlanReaderPort: Send + Sync {
    /// Reads the batch plan `track_id` declares under `items_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPlanReadError::NotFound`] when the track declares no
    /// batch plan, and [`BatchPlanReadError::ReadFailed`] when one exists but
    /// could not be read or decoded.
    fn read(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<BatchPlanDocument, BatchPlanReadError>;
}

// ── PlannedTaskReaderPort ─────────────────────────────────────────────────────

/// Failure of the planned-task reading port (`IN-07`, `AC-07`).
///
/// An absent implementation plan is its own variant so it is reported rather
/// than read as an empty task list.
#[derive(Debug, Error)]
pub enum PlannedTaskReadError {
    /// The track declares no implementation plan.
    #[error("the track declares no implementation plan")]
    NotFound,
    /// The implementation plan exists but could not be read.
    #[error("the implementation plan could not be read: {message}")]
    ReadFailed {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
}

/// Reads a track's planned tasks, keeping their status and declared
/// dependencies.
///
/// Separate from the pre-existing plan-status reader, which returns an
/// unordered map and drops both the commit-bearing status and the dependency
/// declarations — and those declarations are exactly what the batch-order check
/// reads.
pub trait PlannedTaskReaderPort: Send + Sync {
    /// Reads the tasks `track_id` plans under `items_dir`, in declaration
    /// order.
    ///
    /// # Errors
    ///
    /// Returns [`PlannedTaskReadError::NotFound`] when the track declares no
    /// implementation plan, and [`PlannedTaskReadError::ReadFailed`] when one
    /// exists but could not be read or decoded.
    fn read_planned_tasks(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<TrackTask>, PlannedTaskReadError>;
}

// ── ScopeConfigReaderPort ─────────────────────────────────────────────────────

/// Failure of the scope-configuration reading port (`IN-06`, `AC-08`).
///
/// Covers only a configuration that could not be loaded: a scope with no
/// configured ceiling is an unconstrained scope, not a failure.
#[derive(Debug, Error)]
pub enum ScopeConfigReadError {
    /// The scope configuration could not be read.
    #[error("the review scope configuration could not be read: {message}")]
    ReadFailed {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
}

/// Loads the review scope configuration, the source of the per-scope ceilings.
///
/// A port rather than a pre-built injected value, so composition roots stay
/// zero-argument wiring accessors and the items directory travels per call.
pub trait ScopeConfigReaderPort: Send + Sync {
    /// Loads the scope configuration `track_id` is reviewed under.
    ///
    /// # Errors
    ///
    /// Returns [`ScopeConfigReadError::ReadFailed`] when the configuration
    /// could not be read or parsed.
    fn read(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<ReviewScopeConfig, ScopeConfigReadError>;
}

// ── ScopeDiffMeasurePort ──────────────────────────────────────────────────────

/// Failure of the per-scope diff measurement port (`IN-10`, `AC-17`).
///
/// Covers only a measurement that could not be produced; a degraded base is not
/// modelled as a failure.
#[derive(Debug, Error)]
pub enum ScopeDiffMeasureError {
    /// The per-scope diff could not be measured.
    #[error("the per-scope diff could not be measured: {message}")]
    MeasureFailed {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
}

/// Measures a track's actual per-scope diff.
///
/// The surface exposes only the resulting figures: how lines are counted and
/// which base they are measured from are the adapter's responsibility.
pub trait ScopeDiffMeasurePort: Send + Sync {
    /// Measures the per-scope diff of `track_id` under `items_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`ScopeDiffMeasureError::MeasureFailed`] when no measurement
    /// could be produced.
    fn measure_scope_diff(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<MeasuredScopeDiff>, ScopeDiffMeasureError>;
}
