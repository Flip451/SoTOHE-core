//! Baseline capture logic for a single TDDD layer binding.
//!
//! Moved from the CLI layer so that the CLI composition root never imports
//! `domain::schema::SchemaExporter` or `domain::schema::SchemaExportError`
//! directly (CN-01 / AC-03).

use std::collections::HashSet;
use std::path::Path;

use domain::schema::{SchemaExportError, SchemaExporter};

use crate::code_profile_builder::build_type_graph;
use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::{baseline_builder, baseline_codec, catalogue_codec};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;

/// Error type returned by [`capture_baseline_for_layer`] so the CLI can map it
/// to `CliError::Message` without importing domain types.
#[derive(Debug)]
pub struct CaptureBaselineError(pub String);

impl std::fmt::Display for CaptureBaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Capture the baseline for a single TDDD layer binding.
///
/// Handles the skip-if-exists logic, symlink guards, rustdoc export,
/// TypeGraph build, and atomic write for a single layer.
///
/// `items_dir` is the trusted root for symlink guards.
/// `track_id` is accepted as `&str` so the CLI never imports domain types.
///
/// # Errors
///
/// Returns [`CaptureBaselineError`] when the schema export fails, the
/// catalogue is malformed, the baseline cannot be written, or any
/// path-based security guard rejects the access.
pub fn capture_baseline_for_layer(
    items_dir: &Path,
    track_id: &str,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
) -> Result<(), CaptureBaselineError> {
    let err = |s: String| CaptureBaselineError(s);

    // Security: guard root directories themselves before using them as trusted roots.
    // `reject_symlinks_below` only inspects descendants — a symlinked root would bypass it.
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(err(format!(
                "symlink guard: refusing to use symlinked items_dir: {}",
                items_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(err(format!(
                "symlink guard: cannot stat items_dir {}: {e}",
                items_dir.display()
            )));
        }
    }
    match workspace_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(err(format!(
                "symlink guard: refusing to use symlinked workspace_root: {}",
                workspace_root.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(err(format!(
                "symlink guard: cannot stat workspace_root {}: {e}",
                workspace_root.display()
            )));
        }
    }

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

    let catalogue_filename = binding.catalogue_file();
    let baseline_filename = binding.baseline_file();

    // Security: validate track_id via domain::TrackId before joining onto items_dir.
    // `Path::join` resolves `..`, `/`, and multi-segment paths (`foo/bar`) at the OS
    // level. Using the domain type enforces the slug rules (single-segment, no `..`,
    // no path separators) and makes this function safe when called directly without
    // upstream CLI validation.
    let valid_track_id =
        domain::TrackId::try_new(track_id).map_err(|e| err(format!("invalid track_id: {e}")))?;

    let track_dir = items_dir.join(valid_track_id.as_ref());
    let baseline_path = track_dir.join(&baseline_filename);

    // Security: reject symlinks in path components below items_dir.
    reject_symlinks_below(&baseline_path, items_dir)
        .map_err(|e| err(format!("symlink guard: {e}")))?;

    // Idempotent: skip if baseline already exists as a regular file.
    if baseline_path.is_file() {
        println!(
            "[OK] baseline-capture: {baseline_filename} already exists for '{track_id}' (delete the file manually to re-capture)"
        );
        return Ok(());
    }

    // Fail fast if the track directory does not exist.
    if !track_dir.is_dir() {
        return Err(err(format!(
            "track directory not found: {} (did you mean an existing track ID?)",
            track_dir.display()
        )));
    }

    // Read the configured catalogue file to extract typestate names.
    let catalogue_path = track_dir.join(catalogue_filename);
    reject_symlinks_below(&catalogue_path, items_dir)
        .map_err(|e| err(format!("symlink guard: {e}")))?;
    let typestate_names: HashSet<String> = match std::fs::read_to_string(&catalogue_path) {
        Ok(json) => {
            catalogue_codec::decode(&json).map(|doc| doc.typestate_names()).map_err(|e| {
                err(format!(
                    "{} is malformed; fix or delete it before capturing baseline: {e}",
                    catalogue_path.display()
                ))
            })?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashSet::new(),
        Err(e) => {
            return Err(err(format!("cannot read {}: {e}", catalogue_path.display())));
        }
    };

    // Resolve the target crate for schema export from the binding.
    let layer_id = binding.layer_id();
    let target_crate = match binding.targets() {
        [single] => single,
        [] => {
            return Err(err(format!(
                "schema_export.targets is empty for layer '{layer_id}'; check architecture-rules.json"
            )));
        }
        multi => {
            return Err(err(format!(
                "layer '{layer_id}' has {} schema_export.targets ({:?}), but multi-target export is not yet implemented. Use a single-target layer or wait for multi-target merge support.",
                multi.len(),
                multi
            )));
        }
    };

    // Export the target crate's public API via rustdoc JSON.
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let schema = exporter.export(target_crate).map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)".to_owned()
        } else {
            String::new()
        };
        err(format!("failed to export schema: {e}{hint}"))
    })?;

    // Build TypeGraph and convert to TypeBaseline.
    let graph = build_type_graph(&schema, &typestate_names);
    let captured_at = crate::timestamp_now().map_err(|e| err(format!("timestamp error: {e}")))?;
    let baseline = baseline_builder::build_baseline(&graph, captured_at);

    // Encode and write.
    let encoded = baseline_codec::encode(&baseline)
        .map_err(|e| err(format!("baseline encode error: {e}")))?;

    atomic_write_file(&baseline_path, format!("{encoded}\n").as_bytes())
        .map_err(|e| err(format!("cannot write {}: {e}", baseline_path.display())))?;

    let type_count = baseline.types().len();
    let trait_count = baseline.traits().len();
    println!(
        "[OK] baseline-capture: wrote {baseline_filename} ({type_count} types, {trait_count} traits)"
    );

    Ok(())
}
