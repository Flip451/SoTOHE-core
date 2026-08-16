use crate::git_workflow::DiagnosticText;
use crate::task_ops::TaskCountsOutput;
use domain::TrackId;
use thiserror::Error;

use super::{TrackItemsDirectory, TrackSelection};

/// Validated command for querying task counts.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTaskCountsCommand {
    /// The track items directory used by the query.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
}

/// Error returned by the task-counts secondary port.
#[derive(Debug, Error)]
pub enum TrackTaskCountsError {
    /// The storage query or its usecase mapping failed.
    #[error("{0}")]
    ExecutionFailed(DiagnosticText),
}

/// Secondary port for querying per-status task counts.
pub trait TrackTaskCountsQueryPort: Send + Sync {
    /// Returns task counts for the selected track.
    fn task_counts(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
    ) -> Result<TaskCountsOutput, TrackTaskCountsError>;
}
