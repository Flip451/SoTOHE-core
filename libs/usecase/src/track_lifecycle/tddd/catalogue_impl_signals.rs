//! Application boundary for catalogue-to-implementation signal evaluation.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::{TrackCatalogueImplLayerResult, TrackLayerSelection};

/// Typed input for catalogue-to-implementation signal evaluation.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackCatalogueImplSignalsCommand {
    /// Track selection supplied by the primary adapter.
    pub track: TrackSelection,
    /// Workspace containing the track artifacts and catalogue configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// Optional layer filter for the signal evaluation.
    pub layer: TrackLayerSelection,
}

/// Presentation-free result of catalogue-to-implementation signal evaluation.
pub struct TrackCatalogueImplSignalsResult {
    /// Per-layer implementation signals.
    pub layers: Vec<TrackCatalogueImplLayerResult>,
}

/// Error returned by the catalogue-to-implementation signal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackCatalogueImplSignalsError {
    /// The signal evaluation could not be completed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackCatalogueImplSignalsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackCatalogueImplSignalsError {}

/// Secondary port for the blocking catalogue-to-implementation evaluation.
pub trait TrackCatalogueImplSignalsPort: Send + Sync {
    /// Evaluates the selected track after selection has been resolved.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackCatalogueImplSignalsCommand,
    ) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError>;
}

/// Application service for catalogue-to-implementation signal evaluation.
pub trait TrackCatalogueImplSignalsService: Send + Sync {
    /// Resolves active selection when needed and evaluates the selected track.
    fn execute(
        &self,
        command: TrackCatalogueImplSignalsCommand,
    ) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError>;
}

/// Interactor for the catalogue-to-implementation signal command context.
pub struct TrackCatalogueImplSignalsInteractor {
    operation: Arc<dyn TrackCatalogueImplSignalsPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackCatalogueImplSignalsInteractor {
    /// Creates an interactor from the signal operation and selection resolver.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackCatalogueImplSignalsPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackCatalogueImplSignalsService for TrackCatalogueImplSignalsInteractor {
    fn execute(
        &self,
        command: TrackCatalogueImplSignalsCommand,
    ) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError> {
        let track_id = match &command.track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => {
                self.resolver.resolve_active(&command.workspace_root).map_err(execution_failed)?
            }
        };
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackCatalogueImplSignalsError {
    TrackCatalogueImplSignalsError::ExecutionFailed(error)
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
            Ok(TrackId::try_new("required-track").expect("track id is valid"))
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
        calls: Mutex<Vec<(TrackId, TrackSelection, TrackLayerSelection)>>,
        error: Option<DiagnosticText>,
    }

    impl TrackCatalogueImplSignalsPort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            command: TrackCatalogueImplSignalsCommand,
        ) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError> {
            self.calls.lock().expect("operation lock is available").push((
                track_id,
                command.track,
                command.layer,
            ));
            match &self.error {
                Some(error) => Err(TrackCatalogueImplSignalsError::ExecutionFailed(
                    DiagnosticText::new(error.to_string()),
                )),
                None => Ok(TrackCatalogueImplSignalsResult { layers: Vec::new() }),
            }
        }
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace is valid")
    }

    fn command(track: TrackSelection) -> TrackCatalogueImplSignalsCommand {
        TrackCatalogueImplSignalsCommand {
            track,
            workspace_root: workspace_root(),
            layer: TrackLayerSelection::One(
                domain::tddd::LayerId::try_new("usecase").expect("layer is valid"),
            ),
        }
    }

    #[test]
    fn test_track_catalogue_impl_signals_interactor_explicit_selection_forwards_without_resolution()
    {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(TrackId::try_new("active-track").expect("track id is valid")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None });
        let interactor =
            TrackCatalogueImplSignalsInteractor::new(operation.clone(), resolver.clone());

        interactor
            .execute(command(TrackSelection::Explicit(
                TrackId::try_new("explicit-track").expect("track id is valid"),
            )))
            .expect("explicit evaluation succeeds");

        assert_eq!(*resolver.active_calls.lock().expect("resolver lock is available"), 0);
        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(calls.len(), 1);
        let call = calls.first().expect("one operation call is recorded");
        assert_eq!(call.0.as_ref(), "explicit-track");
        assert!(matches!(&call.1, TrackSelection::Explicit(_)));
        assert!(matches!(&call.2, TrackLayerSelection::One(_)));
    }

    #[test]
    fn test_track_catalogue_impl_signals_interactor_success_returns_structured_result() {
        let interactor = TrackCatalogueImplSignalsInteractor::new(
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None }),
            Arc::new(RecordingResolver {
                active: Ok(TrackId::try_new("active-track").expect("track id is valid")),
                active_calls: Mutex::new(0),
            }),
        );

        let result = interactor
            .execute(command(TrackSelection::Explicit(
                TrackId::try_new("explicit-track").expect("track id is valid"),
            )))
            .expect("successful evaluation returns a result");

        assert!(result.layers.is_empty());
    }

    #[test]
    fn test_track_catalogue_impl_signals_interactor_active_selection_resolves_and_forwards() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(TrackId::try_new("active-track").expect("track id is valid")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None });
        let interactor =
            TrackCatalogueImplSignalsInteractor::new(operation.clone(), resolver.clone());

        interactor.execute(command(TrackSelection::Active)).expect("active evaluation succeeds");

        assert_eq!(*resolver.active_calls.lock().expect("resolver lock is available"), 1);
        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(calls.first().map(|call| call.0.as_ref()), Some("active-track"));
    }

    #[test]
    fn test_track_catalogue_impl_signals_interactor_active_resolution_failure_returns_error() {
        let resolver = Arc::new(RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None });
        let interactor = TrackCatalogueImplSignalsInteractor::new(operation.clone(), resolver);

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("active resolution must fail"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "active track unavailable");
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }

    #[test]
    fn test_track_catalogue_impl_signals_interactor_operation_failure_returns_error() {
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            error: Some(DiagnosticText::new("catalogue evaluation failed")),
        });
        let interactor = TrackCatalogueImplSignalsInteractor::new(
            operation,
            Arc::new(RecordingResolver {
                active: Ok(TrackId::try_new("active-track").expect("track id is valid")),
                active_calls: Mutex::new(0),
            }),
        );

        let error = match interactor.execute(command(TrackSelection::Explicit(
            TrackId::try_new("explicit-track").expect("track id is valid"),
        ))) {
            Ok(_) => panic!("operation failure must propagate"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "catalogue evaluation failed");
    }
}
