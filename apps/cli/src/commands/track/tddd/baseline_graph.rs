//! `sotp track baseline-graph` — render the rustdoc-input baseline graph (Reality View).
//!
//! Thin CLI adapter: delegates all orchestration to the composition root in `cli_composition`.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackItemsDirectoryInput, TrackLayersInput, TrackTdddBaselineGraphInput,
    TrackWorkspaceRootInput,
};

use crate::CliError;

/// Render the baseline graph (Reality View) for a single track.
///
/// # Errors
///
/// Returns `CliError` when the underlying `CliApp` composition fails.
pub fn execute_baseline_graph(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layers: Option<String>,
) -> Result<ExitCode, CliError> {
    let track_id =
        track_id.parse::<TrackIdInput>().map_err(|error| CliError::Message(error.to_string()))?;
    let items_dir = TrackItemsDirectoryInput::try_new(items_dir)
        .map_err(|error| CliError::Message(error.to_string()))?;
    let workspace_root =
        TrackWorkspaceRootInput::try_from(workspace_root).map_err(CliError::Message)?;
    let layers = layers
        .map(TrackLayersInput::try_new)
        .transpose()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let outcome = TrackCompositionRoot::new().track_tddd_driver().handle_baseline_graph(
        TrackTdddBaselineGraphInput { track_id: Some(track_id), items_dir, workspace_root, layers },
    );
    super::super::state_ops::track_driver_outcome_to_result(outcome)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Verifies that a malformed track ID is rejected before git discovery.
    #[test]
    fn test_execute_baseline_graph_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result =
            execute_baseline_graph(items_dir, "../evil".to_owned(), dir.path().into(), None);
        let err = result.expect_err("path traversal track id must be rejected");
        let msg = format!("{err}");
        // Error text is the domain form: "track id '...' must be a lowercase slug".
        // Accept either the domain form or legacy "invalid" prefix (behaviour: rejection).
        assert!(
            msg.contains("must be a lowercase slug")
                || msg.contains("invalid track ID")
                || msg.contains("invalid"),
            "error must reject invalid track id, got: {msg}"
        );
    }

    #[test]
    fn test_execute_baseline_graph_with_invalid_layer_returns_error_before_execution() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_baseline_graph(
            items_dir,
            "graph-track".to_owned(),
            dir.path().into(),
            Some("not a layer".to_owned()),
        );
        let err = result.expect_err("invalid layer must be rejected");

        assert!(err.to_string().contains("invalid layer id"));
    }
}
