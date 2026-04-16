//! `sotp track type-graph` — render a mermaid type graph from rustdoc schema export.
//!
//! Reads the target crate's public API via rustdoc JSON, builds a `TypeGraph`,
//! and renders a mermaid flowchart to `<layer>-graph.md` in the track directory.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::TrackStatus;
use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_type_graph;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::type_graph_render::{TypeGraphRenderOptions, write_type_graph_file};
use infrastructure::track::fs_store::read_track_metadata;
use infrastructure::verify::tddd_layers::TdddLayerBinding;

use crate::CliError;

use super::signals::{ensure_active_track, resolve_layers};

/// Render a mermaid type graph for each TDDD-enabled layer and write to
/// `<layer>-graph.md`.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, rustdoc export fails,
/// or the write fails.
pub fn execute_type_graph(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Active-track guard (mirrors type-signals).
    // Symlink protection for metadata read is handled inside `read_track_metadata`.
    let (metadata, doc_meta) = read_track_metadata(&items_dir, &valid_id)
        .map_err(|e| CliError::Message(format!("cannot load metadata for '{track_id}': {e}")))?;
    let effective_status = if doc_meta.original_status.as_deref() == Some("archived") {
        TrackStatus::Archived
    } else {
        metadata.status()
    };
    ensure_active_track(effective_status, &track_id)?;

    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;

    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json; nothing to render".to_owned(),
        ));
    }

    for binding in &bindings {
        execute_type_graph_for_layer(&items_dir, &track_id, &workspace_root, binding)?;
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_type_graph_for_layer(
    items_dir: &std::path::Path,
    track_id: &str,
    workspace_root: &std::path::Path,
    binding: &TdddLayerBinding,
) -> Result<ExitCode, CliError> {
    let layer_id = binding.layer_id();
    let track_dir = items_dir.join(track_id);

    let target_crate = match binding.targets() {
        [single] => single,
        [] => {
            return Err(CliError::Message(format!(
                "schema_export.targets is empty for layer '{layer_id}'; check architecture-rules.json"
            )));
        }
        multi => {
            return Err(CliError::Message(format!(
                "layer '{layer_id}' has {} schema_export.targets ({:?}), but multi-target export \
                 is not yet implemented",
                multi.len(),
                multi
            )));
        }
    };

    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let schema = exporter.export(target_crate).map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)"
        } else {
            ""
        };
        CliError::Message(format!("failed to export schema: {e}{hint}"))
    })?;

    let typestate_names = std::collections::HashSet::new();
    let profile = build_type_graph(&schema, &typestate_names);

    // Render + symlink-checked write (infrastructure layer handles the guard).
    let opts = TypeGraphRenderOptions::default();
    let graph_filename = write_type_graph_file(&profile, layer_id, &track_dir, items_dir, &opts)
        .map_err(|e| CliError::Message(format!("cannot write type graph: {e}")))?;

    println!("[OK] type-graph: wrote {graph_filename} ({layer_id})");

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_type_graph_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_type_graph(items_dir, "../evil".to_owned(), dir.path().into(), None);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_execute_type_graph_with_unknown_layer_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = r#"{
  "schema_version": 3, "id": "test-track", "branch": "track/test-track",
  "title": "Test", "status": "planned",
  "created_at": "2026-04-16T00:00:00Z", "updated_at": "2026-04-16T00:00:00Z",
  "tasks": [{"id":"T001","description":"t","status":"todo","commit_hash":null}],
  "plan": {"summary":["t"],"sections":[{"id":"S001","title":"t","description":["t"],"task_ids":["T001"]}]}
}"#;
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();

        let result = execute_type_graph(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            Some("nonexistent".to_owned()),
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("nonexistent"), "error must mention the unknown layer: {msg}");
    }

    #[test]
    fn test_execute_type_graph_rejects_done_track() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-done");
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = r#"{
  "schema_version": 3, "id": "test-done", "branch": "track/test-done",
  "title": "Done", "status": "done",
  "created_at": "2026-04-16T00:00:00Z", "updated_at": "2026-04-16T00:00:00Z",
  "tasks": [{"id":"T001","description":"t","status":"done","commit_hash":"0000000000000000000000000000000000000000"}],
  "plan": {"summary":["t"],"sections":[{"id":"S001","title":"t","description":["t"],"task_ids":["T001"]}]}
}"#;
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();

        let result = execute_type_graph(items_dir, "test-done".to_owned(), dir.path().into(), None);
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Completed tracks are frozen"), "must reject done track: {msg}");
    }
}
