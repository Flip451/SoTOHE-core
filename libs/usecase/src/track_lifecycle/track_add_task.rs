use domain::{NonEmptyString, TaskId, TrackId};

use crate::task_ops::{TaskOperationError, TaskOperationOutput};

use super::{TrackItemsDirectory, TrackSelection};

/// Validated command for adding a task.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackAddTaskCommand {
    /// The track items directory used by the operation.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
    /// The validated task description.
    pub description: NonEmptyString,
    /// Optional validated task section.
    pub section: Option<NonEmptyString>,
    /// Optional validated predecessor task id.
    pub after: Option<TaskId>,
}

/// Secondary port for adding a task with a validated command.
pub trait TrackTaskAddPort: Send + Sync {
    /// Adds a task using the validated command boundary.
    fn add_task(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
        description: NonEmptyString,
        section: Option<NonEmptyString>,
        after: Option<TaskId>,
    ) -> Result<TaskOperationOutput, TaskOperationError>;
}
