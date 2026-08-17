//! System adapter for the Track TDDD baseline-graph port.

use std::path::Path;

use domain::TrackId;
use usecase::baseline_graph_workflow::{
    RenderBaselineGraph, RenderBaselineGraphCommand, RenderBaselineGraphInteractor,
};
use usecase::track_lifecycle::TrackLayerFilter;
use usecase::track_lifecycle::tddd::baseline_graph::{
    TrackBaselineGraphCommand, TrackBaselineGraphError, TrackBaselineGraphPort,
    TrackBaselineGraphResult,
};

/// System-backed adapter for the Track TDDD baseline-graph operation.
pub struct SystemTrackBaselineGraphAdapter;

impl TrackBaselineGraphPort for SystemTrackBaselineGraphAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackBaselineGraphCommand,
    ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> {
        let items_dir = command.items_dir.as_path().to_path_buf();
        let workspace_root = command.workspace_root.as_path().to_path_buf();
        validate_items_dir_within_workspace(&items_dir, &workspace_root)
            .map_err(|error| execution_failed(format!("baseline-graph render failed: {error}")))?;
        let layer_filter = match command.layers {
            TrackLayerFilter::All => None,
            TrackLayerFilter::Selected(layers) => Some(layers),
        };
        let loader = crate::tddd::baseline_graph_loader_adapter::BaselineGraphLoaderAdapter::new(
            items_dir.clone(),
            workspace_root.join("architecture-rules.json"),
            workspace_root.clone(),
        );
        let renderer =
            crate::tddd::baseline_graph_renderer_adapter::BaselineGraphRendererAdapter::new(
                workspace_root.join(".harness/config/baseline-graph-style.toml"),
            );
        let writer = crate::tddd::baseline_graph_writer_adapter::BaselineGraphWriterAdapter::new(
            items_dir,
            workspace_root.clone(),
        );
        let workflow = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let result = workflow
            .execute(&RenderBaselineGraphCommand { track_id: track_id.clone(), layer_filter })
            .map_err(|error| execution_failed(format!("baseline-graph render failed: {error}")))?;

        Ok(TrackBaselineGraphResult {
            track_id,
            rendered_layers: usecase::track_lifecycle::TrackRenderedLayerCount::new(
                result.rendered_layer_count,
            ),
            written_files: usecase::track_lifecycle::TrackWrittenFileCount::new(
                result.written_file_count,
            ),
        })
    }
}

fn execution_failed(message: impl Into<String>) -> TrackBaselineGraphError {
    TrackBaselineGraphError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(message))
}

fn validate_items_dir_within_workspace(
    items_dir: &Path,
    workspace_root: &Path,
) -> Result<(), String> {
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        format!("cannot resolve workspace root '{}': {error}", workspace_root.display())
    })?;
    let canonical_items = items_dir.canonicalize().map_err(|error| {
        format!("cannot resolve track items directory '{}': {error}", items_dir.display())
    })?;

    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(format!(
            "track items directory '{}' resolves outside workspace root '{}'; only paths under the workspace are allowed",
            items_dir.display(),
            workspace_root.display()
        ));
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;

    use super::*;
    use rustdoc_types::FORMAT_VERSION;
    use usecase::track_lifecycle::{TrackItemsDirectory, TrackSelection, TrackWorkspaceRoot};

    fn minimal_rustdoc_json() -> String {
        format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {FORMAT_VERSION},
                "target": {{"triple": "", "target_features": []}}
            }}"#
        )
    }

    fn write_valid_fixture(root: &std::path::Path) {
        fs::create_dir_all(root.join(".harness/config")).expect("style config directory exists");
        fs::write(
            root.join(".harness/config/baseline-graph-style.toml"),
            "[filter]\ninclude_functions = true\n",
        )
        .expect("style config is written");
        fs::write(
            root.join("architecture-rules.json"),
            r#"{
              "version": 2,
              "layers": [
                {
                  "crate": "domain",
                  "path": "libs/domain",
                  "may_depend_on": [],
                  "deny_reason": "no reverse dep",
                  "tddd": {
                    "enabled": true,
                    "catalogue_file": "domain-types.json",
                    "schema_export": {"method": "rustdoc", "targets": ["domain"]}
                  }
                },
                {
                  "crate": "usecase",
                  "path": "libs/usecase",
                  "may_depend_on": ["domain"],
                  "deny_reason": "no reverse dep",
                  "tddd": {
                    "enabled": true,
                    "catalogue_file": "usecase-types.json",
                    "schema_export": {"method": "rustdoc", "targets": ["usecase"]}
                  }
                }
              ]
            }"#,
        )
        .expect("architecture rules are written");

        let track_dir = root.join("track/items/graph-track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        for baseline in ["domain-types-baseline.json", "usecase-types-baseline.json"] {
            fs::write(track_dir.join(baseline), minimal_rustdoc_json())
                .expect("rustdoc baseline is written");
        }
    }

    fn command(root: &std::path::Path) -> TrackBaselineGraphCommand {
        TrackBaselineGraphCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("graph-track").expect("track id is valid"),
            ),
            items_dir: TrackItemsDirectory::try_new(root.join("track/items"))
                .expect("items directory is valid"),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            layers: TrackLayerFilter::All,
        }
    }

    #[test]
    fn test_system_track_baseline_graph_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/graph-track"))
            .expect("track directory exists");

        let error = SystemTrackBaselineGraphAdapter
            .execute(
                TrackId::try_new("graph-track").expect("track id is valid"),
                command(workspace.path()),
            )
            .expect_err("missing architecture rules must fail");

        assert!(error.to_string().contains("baseline-graph render failed"));
    }

    #[test]
    fn test_system_track_baseline_graph_adapter_items_dir_outside_workspace_returns_execution_error()
     {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let outside = tempfile::tempdir().expect("outside workspace exists");
        let outside_items = outside.path().join("track/items");
        fs::create_dir_all(outside_items.join("graph-track")).expect("track directory exists");

        let mut graph_command = command(workspace.path());
        graph_command.items_dir = TrackItemsDirectory::try_new(outside_items)
            .expect("outside items directory uses the conventional suffix");

        let error = SystemTrackBaselineGraphAdapter
            .execute(TrackId::try_new("graph-track").expect("track id is valid"), graph_command)
            .expect_err("items directory outside workspace must fail");

        assert!(error.to_string().contains("outside workspace root"));
    }

    #[test]
    fn test_system_track_baseline_graph_adapter_valid_selected_layer_returns_summary_and_writes_output()
     {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        write_valid_fixture(workspace.path());

        let mut graph_command = command(workspace.path());
        graph_command.layers = TrackLayerFilter::Selected(vec![
            domain::tddd::LayerId::try_new("domain").expect("layer id is valid"),
        ]);
        let result = SystemTrackBaselineGraphAdapter
            .execute(TrackId::try_new("graph-track").expect("track id is valid"), graph_command)
            .expect("valid baseline graph fixture renders");

        assert_eq!(result.track_id.as_ref(), "graph-track");
        assert_eq!(result.rendered_layers.value(), 1);
        assert_eq!(result.written_files.value(), 1);

        let track_dir = workspace.path().join("track/items/graph-track");
        let overview = track_dir.join("domain-graph-d1/index.md");
        assert!(overview.is_file(), "selected layer overview must be written");
        assert!(
            fs::read_to_string(&overview).expect("overview is readable").contains("flowchart LR")
        );
        assert!(
            !track_dir.join("usecase-graph-d1").exists(),
            "filtered-out layer must not produce output"
        );
    }
}
