//! Application boundary for catalogue-spec signal refresh.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackItemsDirectory, TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::{TrackLayerSelection, TrackLayerSignalResult};

/// Typed input for catalogue-spec signal refresh.
pub struct TrackCatalogueSpecSignalsCommand {
    /// Track selection supplied by the primary adapter.
    pub track: TrackSelection,
    /// Directory containing the selected track.
    pub items_dir: TrackItemsDirectory,
    /// Workspace containing the track and its TDDD configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// Optional layer filter for the refresh operation.
    pub layer: TrackLayerSelection,
}

/// Presentation-free result of catalogue-spec signal refresh.
pub struct TrackCatalogueSpecSignalsResult {
    /// Per-layer refresh outcomes.
    pub layers: Vec<TrackLayerSignalResult>,
}

/// Error returned by the catalogue-spec signal boundary.
#[derive(Debug)]
pub enum TrackCatalogueSpecSignalsError {
    /// The signal refresh could not be completed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackCatalogueSpecSignalsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackCatalogueSpecSignalsError {}

/// Secondary port for the blocking catalogue-spec signal refresh.
pub trait TrackCatalogueSpecSignalsPort: Send + Sync {
    /// Refreshes the selected track after selection has been resolved.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackCatalogueSpecSignalsCommand,
    ) -> Result<TrackCatalogueSpecSignalsResult, TrackCatalogueSpecSignalsError>;
}

/// Application service for catalogue-spec signal refresh.
pub trait TrackCatalogueSpecSignalsService: Send + Sync {
    /// Resolves the requested track and refreshes its catalogue-spec signals.
    fn execute(
        &self,
        command: TrackCatalogueSpecSignalsCommand,
    ) -> Result<TrackCatalogueSpecSignalsResult, TrackCatalogueSpecSignalsError>;
}

/// Interactor for the catalogue-spec signal command context.
pub struct TrackCatalogueSpecSignalsInteractor {
    operation: Arc<dyn TrackCatalogueSpecSignalsPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackCatalogueSpecSignalsInteractor {
    /// Creates an interactor from the signal operation and selection resolver.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackCatalogueSpecSignalsPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackCatalogueSpecSignalsService for TrackCatalogueSpecSignalsInteractor {
    fn execute(
        &self,
        command: TrackCatalogueSpecSignalsCommand,
    ) -> Result<TrackCatalogueSpecSignalsResult, TrackCatalogueSpecSignalsError> {
        let track_id = self
            .resolver
            .resolve_required(&command.items_dir, &command.track)
            .map_err(execution_failed)?;
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackCatalogueSpecSignalsError {
    TrackCatalogueSpecSignalsError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::TrackViewsScope;

    struct RecordingResolver {
        result: Result<TrackId, DiagnosticText>,
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            items_dir: &TrackItemsDirectory,
            selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            self.calls
                .lock()
                .expect("resolver lock is available")
                .push((items_dir.as_path().to_path_buf(), selection.clone()));
            self.result.clone()
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            self.result.clone()
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
        calls: Mutex<Vec<(TrackId, TrackSelection, PathBuf, TrackLayerSelection)>>,
        error: Option<DiagnosticText>,
    }

    impl TrackCatalogueSpecSignalsPort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            command: TrackCatalogueSpecSignalsCommand,
        ) -> Result<TrackCatalogueSpecSignalsResult, TrackCatalogueSpecSignalsError> {
            self.calls.lock().expect("operation lock is available").push((
                track_id,
                command.track,
                command.items_dir.as_path().to_path_buf(),
                command.layer,
            ));
            match &self.error {
                Some(error) => Err(TrackCatalogueSpecSignalsError::ExecutionFailed(
                    DiagnosticText::new(error.to_string()),
                )),
                None => Ok(TrackCatalogueSpecSignalsResult { layers: Vec::new() }),
            }
        }
    }

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
            .expect("items directory is valid")
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace is valid")
    }

    fn command(track: TrackSelection) -> TrackCatalogueSpecSignalsCommand {
        TrackCatalogueSpecSignalsCommand {
            track,
            items_dir: items_dir(),
            workspace_root: workspace_root(),
            layer: TrackLayerSelection::One(
                domain::tddd::LayerId::try_new("usecase").expect("layer is valid"),
            ),
        }
    }

    #[test]
    fn test_track_catalogue_spec_signals_interactor_resolves_selection_and_forwards_command() {
        let resolver = Arc::new(RecordingResolver {
            result: Ok(TrackId::try_new("signals-track").expect("track id is valid")),
            calls: Mutex::new(Vec::new()),
        });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None });
        let interactor =
            TrackCatalogueSpecSignalsInteractor::new(operation.clone(), resolver.clone());

        interactor
            .execute(command(TrackSelection::Explicit(
                TrackId::try_new("signals-track").expect("track id is valid"),
            )))
            .map(|_| ())
            .expect("catalogue-spec refresh succeeds");

        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(calls.len(), 1);
        let call = calls.first().expect("one operation call is recorded");
        assert_eq!(call.0.as_ref(), "signals-track");
        assert!(matches!(&call.1, TrackSelection::Explicit(_)));
        assert_eq!(call.2, PathBuf::from("workspace/track/items"));
        assert!(matches!(&call.3, TrackLayerSelection::One(_)));
        assert_eq!(resolver.calls.lock().expect("resolver lock is available").len(), 1);
    }

    #[test]
    fn test_track_catalogue_spec_signals_interactor_resolver_failure_returns_execution_error() {
        let interactor = TrackCatalogueSpecSignalsInteractor::new(
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), error: None }),
            Arc::new(RecordingResolver {
                result: Err(DiagnosticText::new("active track unavailable")),
                calls: Mutex::new(Vec::new()),
            }),
        );

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "active track unavailable");
    }

    #[test]
    fn test_track_catalogue_spec_signals_interactor_operation_failure_returns_error() {
        let interactor = TrackCatalogueSpecSignalsInteractor::new(
            Arc::new(RecordingOperation {
                calls: Mutex::new(Vec::new()),
                error: Some(DiagnosticText::new("catalogue refresh failed")),
            }),
            Arc::new(RecordingResolver {
                result: Ok(TrackId::try_new("signals-track").expect("track id is valid")),
                calls: Mutex::new(Vec::new()),
            }),
        );

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("operation failure must propagate"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "catalogue refresh failed");
    }
}
