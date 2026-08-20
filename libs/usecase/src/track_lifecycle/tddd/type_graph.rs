//! Application boundary for the removed TDDD type-graph command.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackItemsDirectory, TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::TrackLayerSelection;

/// Track TypeGraph cluster-depth application-boundary value.
#[derive(PartialEq, Eq)]
pub struct TrackTypeGraphClusterDepth(usize);

impl TrackTypeGraphClusterDepth {
    /// Wraps a cluster-depth value.
    #[must_use]
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the numeric cluster depth.
    #[must_use]
    pub fn value(&self) -> usize {
        self.0
    }
}

/// Track TypeGraph edge-selection application-boundary value.
#[derive(PartialEq, Eq)]
pub enum TrackTypeGraphEdgeSelection {
    /// Include method edges.
    Methods,
    /// Include field edges.
    Fields,
    /// Include impl edges.
    Impls,
    /// Include every edge kind.
    All,
}

/// Typed command for the removed type-graph command context.
pub struct TrackTypeGraphCommand {
    /// Explicit or active track selection.
    pub track: TrackSelection,
    /// Directory containing the selected track.
    pub items_dir: TrackItemsDirectory,
    /// Workspace containing the track artifacts.
    pub workspace_root: TrackWorkspaceRoot,
    /// Optional layer filter.
    pub layer: TrackLayerSelection,
    /// Requested cluster depth.
    pub cluster_depth: TrackTypeGraphClusterDepth,
    /// Requested edge selection.
    pub edges: TrackTypeGraphEdgeSelection,
}

/// Uninhabited type-graph result. Success is unrepresentable.
pub enum TrackTypeGraphResult {}

/// Error returned by the type-graph command boundary.
#[derive(Debug)]
pub enum TrackTypeGraphError {
    /// The command is removed and cannot succeed.
    RemovedCommand(DiagnosticText),
}

impl std::fmt::Display for TrackTypeGraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RemovedCommand(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackTypeGraphError {}

/// Secondary port for the blocking type-graph operation.
pub trait TrackTypeGraphPort: Send + Sync {
    /// Executes the removed type-graph operation for a resolved track.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackTypeGraphCommand,
    ) -> Result<TrackTypeGraphResult, TrackTypeGraphError>;
}

/// Application service for the removed type-graph command.
pub trait TrackTypeGraphService: Send + Sync {
    /// Resolves the requested track and executes the type-graph operation.
    fn execute(
        &self,
        command: TrackTypeGraphCommand,
    ) -> Result<TrackTypeGraphResult, TrackTypeGraphError>;
}

/// Interactor for the TDDD type-graph command context.
pub struct TrackTypeGraphInteractor {
    operation: Arc<dyn TrackTypeGraphPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackTypeGraphInteractor {
    /// Creates an interactor from the type-graph operation and selection resolver.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackTypeGraphPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackTypeGraphService for TrackTypeGraphInteractor {
    fn execute(
        &self,
        command: TrackTypeGraphCommand,
    ) -> Result<TrackTypeGraphResult, TrackTypeGraphError> {
        let track_id = match &command.track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => {
                self.resolver.resolve_active(&command.workspace_root).map_err(removed_command)?
            }
        };
        self.operation.execute(track_id, command)
    }
}

fn removed_command(error: DiagnosticText) -> TrackTypeGraphError {
    TrackTypeGraphError::RemovedCommand(error)
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
        calls: Mutex<Vec<(TrackId, usize, TrackTypeGraphEdgeSelection, TrackLayerSelection)>>,
        error: String,
    }

    impl TrackTypeGraphPort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            command: TrackTypeGraphCommand,
        ) -> Result<TrackTypeGraphResult, TrackTypeGraphError> {
            self.calls.lock().expect("operation lock is available").push((
                track_id,
                command.cluster_depth.value(),
                command.edges,
                command.layer,
            ));
            Err(TrackTypeGraphError::RemovedCommand(DiagnosticText::new(&self.error)))
        }
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace is valid")
    }

    fn command(track: TrackSelection) -> TrackTypeGraphCommand {
        TrackTypeGraphCommand {
            track,
            items_dir: TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            workspace_root: workspace_root(),
            layer: TrackLayerSelection::One(
                domain::tddd::LayerId::try_new("usecase").expect("layer is valid"),
            ),
            cluster_depth: TrackTypeGraphClusterDepth::new(2),
            edges: TrackTypeGraphEdgeSelection::Methods,
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    fn removed_message() -> &'static str {
        "sotp track type-graph is removed in T008. Use `sotp track catalogue-impl-signals` instead."
    }

    #[test]
    fn test_track_type_graph_cluster_depth_new_returns_wrapped_value() {
        let depth = TrackTypeGraphClusterDepth::new(3);
        assert_eq!(depth.value(), 3);
    }

    #[test]
    fn test_track_type_graph_interactor_explicit_selection_forwards_without_resolution() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            error: removed_message().to_owned(),
        });
        let interactor = TrackTypeGraphInteractor::new(operation.clone(), resolver.clone());

        let error = match interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
        {
            Ok(_) => panic!("removed command must not succeed"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), removed_message());
        assert_eq!(*resolver.active_calls.lock().expect("resolver lock is available"), 0);
        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(calls.len(), 1);
        let call = calls.first().expect("one operation call is recorded");
        assert_eq!(call.0.as_ref(), "explicit-track");
        assert_eq!(call.1, 2);
        assert!(matches!(call.2, TrackTypeGraphEdgeSelection::Methods));
        assert!(matches!(call.3, TrackLayerSelection::One(_)));
    }

    #[test]
    fn test_track_type_graph_interactor_active_selection_resolves_and_forwards() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            error: removed_message().to_owned(),
        });
        let interactor = TrackTypeGraphInteractor::new(operation.clone(), resolver.clone());

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("removed command must not succeed"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), removed_message());
        assert_eq!(*resolver.active_calls.lock().expect("resolver lock is available"), 1);
        assert_eq!(
            operation
                .calls
                .lock()
                .expect("operation lock is available")
                .first()
                .expect("one call")
                .0
                .as_ref(),
            "active-track"
        );
    }

    #[test]
    fn test_track_type_graph_interactor_resolution_failure_returns_removed_command() {
        let resolver = Arc::new(RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            active_calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            error: removed_message().to_owned(),
        });
        let interactor = TrackTypeGraphInteractor::new(operation.clone(), resolver);

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("active resolution must fail"),
            Err(error) => error,
        };

        assert!(
            matches!(error, TrackTypeGraphError::RemovedCommand(message) if message.as_str() == "active track unavailable")
        );
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }

    #[test]
    fn test_track_type_graph_command_context_is_presentation_free() {
        let source = include_str!("type_graph.rs");
        let production =
            source.split("#[cfg(test)]").next().expect("production source precedes tests");
        assert!(production.contains("pub struct TrackTypeGraphCommand"));
        assert!(production.contains("pub enum TrackTypeGraphResult"));
        assert!(production.contains("pub enum TrackTypeGraphError"));
        assert!(!production.contains("[OK]"));
        assert!(!production.contains("[ERROR]"));
        assert!(!production.contains("CommandOutcome"));
        assert!(!production.contains("TrackServiceImpl"));

        fn only_err(
            result: Result<TrackTypeGraphResult, TrackTypeGraphError>,
        ) -> TrackTypeGraphError {
            match result {
                Ok(uninhabited) => match uninhabited {},
                Err(error) => error,
            }
        }

        let interactor = TrackTypeGraphInteractor::new(
            Arc::new(RecordingOperation {
                calls: Mutex::new(Vec::new()),
                error: removed_message().to_owned(),
            }),
            Arc::new(RecordingResolver {
                active: Ok(track_id("active-track")),
                active_calls: Mutex::new(0),
            }),
        );
        let error = only_err(interactor.execute(command(TrackSelection::Active)));
        assert!(matches!(error, TrackTypeGraphError::RemovedCommand(_)));
        assert_eq!(error.to_string(), removed_message());
        assert!(!error.to_string().starts_with("[ERROR]"));
        assert!(!error.to_string().starts_with("[OK]"));
    }

    #[test]
    fn test_track_type_graph_interactor_operation_failure_returns_removed_command() {
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            error: removed_message().to_owned(),
        });
        let interactor = TrackTypeGraphInteractor::new(
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
            matches!(error, TrackTypeGraphError::RemovedCommand(message) if message.as_str() == removed_message())
        );
    }
}
