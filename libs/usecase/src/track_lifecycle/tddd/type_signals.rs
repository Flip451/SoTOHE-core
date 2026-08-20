//! Application boundary for TDDD type-signal evaluation.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::TrackLayerSelection;

/// Typed command for evaluating implementation type signals.
pub struct TrackTypeSignalsCommand {
    /// Track selection supplied by the primary adapter.
    pub track: TrackSelection,
    /// Workspace containing the track artifacts and TDDD configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// Optional layer filter for the signal evaluation.
    pub layer: TrackLayerSelection,
}

/// Presentation-free result of TDDD type-signal evaluation.
pub struct TrackTypeSignalsResult {
    /// Per-layer signal counts written by the operation.
    pub layers: Vec<super::TrackLayerSignalResult>,
}

/// Error returned by the type-signals command boundary.
#[derive(Debug)]
pub enum TrackTypeSignalsError {
    /// The type-signal evaluation could not be completed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackTypeSignalsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackTypeSignalsError {}

/// Secondary port for the blocking type-signal operation.
pub trait TrackTypeSignalsPort: Send + Sync {
    /// Evaluates type signals for a resolved track.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackTypeSignalsCommand,
    ) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError>;
}

/// Application service for TDDD type-signal evaluation.
pub trait TrackTypeSignalsService: Send + Sync {
    /// Resolves the requested track and evaluates its type signals.
    fn execute(
        &self,
        command: TrackTypeSignalsCommand,
    ) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError>;
}

/// Interactor for the TDDD type-signals command context.
pub struct TrackTypeSignalsInteractor {
    operation: Arc<dyn TrackTypeSignalsPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackTypeSignalsInteractor {
    /// Creates an interactor from the signal operation and selection resolver.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackTypeSignalsPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackTypeSignalsService for TrackTypeSignalsInteractor {
    fn execute(
        &self,
        command: TrackTypeSignalsCommand,
    ) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError> {
        let track_id = match &command.track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => {
                self.resolver.resolve_active(&command.workspace_root).map_err(execution_failed)?
            }
        };
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackTypeSignalsError {
    TrackTypeSignalsError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::{TrackItemsDirectory, TrackViewsScope};

    struct RecordingResolver {
        active: Result<TrackId, DiagnosticText>,
        active_calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            self.active.clone()
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            *self.active_calls.lock().expect("resolver lock is available") += 1;
            self.active.clone()
        }

        fn resolve_views_scope(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _selection: &TrackSelection,
        ) -> Result<TrackViewsScope, DiagnosticText> {
            Ok(TrackViewsScope::RegistryOnly)
        }
    }

    struct RecordingOperation {
        calls: Mutex<Vec<(TrackId, PathBuf, TrackLayerSelection)>>,
        error: Option<String>,
    }

    impl TrackTypeSignalsPort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            command: TrackTypeSignalsCommand,
        ) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError> {
            self.calls.lock().expect("operation lock is available").push((
                track_id,
                command.workspace_root.as_path().to_path_buf(),
                command.layer,
            ));
            match &self.error {
                Some(error) => {
                    Err(TrackTypeSignalsError::ExecutionFailed(DiagnosticText::new(error)))
                }
                None => Ok(TrackTypeSignalsResult { layers: Vec::new() }),
            }
        }
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace is valid")
    }

    fn command(track: TrackSelection) -> TrackTypeSignalsCommand {
        TrackTypeSignalsCommand {
            track,
            workspace_root: workspace_root(),
            layer: TrackLayerSelection::One(
                domain::tddd::LayerId::try_new("usecase").expect("layer is valid"),
            ),
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    #[test]
    fn test_track_type_signals_interactor_explicit_selection_forwards_without_resolution() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None });
        let interactor = TrackTypeSignalsInteractor::new(operation.clone(), resolver.clone());

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
            .expect("explicit signal evaluation succeeds");

        assert!(result.layers.is_empty());
        assert_eq!(*resolver.active_calls.lock().expect("resolver lock is available"), 0);
        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(calls.len(), 1);
        let call = calls.first().expect("one operation call is recorded");
        assert_eq!(call.0.as_ref(), "explicit-track");
        assert_eq!(call.1, PathBuf::from("workspace"));
        assert!(matches!(call.2, TrackLayerSelection::One(_)));
    }

    #[test]
    fn test_track_type_signals_interactor_active_selection_resolves_and_forwards() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None });
        let interactor = TrackTypeSignalsInteractor::new(operation.clone(), resolver.clone());

        interactor
            .execute(command(TrackSelection::Active))
            .expect("active signal evaluation succeeds");

        assert_eq!(*resolver.active_calls.lock().expect("resolver lock is available"), 1);
        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(
            calls.first().expect("one operation call is recorded").0.as_ref(),
            "active-track"
        );
    }

    #[test]
    fn test_track_type_signals_interactor_resolution_failure_returns_execution_error() {
        let resolver = Arc::new(RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None });
        let interactor = TrackTypeSignalsInteractor::new(operation.clone(), resolver);

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("active resolution must fail"),
            Err(error) => error,
        };

        assert!(
            matches!(error, TrackTypeSignalsError::ExecutionFailed(message) if message.as_str() == "active track unavailable")
        );
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }

    #[test]
    fn test_track_type_signals_interactor_operation_failure_returns_execution_error() {
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            error: Some("type-signals evaluation failed".to_owned()),
        });
        let interactor = TrackTypeSignalsInteractor::new(
            operation,
            Arc::new(RecordingResolver {
                active: Ok(track_id("active-track")),
                active_calls: Mutex::new(0),
            }),
        );

        let error = match interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
        {
            Ok(_) => panic!("operation failure must be returned"),
            Err(error) => error,
        };

        assert!(
            matches!(error, TrackTypeSignalsError::ExecutionFailed(message) if message.as_str() == "type-signals evaluation failed")
        );
    }
}
