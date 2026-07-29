//! Application contracts for the declared batch plan (spec `IN-01`, `IN-06`,
//! `IN-07`, `AC-01`, `AC-05`, `AC-06`, `AC-07`, `AC-08`, `CN-09`).
//!
//! This module owns the command the Phase 3 gate is driven by, the primary port
//! that runs it, the interactor implementing that port, and the driven ports
//! the inputs are read through. The judgement itself belongs to the domain: the
//! interactor reads and delegates, and holds no ceiling arithmetic.

mod check_service;
mod ports;

use std::path::PathBuf;

use domain::TrackId;

pub use check_service::{BatchPlanCheckError, BatchPlanCheckInteractor, BatchPlanCheckService};
pub use ports::{
    BatchPlanReadError, BatchPlanReaderPort, PlannedTaskReadError, PlannedTaskReaderPort,
    ScopeConfigReadError, ScopeConfigReaderPort, ScopeDiffMeasureError, ScopeDiffMeasurePort,
};

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

/// What to run the Phase 3 gate over (`IN-06`, `AC-06`).
///
/// `track_id` is the domain identifier, so the primary adapter validates it
/// once at the CLI boundary; `items_dir` is a raw path with no domain meaning
/// and travels with the command rather than with construction, which keeps the
/// composition root a zero-argument wiring accessor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlanCheckCommand {
    /// The track whose plan is checked.
    pub track_id: TrackId,
    /// The directory the track's artifacts live under.
    pub items_dir: PathBuf,
}
