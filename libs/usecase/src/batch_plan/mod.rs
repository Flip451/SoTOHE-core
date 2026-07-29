//! Application contracts for the declared batch plan (spec `IN-01`, `IN-06`,
//! `IN-07`, `AC-01`, `AC-05`, `AC-06`, `AC-07`, `AC-08`, `CN-09`).
//!
//! This module owns the command the Phase 3 gate is driven by, the primary port
//! that runs it, the interactor implementing that port, and the driven ports
//! the inputs are read through. The judgement itself belongs to the domain: the
//! interactor reads and delegates, and holds no ceiling arithmetic.

mod check_service;
mod output;
mod ports;

use std::path::PathBuf;

pub use check_service::{BatchPlanCheckError, BatchPlanCheckInteractor, BatchPlanCheckService};
// The gate verdict crosses the boundary as this crate's own record: the
// interactor maps the domain value here, so the adapter never holds one.
pub use output::{BatchPlanCheckOutput, BatchPlanViolationOutput, NonEmptyViolationOutputs};
pub use ports::{
    BatchPlanReaderPort, PlanArtifactReadError, PlannedTaskReaderPort, ScopeConfigReadError,
    ScopeConfigReaderPort, ScopeDiffMeasureError, ScopeDiffMeasurePort,
};

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

/// What to run the Phase 3 gate over, carrying the track context unresolved
/// (`IN-06`, `AC-06`).
///
/// `track_id` is optional because resolving it is part of this use case rather
/// than a step the adapter performs first: the interactor owns both the
/// resolution and the check, so one request drives one interactor. The raw
/// string is the deliberately unresolved transport value, and turning it into a
/// domain identifier is exactly the resolution the interactor does. `items_dir`
/// is a raw path with no domain meaning and travels with the command rather
/// than with construction, which keeps the composition root a zero-argument
/// wiring accessor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlanCheckCommand {
    /// The track whose plan is checked, or `None` to resolve it from the
    /// current branch.
    pub track_id: Option<String>,
    /// The directory the track's artifacts live under.
    pub items_dir: PathBuf,
}
