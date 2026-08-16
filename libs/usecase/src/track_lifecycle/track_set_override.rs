use domain::{StatusOverrideKind, TrackId};

use crate::task_ops::{TaskOperationError, TaskOperationOutput};

use super::{DiagnosticText, TrackItemsDirectory, TrackSelection};

/// Validated command for setting a status override.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackSetOverrideCommand {
    /// The track items directory used by the operation.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
    /// The validated override kind.
    pub status: StatusOverrideKind,
    /// The diagnostic reason shown to callers.
    pub reason: DiagnosticText,
}

/// Secondary port for setting a track status override.
pub trait TrackOverrideSetPort: Send + Sync {
    /// Sets the requested status override.
    fn set_override(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
        status: StatusOverrideKind,
        reason: DiagnosticText,
    ) -> Result<TaskOperationOutput, TaskOperationError>;
}
