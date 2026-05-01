//! Per-layer type graph export and render logic.
//!
//! Moved from the CLI layer so that the CLI composition root never imports
//! `domain::schema::SchemaExporter` or `domain::schema::SchemaExportError`
//! directly (CN-01 / AC-03).

use std::collections::HashSet;
use std::path::Path;
use std::process::ExitCode;

use domain::schema::{SchemaExportError, SchemaExporter};

use crate::code_profile_builder::build_type_graph;
use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::type_graph_render::{
    EdgeSet, TypeGraphRenderOptions, write_type_graph_dir, write_type_graph_file,
};
use crate::verify::tddd_layers::TdddLayerBinding;

/// Validate that `path` is not itself a symlink, for use as a trusted root.
///
/// Returns `Err(String)` with a descriptive message when the path is a symlink
/// or its metadata cannot be read.
fn guard_root_not_symlink(path: &Path, label: &str) -> Result<(), TypeGraphExportError> {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(TypeGraphExportError(format!(
            "symlink guard: refusing to use symlinked {label}: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) => Err(TypeGraphExportError(format!(
            "symlink guard: cannot stat {label} {}: {e}",
            path.display()
        ))),
    }
}

/// Error type returned by [`execute_type_graph_for_layer`] so the CLI can
/// map it to `CliError::Message` without importing domain types.
#[derive(Debug)]
pub struct TypeGraphExportError(pub String);

impl std::fmt::Display for TypeGraphExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Output mode selected by the `--cluster-depth` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteMode {
    Flat,
    Cluster,
}

fn select_write_mode(cluster_depth: usize) -> WriteMode {
    if cluster_depth == 0 { WriteMode::Flat } else { WriteMode::Cluster }
}

/// Render a mermaid type graph for a single TDDD-enabled layer binding.
///
/// `items_dir` is the trusted root for symlink guards.
/// `track_id` is accepted as `&str` so the CLI never imports domain types.
///
/// # Errors
///
/// Returns [`TypeGraphExportError`] when rustdoc export fails, targets are
/// misconfigured, or the file/directory write fails.
pub fn execute_type_graph_for_layer(
    items_dir: &Path,
    track_id: &str,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    cluster_depth: usize,
    edge_set: EdgeSet,
) -> Result<ExitCode, TypeGraphExportError> {
    let err = |s: String| TypeGraphExportError(s);

    // Security: guard root directories themselves before using them as trusted roots.
    // `reject_symlinks_below`/write-path guards only inspect descendants — a symlinked
    // root would bypass them (fail-closed per ADR §D7).
    guard_root_not_symlink(items_dir, "items_dir")?;
    guard_root_not_symlink(workspace_root, "workspace_root")?;

    // Containment: verify items_dir resolves under workspace_root.
    // This prevents directory-traversal (`..`) bypasses even when no symlinks are involved.
    let canonical_workspace = workspace_root.canonicalize().map_err(|e| {
        err(format!("cannot canonicalize workspace_root {}: {e}", workspace_root.display()))
    })?;
    let canonical_items = items_dir
        .canonicalize()
        .map_err(|e| err(format!("cannot canonicalize items_dir {}: {e}", items_dir.display())))?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(err(format!(
            "items_dir '{}' is outside workspace_root '{}'. Only paths under the workspace are allowed.",
            items_dir.display(),
            workspace_root.display()
        )));
    }

    let layer_id = binding.layer_id();

    // Security: validate track_id via domain::TrackId before joining onto items_dir.
    // `Path::join` resolves `..`, `/`, and multi-segment paths (`foo/bar`) at the OS
    // level. Using the domain type enforces the slug rules (single-segment, no `..`,
    // no path separators) and makes this function safe when called directly without
    // upstream CLI validation.
    let valid_track_id =
        domain::TrackId::try_new(track_id).map_err(|e| err(format!("invalid track_id: {e}")))?;

    let track_dir = items_dir.join(valid_track_id.as_ref());

    let target_crate = match binding.targets() {
        [single] => single,
        [] => {
            return Err(err(format!(
                "schema_export.targets is empty for layer '{layer_id}'; check architecture-rules.json"
            )));
        }
        multi => {
            return Err(err(format!(
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
        err(format!("failed to export schema: {e}{hint}"))
    })?;

    let typestate_names = HashSet::new();
    let profile = build_type_graph(&schema, &typestate_names);

    let opts =
        TypeGraphRenderOptions { cluster_depth, edge_set, ..TypeGraphRenderOptions::default() };

    match select_write_mode(cluster_depth) {
        WriteMode::Flat => {
            let graph_filename =
                write_type_graph_file(&profile, layer_id, &track_dir, items_dir, &opts)
                    .map_err(|e| err(format!("cannot write type graph: {e}")))?;
            println!("[OK] type-graph: wrote {graph_filename} ({layer_id})");
        }
        WriteMode::Cluster => {
            let written = write_type_graph_dir(&profile, layer_id, &track_dir, items_dir, &opts)
                .map_err(|e| err(format!("cannot write type graph dir: {e}")))?;
            for path in &written {
                println!("[OK] type-graph: wrote {path} ({layer_id})");
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}
