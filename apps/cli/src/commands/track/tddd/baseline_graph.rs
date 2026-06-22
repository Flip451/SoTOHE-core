//! `sotp track baseline-graph` — render the rustdoc-input baseline graph (Reality View).
//!
//! Thin CLI adapter: delegates all orchestration to [`cli_composition::CliApp`].

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::CliApp;

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
    let outcome = CliApp::new()
        .track_baseline_graph(items_dir, Some(track_id), workspace_root, layers)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
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
}
