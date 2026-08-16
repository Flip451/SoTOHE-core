use crate::git_workflow::DiagnosticText;
use crate::task_ops::NextTaskOutput;
use domain::TrackId;
use thiserror::Error;

use super::{TrackItemsDirectory, TrackSelection};

/// Validated command for querying the next open task.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackNextTaskCommand {
    /// The track items directory used by the query.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
}

/// Error returned by the next-task secondary port.
#[derive(Debug, Error)]
pub enum TrackNextTaskError {
    /// The storage query or its usecase mapping failed.
    #[error("{0}")]
    ExecutionFailed(DiagnosticText),
}

/// Secondary port for querying the next open task.
pub trait TrackNextTaskQueryPort: Send + Sync {
    /// Returns the next open task, when one exists.
    fn next_task(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
    ) -> Result<Option<NextTaskOutput>, TrackNextTaskError>;
}
