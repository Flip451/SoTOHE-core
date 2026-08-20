//! Compatibility resolution boundary for callers outside the track context.
//!
//! The legacy composition-root resolution helpers are retained while their
//! callers migrate.  This module gives those callers the same read, write, and
//! active-detection semantics through the normal driver → usecase → port path.

use std::sync::Arc;

use domain::TrackId;
use thiserror::Error;

use crate::git_workflow::DiagnosticText;

use super::{TrackItemsDirectory, TrackSelection, TrackWorkspaceRoot};

/// Typed command for the compatibility resolution use case.
#[derive(Debug, PartialEq, Eq)]
pub enum TrackResolutionCommand {
    /// Resolve a selection using the repository derived from `track/items`.
    ReadFromItems { track: TrackSelection, items_dir: TrackItemsDirectory },
    /// Resolve a selection using an explicit workspace root for a read.
    ReadFromRoot { track: TrackSelection, workspace_root: TrackWorkspaceRoot },
    /// Resolve a selection using the repository derived from `track/items` and
    /// enforce the write branch guard.
    WriteFromItems { track: TrackSelection, items_dir: TrackItemsDirectory },
    /// Resolve a selection using an explicit workspace root and enforce the
    /// write branch guard.
    WriteFromRoot { track: TrackSelection, workspace_root: TrackWorkspaceRoot },
    /// Detect whether the current branch identifies an active track.
    DetectActive { workspace_root: TrackWorkspaceRoot },
}

/// Presentation-free result of compatibility resolution.
#[derive(Debug, PartialEq, Eq)]
pub enum TrackResolutionResult {
    /// A validated track id was resolved.
    Resolved(TrackId),
    /// The current workspace has no active track.
    Inactive,
}

/// Error returned when infrastructure cannot perform compatibility resolution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrackResolutionCompatError {
    /// A user-safe infrastructure diagnostic.
    #[error("{0}")]
    Unavailable(DiagnosticText),
}

/// Secondary port for compatibility resolution.
pub trait TrackResolutionPort: Send + Sync {
    /// Executes one typed resolution command.
    fn execute(
        &self,
        command: TrackResolutionCommand,
    ) -> Result<TrackResolutionResult, TrackResolutionCompatError>;
}

/// Application-service boundary for compatibility resolution.
pub trait TrackResolutionService: Send + Sync {
    /// Executes one typed resolution command.
    fn execute(
        &self,
        command: TrackResolutionCommand,
    ) -> Result<TrackResolutionResult, TrackResolutionCompatError>;
}

/// Interactor that forwards the command to the injected resolution port.
pub struct TrackResolutionInteractor {
    operation: Arc<dyn TrackResolutionPort>,
}

impl TrackResolutionInteractor {
    /// Creates an interactor over a resolution port.
    #[must_use]
    pub fn new(operation: Arc<dyn TrackResolutionPort>) -> Self {
        Self { operation }
    }
}

impl TrackResolutionService for TrackResolutionInteractor {
    fn execute(
        &self,
        command: TrackResolutionCommand,
    ) -> Result<TrackResolutionResult, TrackResolutionCompatError> {
        self.operation.execute(command)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct RecordingPort {
        result: Mutex<Option<Result<TrackResolutionResult, TrackResolutionCompatError>>>,
        commands: Mutex<Vec<TrackResolutionCommand>>,
    }

    impl RecordingPort {
        fn new(result: Result<TrackResolutionResult, TrackResolutionCompatError>) -> Self {
            Self { result: Mutex::new(Some(result)), commands: Mutex::new(Vec::new()) }
        }
    }

    impl TrackResolutionPort for RecordingPort {
        fn execute(
            &self,
            command: TrackResolutionCommand,
        ) -> Result<TrackResolutionResult, TrackResolutionCompatError> {
            self.commands.lock().unwrap().push(command);
            self.result.lock().unwrap().take().unwrap()
        }
    }

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new("track/items".into()).unwrap()
    }

    #[test]
    fn test_track_resolution_interactor_forwards_command_and_result() {
        let port = Arc::new(RecordingPort::new(Ok(TrackResolutionResult::Inactive)));
        let interactor = TrackResolutionInteractor::new(port.clone());
        let command = TrackResolutionCommand::ReadFromItems {
            track: TrackSelection::Active,
            items_dir: items_dir(),
        };

        let result = interactor.execute(command);

        assert_eq!(result, Ok(TrackResolutionResult::Inactive));
        assert_eq!(port.commands.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_track_resolution_interactor_forwards_port_error() {
        let port = Arc::new(RecordingPort::new(Err(TrackResolutionCompatError::Unavailable(
            DiagnosticText::new("resolution failed"),
        ))));
        let interactor = TrackResolutionInteractor::new(port);
        let result = interactor.execute(TrackResolutionCommand::ReadFromItems {
            track: TrackSelection::Active,
            items_dir: items_dir(),
        });

        assert_eq!(
            result,
            Err(TrackResolutionCompatError::Unavailable(DiagnosticText::new("resolution failed",)))
        );
    }
}
