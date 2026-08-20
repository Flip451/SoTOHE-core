//! System adapter for the removed Track TDDD type-graph port.

use domain::TrackId;
use usecase::git_workflow::DiagnosticText;
use usecase::track_lifecycle::tddd::type_graph::{
    TrackTypeGraphCommand, TrackTypeGraphError, TrackTypeGraphPort, TrackTypeGraphResult,
};

const REMOVED_COMMAND: &str =
    "sotp track type-graph is removed in T008. Use `sotp track catalogue-impl-signals` instead.";

/// System-backed adapter for the removed TDDD type-graph command.
pub struct SystemTrackTypeGraphAdapter;

impl TrackTypeGraphPort for SystemTrackTypeGraphAdapter {
    fn execute(
        &self,
        _track_id: TrackId,
        _command: TrackTypeGraphCommand,
    ) -> Result<TrackTypeGraphResult, TrackTypeGraphError> {
        Err(TrackTypeGraphError::RemovedCommand(DiagnosticText::new(REMOVED_COMMAND)))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use usecase::track_lifecycle::tddd::type_graph::{
        TrackTypeGraphClusterDepth, TrackTypeGraphEdgeSelection,
    };
    use usecase::track_lifecycle::{
        TrackItemsDirectory, TrackLayerSelection, TrackSelection, TrackWorkspaceRoot,
    };

    fn command() -> TrackTypeGraphCommand {
        TrackTypeGraphCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("graph-track").expect("track id is valid"),
            ),
            items_dir: TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            workspace_root: TrackWorkspaceRoot::try_new(PathBuf::from("workspace"))
                .expect("workspace is valid"),
            layer: TrackLayerSelection::All,
            cluster_depth: TrackTypeGraphClusterDepth::new(0),
            edges: TrackTypeGraphEdgeSelection::Methods,
        }
    }

    #[test]
    fn test_system_track_type_graph_adapter_always_returns_removed_command() {
        let error = match SystemTrackTypeGraphAdapter
            .execute(TrackId::try_new("graph-track").expect("track id is valid"), command())
        {
            Ok(_) => panic!("removed type-graph command must not succeed"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), REMOVED_COMMAND);
    }

    #[test]
    fn test_system_track_type_graph_adapter_has_no_shim_or_reverse_delegation() {
        let source = include_str!("type_graph.rs");
        let production =
            source.split("#[cfg(test)]").next().expect("production source precedes tests");
        assert!(production.contains("impl TrackTypeGraphPort for SystemTrackTypeGraphAdapter"));
        assert!(!production.contains("TrackServiceImpl"));
        assert!(!production.contains(".handle("));
        assert!(!production.contains("compatibility shim"));
        assert!(!production.contains("composition_root"));
    }
}
