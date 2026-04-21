//! `sotp track type-graph` — render a mermaid type graph from rustdoc schema export.
//!
//! Reads the target crate's public API via rustdoc JSON, builds a `TypeGraph`,
//! and renders a mermaid flowchart to the track directory.
//!
//! When `--cluster-depth N` is 0 (or omitted and the default is 0), writes a
//! single flat file `<layer>-graph.md`. When N ≥ 1 (default 2), writes a
//! cluster directory `<layer>-graph/` with `index.md` + per-cluster files.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::ImplPlanReader;
use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_type_graph;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::type_graph_render::{
    EdgeSet, TypeGraphRenderOptions, write_type_graph_dir, write_type_graph_file,
};
use infrastructure::track::fs_store::{FsTrackStore, read_track_metadata};
use infrastructure::verify::tddd_layers::TdddLayerBinding;

use crate::CliError;

use super::signals::{ensure_active_track, resolve_layers};

/// Parses the `--edges` CLI flag value into an `EdgeSet`.
///
/// Accepted values: `"methods"`, `"fields"`, `"impls"`, `"all"` (case-insensitive).
///
/// # Errors
///
/// Returns `CliError::Message` when the value is not one of the accepted tokens.
fn parse_edge_set(value: &str) -> Result<EdgeSet, CliError> {
    match value.to_lowercase().as_str() {
        "methods" => Ok(EdgeSet::Methods),
        "fields" => Ok(EdgeSet::Fields),
        "impls" => Ok(EdgeSet::Impls),
        "all" => Ok(EdgeSet::All),
        other => Err(CliError::Message(format!(
            "unknown --edges value '{other}'; expected one of: methods, fields, impls, all"
        ))),
    }
}

/// Render a mermaid type graph for each TDDD-enabled layer.
///
/// When `cluster_depth` is 0 writes `<layer>-graph.md` (flat mode).
/// When `cluster_depth` ≥ 1 writes `<layer>-graph/` directory layout.
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
    cluster_depth: usize,
    edges: String,
) -> Result<ExitCode, CliError> {
    let valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Active-track guard (mirrors type-signals).
    // Symlink protection for metadata read is handled inside `read_track_metadata`.
    let (metadata, _doc_meta) = read_track_metadata(&items_dir, &valid_id)
        .map_err(|e| CliError::Message(format!("cannot load metadata for '{track_id}': {e}")))?;
    // Status is derived on demand from impl-plan + status_override.
    // Use FsTrackStore::load_impl_plan (fail-closed) so a corrupt impl-plan.json
    // blocks the guard instead of being silently treated as absent.
    let store = FsTrackStore::new(items_dir.clone());
    let impl_plan = store
        .load_impl_plan(&valid_id)
        .map_err(|e| CliError::Message(format!("cannot load impl-plan for '{track_id}': {e}")))?;
    // Fail-closed: an activated track (branch set, or non-Planned derived status) with
    // no impl-plan is potentially corrupt — reject rather than treating as Planned.
    let effective_status =
        domain::derive_track_status(impl_plan.as_ref(), metadata.status_override());
    if impl_plan.is_none()
        && (metadata.branch().is_some() || effective_status != domain::TrackStatus::Planned)
    {
        return Err(CliError::Message(format!(
            "cannot run type-graph on '{track_id}': track has no impl-plan.json but is \
             not in planning state (derived_status={effective_status}); track may be corrupt"
        )));
    }
    ensure_active_track(effective_status, &track_id)?;

    let edge_set = parse_edge_set(&edges)?;
    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;

    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json; nothing to render".to_owned(),
        ));
    }

    for binding in &bindings {
        execute_type_graph_for_layer(
            &items_dir,
            &track_id,
            &workspace_root,
            binding,
            cluster_depth,
            edge_set,
        )?;
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_type_graph_for_layer(
    items_dir: &std::path::Path,
    track_id: &str,
    workspace_root: &std::path::Path,
    binding: &TdddLayerBinding,
    cluster_depth: usize,
    edge_set: EdgeSet,
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
    let opts =
        TypeGraphRenderOptions { cluster_depth, edge_set, ..TypeGraphRenderOptions::default() };

    match select_write_mode(cluster_depth) {
        WriteMode::Flat => {
            let graph_filename =
                write_type_graph_file(&profile, layer_id, &track_dir, items_dir, &opts)
                    .map_err(|e| CliError::Message(format!("cannot write type graph: {e}")))?;
            println!("[OK] type-graph: wrote {graph_filename} ({layer_id})");
        }
        WriteMode::Cluster => {
            let written = write_type_graph_dir(&profile, layer_id, &track_dir, items_dir, &opts)
                .map_err(|e| CliError::Message(format!("cannot write type graph dir: {e}")))?;
            for path in &written {
                println!("[OK] type-graph: wrote {path} ({layer_id})");
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// Output mode selected by the `--cluster-depth` flag.
///
/// Extracted from `execute_type_graph_for_layer` so the dispatch predicate can
/// be unit-tested without spinning up rustdoc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteMode {
    Flat,
    Cluster,
}

/// Selects the write mode for a given `cluster_depth`.
///
/// - `cluster_depth == 0` → [`WriteMode::Flat`] (single `<layer>-graph.md` file)
/// - `cluster_depth >= 1` → [`WriteMode::Cluster`] (`<layer>-graph/` directory)
#[must_use]
fn select_write_mode(cluster_depth: usize) -> WriteMode {
    if cluster_depth == 0 { WriteMode::Flat } else { WriteMode::Cluster }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // Default cluster_depth for CLI unit tests: use 0 (flat mode) to avoid
    // touching architecture-rules.json or nightly rustdoc in unit test context.
    const TEST_CLUSTER_DEPTH: usize = 0;
    // Default edges value for CLI unit tests: "methods" (the default).
    const TEST_EDGES: &str = "methods";

    #[test]
    fn test_execute_type_graph_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_type_graph(
            items_dir,
            "../evil".to_owned(),
            dir.path().into(),
            None,
            TEST_CLUSTER_DEPTH,
            TEST_EDGES.to_owned(),
        );
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_execute_type_graph_with_unknown_layer_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // v5 format, no status field
        let metadata = r#"{
  "schema_version": 5, "id": "test-track", "branch": "track/test-track",
  "title": "Test",
  "created_at": "2026-04-16T00:00:00Z", "updated_at": "2026-04-16T00:00:00Z"
}"#;
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        // Provide a minimal impl-plan.json so the activated-track guard passes
        // and the test reaches the layer-name validation step.
        let impl_plan = r#"{
  "schema_version": 1,
  "tasks": [],
  "plan": { "summary": [], "sections": [] }
}"#;
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();

        let result = execute_type_graph(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            Some("nonexistent".to_owned()),
            TEST_CLUSTER_DEPTH,
            TEST_EDGES.to_owned(),
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("nonexistent"), "error must mention the unknown layer: {msg}");
    }

    // --- select_write_mode (pure dispatch predicate) ---

    #[test]
    fn test_select_write_mode_zero_selects_flat() {
        assert_eq!(select_write_mode(0), WriteMode::Flat);
    }

    #[test]
    fn test_select_write_mode_one_selects_cluster() {
        assert_eq!(select_write_mode(1), WriteMode::Cluster);
    }

    #[test]
    fn test_select_write_mode_default_depth_two_selects_cluster() {
        // Guards against a regression where the default cluster mode
        // (TypeGraphRenderOptions::default().cluster_depth == 2) would fall
        // into the flat branch. The `--cluster-depth` CLI flag defaults to 2.
        assert_eq!(select_write_mode(2), WriteMode::Cluster);
    }

    #[test]
    fn test_select_write_mode_large_depth_selects_cluster() {
        // Any non-zero depth selects cluster mode.
        assert_eq!(select_write_mode(10), WriteMode::Cluster);
    }

    /// Integration test for the cluster_depth dispatch (flat vs directory mode).
    ///
    /// Requires nightly toolchain for `cargo +nightly rustdoc`. Run with:
    /// `cargo test --package cli -- --ignored`
    ///
    /// Exercises both dispatch branches of `execute_type_graph_for_layer`:
    /// 1. `cluster_depth = 2` (default) writes `<layer>-graph/` directory with `index.md` + cluster files
    /// 2. `cluster_depth = 0` writes flat `<layer>-graph.md` and removes the stale cluster directory
    ///
    /// Guards against regressions in the dispatch branch AND the stale-file cleanup.
    #[test]
    #[ignore]
    fn test_execute_type_graph_cluster_depth_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_id = "test-dispatch";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = format!(
            r#"{{
  "schema_version": 3, "id": "{track_id}", "branch": "track/{track_id}",
  "title": "Test dispatch", "status": "in_progress",
  "created_at": "2026-04-17T00:00:00Z", "updated_at": "2026-04-17T00:00:00Z",
  "tasks": [{{"id":"T001","description":"t","status":"in_progress","commit_hash":null}}],
  "plan": {{"summary":["t"],"sections":[{{"id":"S001","title":"t","description":["t"],"task_ids":["T001"]}}]}}
}}"#
        );
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();

        // workspace_root must point to the real workspace so rustdoc can find the domain crate.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();

        // Branch 1: cluster_depth = 2 → directory layout written
        let result = execute_type_graph(
            items_dir.clone(),
            track_id.to_owned(),
            workspace_root.clone(),
            Some("domain".to_owned()),
            2,
            TEST_EDGES.to_owned(),
        );
        assert!(result.is_ok(), "cluster_depth=2 must succeed: {result:?}");
        let cluster_dir = track_dir.join("domain-graph");
        assert!(cluster_dir.is_dir(), "cluster_depth=2 must create <layer>-graph/ directory");
        assert!(cluster_dir.join("index.md").is_file(), "cluster mode must write index.md");

        // Branch 2: cluster_depth = 0 → flat file + stale cluster dir cleanup
        let result = execute_type_graph(
            items_dir.clone(),
            track_id.to_owned(),
            workspace_root,
            Some("domain".to_owned()),
            0,
            TEST_EDGES.to_owned(),
        );
        assert!(result.is_ok(), "cluster_depth=0 must succeed: {result:?}");
        assert!(track_dir.join("domain-graph.md").is_file(), "flat mode must write .md file");
        assert!(!cluster_dir.exists(), "flat mode must remove the stale <layer>-graph/ directory");
    }

    #[test]
    fn test_execute_type_graph_rejects_done_track() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-done");
        std::fs::create_dir_all(&track_dir).unwrap();

        // v5 format, no status field. All tasks done → derive_track_status → Done.
        let metadata = r#"{
  "schema_version": 5, "id": "test-done", "branch": "track/test-done",
  "title": "Done",
  "created_at": "2026-04-16T00:00:00Z", "updated_at": "2026-04-16T00:00:00Z"
}"#;
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        let impl_plan = r#"{"schema_version":1,"tasks":[{"id":"T001","description":"t","status":"done","commit_hash":"abc1234"}],"plan":{"summary":[],"sections":[{"id":"S001","title":"t","description":[],"task_ids":["T001"]}]}}"#;
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();

        let result = execute_type_graph(
            items_dir,
            "test-done".to_owned(),
            dir.path().into(),
            None,
            TEST_CLUSTER_DEPTH,
            TEST_EDGES.to_owned(),
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Completed tracks are frozen"), "must reject done track: {msg}");
    }

    // --- parse_edge_set ---

    #[test]
    fn test_parse_edge_set_methods_succeeds() {
        assert_eq!(parse_edge_set("methods").unwrap(), EdgeSet::Methods);
    }

    #[test]
    fn test_parse_edge_set_fields_succeeds() {
        assert_eq!(parse_edge_set("fields").unwrap(), EdgeSet::Fields);
    }

    #[test]
    fn test_parse_edge_set_impls_succeeds() {
        assert_eq!(parse_edge_set("impls").unwrap(), EdgeSet::Impls);
    }

    #[test]
    fn test_parse_edge_set_all_succeeds() {
        assert_eq!(parse_edge_set("all").unwrap(), EdgeSet::All);
    }

    #[test]
    fn test_parse_edge_set_case_insensitive() {
        assert_eq!(parse_edge_set("METHODS").unwrap(), EdgeSet::Methods);
        assert_eq!(parse_edge_set("Fields").unwrap(), EdgeSet::Fields);
        assert_eq!(parse_edge_set("ALL").unwrap(), EdgeSet::All);
    }

    #[test]
    fn test_parse_edge_set_unknown_value_returns_error() {
        let result = parse_edge_set("unknown");
        assert!(result.is_err(), "unknown value must return error");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("unknown"), "error must mention the bad value: {msg}");
        // Regression guard: error must list every accepted option so users know the full set.
        for expected in ["methods", "fields", "impls", "all"] {
            assert!(
                msg.contains(expected),
                "error must list '{expected}' as a valid option: {msg}"
            );
        }
    }
}
