//! T008: `sotp track type-graph` is deleted.
//!
//! The command depended on `TypeGraph` (now deleted) for its rendering pipeline.
//! Callers should use `sotp track catalogue-impl-signals` for type signal evaluation.
//!
//! This stub keeps the CLI compile surface intact while the command is phased out.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::CliError;

/// Stub — always returns an error explaining the command is removed.
///
/// T008: The real implementation is removed with `TypeGraph`.
///
/// # Errors
///
/// Always returns `CliError::Message` pointing to the replacement command.
pub fn execute_type_graph(
    _items_dir: PathBuf,
    _track_id: String,
    _workspace_root: PathBuf,
    _layer: Option<String>,
    _cluster_depth: usize,
    _edges: String,
) -> Result<ExitCode, CliError> {
    Err(CliError::Message(
        "sotp track type-graph is removed in T008. \
         Use `sotp track catalogue-impl-signals` instead."
            .to_owned(),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_type_graph_stub_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = execute_type_graph(
            dir.path().join("track/items"),
            "test-track".to_owned(),
            dir.path().into(),
            None,
            0,
            "methods".to_owned(),
        );
        assert!(result.is_err(), "stub must always return an error");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("T008"), "error must mention T008: {msg}");
    }
}
