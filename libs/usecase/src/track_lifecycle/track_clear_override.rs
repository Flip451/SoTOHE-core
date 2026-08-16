use crate::task_ops::{TaskOperationError, TaskOperationOutput};
use domain::TrackId;

use super::{TrackItemsDirectory, TrackSelection};

/// Validated command for clearing a status override.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackClearOverrideCommand {
    /// The track items directory used by the operation.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
}

/// Secondary port for clearing a track status override.
pub trait TrackOverrideClearPort: Send + Sync {
    /// Clears the current status override.
    fn clear_override(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
    ) -> Result<TaskOperationOutput, TaskOperationError>;
}
