use domain::{TaskId, TrackId};

use crate::task_ops::{TaskOperationError, TaskTransitionOutcome};

use super::{TrackItemsDirectory, TrackSelection, TrackTaskTransition};

/// Validated command for a task transition.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTransitionCommand {
    /// The track items directory used by the operation.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
    /// The task to transition.
    pub task_id: TaskId,
    /// The validated target transition.
    pub transition: TrackTaskTransition,
}

/// Secondary port for applying a validated task transition.
pub trait TrackTaskTransitionPort: Send + Sync {
    /// Applies the requested task transition.
    fn transition_task(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
        task_id: TaskId,
        transition: TrackTaskTransition,
    ) -> Result<TaskTransitionOutcome, TaskOperationError>;
}
