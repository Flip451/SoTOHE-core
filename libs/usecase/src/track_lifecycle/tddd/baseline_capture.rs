//! Application boundary for TDDD baseline capture.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackItemsDirectory, TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::{TrackLayerSelection, TrackSourceWorkspace};

/// Typed input for a TDDD baseline-capture operation.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackBaselineCaptureCommand {
    /// Track selection supplied by the primary adapter.
    pub track: TrackSelection,
    /// Workspace containing the track and its configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// Optional workspace from which rustdoc is executed.
    pub source_workspace: Option<TrackSourceWorkspace>,
    /// Layer filter for the capture operation.
    pub layer: TrackLayerSelection,
}

/// Result for a single captured layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackBaselineCaptureLayerResult {
    /// A new baseline was written for the layer.
    Captured { layer: domain::tddd::LayerId },
    /// An existing baseline was retained unchanged.
    AlreadyExists { layer: domain::tddd::LayerId },
}

/// Result of TDDD baseline capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackBaselineCaptureResult {
    /// Per-layer capture outcomes.
    pub layers: Vec<TrackBaselineCaptureLayerResult>,
}

/// Error returned by the TDDD baseline-capture boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackBaselineCaptureError {
    /// The operation could not be completed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackBaselineCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackBaselineCaptureError {}

/// Secondary port for the blocking baseline-capture operation.
pub trait TrackBaselineCapturePort: Send + Sync {
    /// Executes the capture after track selection has been resolved.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackBaselineCaptureCommand,
    ) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError>;
}

/// Application service for TDDD baseline capture.
pub trait TrackBaselineCaptureService: Send + Sync {
    /// Resolves the write selection and executes the capture operation.
    fn execute(
        &self,
        command: TrackBaselineCaptureCommand,
    ) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError>;
}

/// Interactor for the TDDD baseline-capture command context.
pub struct TrackBaselineCaptureInteractor {
    operation: Arc<dyn TrackBaselineCapturePort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackBaselineCaptureInteractor {
    /// Creates an interactor from the capture operation and selection resolver.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackBaselineCapturePort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackBaselineCaptureService for TrackBaselineCaptureInteractor {
    fn execute(
        &self,
        command: TrackBaselineCaptureCommand,
    ) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError> {
        let items_dir = items_dir_for_workspace(&command.workspace_root)?;
        let track_id =
            self.resolver.resolve_required(&items_dir, &command.track).map_err(execution_failed)?;
        self.operation.execute(track_id, command)
    }
}

fn items_dir_for_workspace(
    workspace_root: &TrackWorkspaceRoot,
) -> Result<TrackItemsDirectory, TrackBaselineCaptureError> {
    TrackItemsDirectory::try_new(workspace_root.as_path().join("track").join("items"))
        .map_err(execution_failed)
}

fn execution_failed(error: DiagnosticText) -> TrackBaselineCaptureError {
    TrackBaselineCaptureError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::TrackViewsScope;

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new("workspace".into()).expect("workspace is valid")
    }

    fn command() -> TrackBaselineCaptureCommand {
        TrackBaselineCaptureCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("capture-track").expect("track id is valid"),
            ),
            workspace_root: workspace_root(),
            source_workspace: Some(
                TrackSourceWorkspace::try_new("source".into()).expect("source workspace is valid"),
            ),
            layer: TrackLayerSelection::One(
                domain::tddd::LayerId::try_new("usecase").expect("layer is valid"),
            ),
        }
    }

    struct RecordingResolver {
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
        error: Option<DiagnosticText>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            items_dir: &TrackItemsDirectory,
            selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            if let Some(error) = &self.error {
                return Err(DiagnosticText::new(error.to_string()));
            }
            self.calls
                .lock()
                .expect("resolver lock is available")
                .push((items_dir.as_path().to_path_buf(), selection.clone()));
            Ok(TrackId::try_new("capture-track").expect("track id is valid"))
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            Ok(TrackId::try_new("capture-track").expect("track id is valid"))
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
    }

    impl TrackBaselineCapturePort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            command: TrackBaselineCaptureCommand,
        ) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError> {
            assert_eq!(command.workspace_root.as_path(), Path::new("workspace"));
            self.calls.lock().expect("operation lock is available").push(track_id.clone());
            Ok(TrackBaselineCaptureResult {
                layers: vec![TrackBaselineCaptureLayerResult::Captured {
                    layer: domain::tddd::LayerId::try_new("usecase").expect("layer is valid"),
                }],
            })
        }
    }

    #[test]
    fn test_track_baseline_capture_interactor_resolves_selection_and_forwards_command() {
        let resolver = Arc::new(RecordingResolver { calls: Mutex::new(Vec::new()), error: None });
        let operation = Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()) });
        let interactor = TrackBaselineCaptureInteractor::new(operation.clone(), resolver.clone());

        let result = interactor.execute(command()).expect("capture succeeds");

        assert!(matches!(
            result.layers.as_slice(),
            [TrackBaselineCaptureLayerResult::Captured { layer }] if layer.as_ref() == "usecase"
        ));
        assert_eq!(operation.calls.lock().expect("operation lock is available").len(), 1);
        let resolver_calls = resolver.calls.lock().expect("resolver lock is available");
        assert_eq!(
            resolver_calls.first().map(|call| call.0.as_path()),
            Some(Path::new("workspace/track/items"))
        );
    }

    #[test]
    fn test_track_baseline_capture_interactor_resolver_failure_maps_to_execution_error() {
        let interactor = TrackBaselineCaptureInteractor::new(
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()) }),
            Arc::new(RecordingResolver {
                calls: Mutex::new(Vec::new()),
                error: Some(DiagnosticText::new("branch unavailable")),
            }),
        );

        let error = interactor.execute(command()).expect_err("resolver must fail");

        assert_eq!(error.to_string(), "branch unavailable");
    }
}
