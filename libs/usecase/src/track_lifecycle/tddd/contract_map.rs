//! Shared value objects and the Track contract-map command context.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{
    TrackCatalogueEntryCount, TrackItemsDirectory, TrackLayerFilter, TrackRenderedLayerCount,
    TrackSelection, TrackSelectionPort, TrackWorkspaceRoot,
};

/// Typed command for rendering a track's catalogue-input contract map.
pub struct TrackContractMapCommand {
    /// Explicit or active track selection.
    pub track: TrackSelection,
    /// Directory containing the track artifacts.
    pub items_dir: TrackItemsDirectory,
    /// Workspace containing architecture rules and renderer configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// Layers to include in the rendered map.
    pub layers: TrackLayerFilter,
}

/// Presentation-free result of contract-map rendering.
pub struct TrackContractMapResult {
    /// The track whose map was written.
    pub track_id: TrackId,
    /// Number of layers rendered after filtering.
    pub rendered_layers: TrackRenderedLayerCount,
    /// Number of catalogue entries loaded for the track.
    pub catalogue_entries: TrackCatalogueEntryCount,
    /// Non-fatal renderer warnings.
    pub warnings: Vec<domain::tddd::ContractMapRenderWarning>,
}

/// Error returned by the contract-map command boundary.
#[derive(Debug)]
pub enum TrackContractMapError {
    /// The contract-map operation or its selection resolution failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackContractMapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackContractMapError {}

/// Secondary port for the blocking contract-map operation.
pub trait TrackContractMapPort: Send + Sync {
    /// Renders and persists a contract map for a resolved track.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackContractMapCommand,
    ) -> Result<TrackContractMapResult, TrackContractMapError>;
}

/// Application service for contract-map rendering.
pub trait TrackContractMapService: Send + Sync {
    /// Resolves the selection and executes the contract-map operation.
    fn execute(
        &self,
        command: TrackContractMapCommand,
    ) -> Result<TrackContractMapResult, TrackContractMapError>;
}

/// Interactor for the contract-map command context.
pub struct TrackContractMapInteractor {
    operation: Arc<dyn TrackContractMapPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackContractMapInteractor {
    /// Creates an interactor from the operation and selection ports.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackContractMapPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackContractMapService for TrackContractMapInteractor {
    fn execute(
        &self,
        command: TrackContractMapCommand,
    ) -> Result<TrackContractMapResult, TrackContractMapError> {
        let track_id = match &command.track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => {
                self.resolver.resolve_active(&command.workspace_root).map_err(execution_failed)?
            }
        };
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackContractMapError {
    TrackContractMapError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::TrackViewsScope;

    struct RecordingResolver {
        active: Result<TrackId, DiagnosticText>,
        calls: Mutex<usize>,
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
            *self.calls.lock().expect("resolver lock is available") += 1;
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
        calls: Mutex<Vec<TrackId>>,
        result: Result<TrackContractMapResult, TrackContractMapError>,
    }

    impl TrackContractMapPort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            _command: TrackContractMapCommand,
        ) -> Result<TrackContractMapResult, TrackContractMapError> {
            self.calls.lock().expect("operation lock is available").push(track_id);
            match &self.result {
                Ok(result) => Ok(TrackContractMapResult {
                    track_id: result.track_id.clone(),
                    rendered_layers: result.rendered_layers,
                    catalogue_entries: result.catalogue_entries,
                    warnings: Vec::new(),
                }),
                Err(_) => Err(TrackContractMapError::ExecutionFailed(DiagnosticText::new(
                    "contract-map operation failed",
                ))),
            }
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    fn command(track: TrackSelection) -> TrackContractMapCommand {
        TrackContractMapCommand {
            track,
            items_dir: TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            workspace_root: TrackWorkspaceRoot::try_new(PathBuf::from("workspace"))
                .expect("workspace root is valid"),
            layers: TrackLayerFilter::All,
        }
    }

    fn result() -> TrackContractMapResult {
        TrackContractMapResult {
            track_id: track_id("contract-map-track"),
            rendered_layers: TrackRenderedLayerCount::new(2),
            catalogue_entries: TrackCatalogueEntryCount::new(3),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn test_track_contract_map_interactor_explicit_selection_forwards_without_resolution() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            calls: Mutex::new(0),
        });
        let operation =
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok(result()) });
        let interactor = TrackContractMapInteractor::new(operation.clone(), resolver.clone());

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
            .expect("contract map succeeds");

        assert_eq!(result.track_id.as_ref(), "contract-map-track");
        assert_eq!(result.rendered_layers.value(), 2);
        assert_eq!(
            operation
                .calls
                .lock()
                .expect("operation lock is available")
                .first()
                .expect("one operation call")
                .as_ref(),
            "explicit-track"
        );
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 0);
    }

    #[test]
    fn test_track_contract_map_interactor_active_selection_uses_resolver() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            calls: Mutex::new(0),
        });
        let operation =
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok(result()) });
        let interactor = TrackContractMapInteractor::new(operation.clone(), resolver.clone());

        interactor.execute(command(TrackSelection::Active)).expect("active contract map succeeds");

        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 1);
        assert_eq!(
            operation
                .calls
                .lock()
                .expect("operation lock is available")
                .first()
                .expect("one operation call")
                .as_ref(),
            "active-track"
        );
    }

    #[test]
    fn test_track_contract_map_interactor_resolver_failure_returns_execution_error() {
        let resolver = Arc::new(RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            calls: Mutex::new(0),
        });
        let operation =
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok(result()) });
        let interactor = TrackContractMapInteractor::new(operation.clone(), resolver);

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "active track unavailable");
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }
}
