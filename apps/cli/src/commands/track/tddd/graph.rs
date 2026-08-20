//! `sotp track type-graph` — removed in T008.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackItemsDirectoryInput, TrackLayerInput, TrackTdddTypeGraphInput,
    TrackTypeGraphClusterDepthInput, TrackTypeGraphEdgeInput, TrackWorkspaceRootInput,
};

use crate::CliError;

/// Execute the removed type-graph command through the TDDD driver.
///
/// Signature is kept identical to the rustdoc baseline stub. The command
/// context always fails with `RemovedCommand`. The driver owns stderr/exit
/// mapping; this handler preserves the T008 recovery wording.
///
/// # Errors
///
/// Returns `CliError::Message` when input validation fails or the removed
/// command context reports `RemovedCommand`.
pub fn execute_type_graph(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
    cluster_depth: usize,
    edges: String,
) -> Result<ExitCode, CliError> {
    let track_id = track_id
        .parse::<TrackIdInput>()
        .map_err(|error| CliError::Message(format!("invalid track id: {error}")))?;
    let items_dir = TrackItemsDirectoryInput::try_new(items_dir)
        .map_err(|error| CliError::Message(error.to_string()))?;
    let workspace_root =
        TrackWorkspaceRootInput::try_from(workspace_root).map_err(CliError::Message)?;
    let layer = layer
        .map(TrackLayerInput::try_from)
        .transpose()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let edges = match edges.as_str() {
        "methods" => TrackTypeGraphEdgeInput::Methods,
        "fields" => TrackTypeGraphEdgeInput::Fields,
        "impls" => TrackTypeGraphEdgeInput::Impls,
        "all" => TrackTypeGraphEdgeInput::All,
        other => {
            return Err(CliError::Message(format!("invalid type-graph edges '{other}'")));
        }
    };
    let outcome = TrackCompositionRoot::new().track_tddd_driver().handle_type_graph(
        TrackTdddTypeGraphInput {
            track_id: Some(track_id),
            items_dir,
            workspace_root,
            layer,
            cluster_depth: TrackTypeGraphClusterDepthInput::new(cluster_depth),
            edges,
        },
    );
    super::emit_driver_outcome!(outcome, &mut std::io::stdout(), &mut std::io::stderr())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_type_graph_removed_command_returns_t008_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let argv_items = items_dir.clone();
        let argv_track_id = "test-track".to_owned();
        let argv_workspace = dir.path().to_path_buf();
        let argv_layer = Option::<String>::None;
        let argv_cluster_depth = 0usize;
        let argv_edges = "methods".to_owned();

        let result = execute_type_graph(
            argv_items.clone(),
            argv_track_id.clone(),
            argv_workspace.clone(),
            argv_layer.clone(),
            argv_cluster_depth,
            argv_edges.clone(),
        );

        let msg = result.expect_err("removed type-graph command must fail").to_string();
        assert!(msg.contains("T008"), "error must mention T008: {msg}");
        assert!(
            msg.contains("catalogue-impl-signals"),
            "error must mention the replacement command: {msg}"
        );
        assert_eq!(argv_items, items_dir);
        assert_eq!(argv_track_id, "test-track");
        assert_eq!(argv_workspace, dir.path());
        assert_eq!(argv_cluster_depth, 0);
        assert_eq!(argv_edges, "methods");
        assert!(
            !dir.path().join("track/items/test-track").exists(),
            "removed type-graph command must not persist artifacts"
        );
    }

    #[test]
    fn test_execute_type_graph_rejects_invalid_track_id_before_execution() {
        let result = execute_type_graph(
            PathBuf::from("workspace/track/items"),
            "../escape".to_owned(),
            PathBuf::from("workspace"),
            None,
            0,
            "methods".to_owned(),
        );

        let msg = result.expect_err("invalid track id must be rejected").to_string();
        assert!(msg.contains("invalid track id"), "got: {msg}");
    }
}
