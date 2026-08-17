//! Application boundary for rendering a track's TDDD baseline graph.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackItemsDirectory, TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::{TrackLayerFilter, TrackRenderedLayerCount, TrackWrittenFileCount};

/// Typed input for a TDDD baseline-graph operation.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackBaselineGraphCommand {
    /// Track selection supplied by the primary adapter.
    pub track: TrackSelection,
    /// Directory containing the selected track.
    pub items_dir: TrackItemsDirectory,
    /// Workspace containing the track and its configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// Optional layer filter for the graph render.
    pub layers: TrackLayerFilter,
}

/// Presentation-free result of a TDDD baseline-graph render.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackBaselineGraphResult {
    /// Identity of the rendered track.
    pub track_id: TrackId,
    /// Number of layers rendered by the existing graph workflow.
    pub rendered_layers: TrackRenderedLayerCount,
    /// Number of graph files written by the existing graph workflow.
    pub written_files: TrackWrittenFileCount,
}

/// Error returned by the TDDD baseline-graph boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackBaselineGraphError {
    /// The graph operation could not be completed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackBaselineGraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackBaselineGraphError {}

/// Secondary port for the blocking baseline-graph operation.
pub trait TrackBaselineGraphPort: Send + Sync {
    /// Executes the graph render after track selection has been resolved.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackBaselineGraphCommand,
    ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError>;
}

/// Application service for TDDD baseline-graph rendering.
pub trait TrackBaselineGraphService: Send + Sync {
    /// Resolves the track selection and executes the graph operation.
    fn execute(
        &self,
        command: TrackBaselineGraphCommand,
    ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError>;
}

/// Interactor for the TDDD baseline-graph command context.
pub struct TrackBaselineGraphInteractor {
    operation: Arc<dyn TrackBaselineGraphPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackBaselineGraphInteractor {
    /// Creates an interactor from the graph operation and selection resolver.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackBaselineGraphPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackBaselineGraphService for TrackBaselineGraphInteractor {
    fn execute(
        &self,
        command: TrackBaselineGraphCommand,
    ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> {
        let track_id = self
            .resolver
            .resolve_required(&command.items_dir, &command.track)
            .map_err(execution_failed)?;
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackBaselineGraphError {
    TrackBaselineGraphError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::TrackViewsScope;

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
            .expect("items directory is valid")
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace root is valid")
    }

    fn command() -> TrackBaselineGraphCommand {
        TrackBaselineGraphCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("graph-track").expect("track id is valid"),
            ),
            items_dir: items_dir(),
            workspace_root: workspace_root(),
            layers: TrackLayerFilter::Selected(vec![
                domain::tddd::LayerId::try_new("usecase").expect("layer is valid"),
            ]),
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
            Ok(TrackId::try_new("graph-track").expect("track id is valid"))
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            Ok(TrackId::try_new("graph-track").expect("track id is valid"))
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
        calls: Mutex<Vec<(TrackId, TrackLayerFilter)>>,
        result: Result<(usize, usize), DiagnosticText>,
    }

    impl TrackBaselineGraphPort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            command: TrackBaselineGraphCommand,
        ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> {
            assert_eq!(command.workspace_root.as_path(), Path::new("workspace"));
            self.calls
                .lock()
                .expect("operation lock is available")
                .push((track_id.clone(), command.layers));
            match &self.result {
                Ok((rendered_layers, written_files)) => Ok(TrackBaselineGraphResult {
                    track_id,
                    rendered_layers: TrackRenderedLayerCount::new(*rendered_layers),
                    written_files: TrackWrittenFileCount::new(*written_files),
                }),
                Err(error) => Err(TrackBaselineGraphError::ExecutionFailed(DiagnosticText::new(
                    error.to_string(),
                ))),
            }
        }
    }

    #[test]
    fn test_track_baseline_graph_interactor_resolves_selection_and_forwards_command() {
        let resolver = Arc::new(RecordingResolver { calls: Mutex::new(Vec::new()), error: None });
        let operation =
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok((1, 2)) });
        let interactor = TrackBaselineGraphInteractor::new(operation.clone(), resolver.clone());

        let result = interactor.execute(command()).expect("baseline graph succeeds");

        assert_eq!(result.track_id.as_ref(), "graph-track");
        assert_eq!(result.rendered_layers.value(), 1);
        assert_eq!(result.written_files.value(), 2);
        assert_eq!(operation.calls.lock().expect("operation lock is available").len(), 1);
        let resolver_calls = resolver.calls.lock().expect("resolver lock is available");
        assert_eq!(
            resolver_calls.first().map(|call| call.0.as_path()),
            Some(Path::new("workspace/track/items"))
        );
    }

    #[test]
    fn test_track_baseline_graph_interactor_resolver_failure_maps_to_execution_error() {
        let interactor = TrackBaselineGraphInteractor::new(
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok((1, 1)) }),
            Arc::new(RecordingResolver {
                calls: Mutex::new(Vec::new()),
                error: Some(DiagnosticText::new("branch unavailable")),
            }),
        );

        let error = interactor.execute(command()).expect_err("resolver must fail");

        assert_eq!(error.to_string(), "branch unavailable");
    }

    #[test]
    fn test_track_baseline_graph_interactor_explicit_selection_write_guard_failure_fails_closed() {
        let interactor = TrackBaselineGraphInteractor::new(
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok((1, 1)) }),
            Arc::new(RecordingResolver {
                calls: Mutex::new(Vec::new()),
                error: Some(DiagnosticText::new(
                    "WRITE guard mismatch: explicit track-id 'graph-track' does not match branch-derived track-id 'other-track'",
                )),
            }),
        );

        let error = interactor
            .execute(command())
            .expect_err("explicit selection must fail closed on a branch mismatch");

        assert!(error.to_string().starts_with("WRITE guard mismatch:"));
    }
}
