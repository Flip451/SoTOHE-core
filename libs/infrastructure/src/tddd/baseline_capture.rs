//! Baseline capture logic for a single TDDD layer binding.
//!
//! Moved from the CLI layer so that the CLI composition root never imports
//! `domain::schema::SchemaExporter` or `domain::schema::SchemaExportError`
//! directly (CN-01 / AC-03).
//!
//! T008: The old `capture_baseline_for_layer` path (which wrote a `TypeBaseline`
//! JSON using the now-deleted `TypeGraph`) is removed.  Only the rustdoc-format
//! path (`capture_rustdoc_baseline_for_layer`) remains.

use std::path::Path;

use domain::schema::SchemaExportError;

use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;

/// Error type returned by capture functions so the CLI can map to `CliError::Message`.
#[derive(Debug)]
pub struct CaptureBaselineError(pub String);

impl std::fmt::Display for CaptureBaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Capture the rustdoc-format baseline for a single TDDD layer binding.
///
/// Writes the raw `rustdoc_types::Crate` JSON produced by `cargo +nightly rustdoc`
/// to `<track_dir>/<layer>-types-baseline.json`.  If the file already exists the
/// function returns `Ok(())` immediately (idempotent). To re-capture, delete the
/// baseline file first.
///
/// `workspace_root` is the Cargo workspace from which rustdoc is invoked.
/// It may differ from the workspace that contains `items_dir`; this is the
/// intended use-case when capturing a baseline from a git worktree at the configured base branch
/// while writing the output into the current branch's track directory via
/// `--source-workspace`.
///
/// # Errors
///
/// Returns [`CaptureBaselineError`] when security guards reject the path, the
/// track directory does not exist, the rustdoc export fails, or the write fails.
pub fn capture_rustdoc_baseline_for_layer(
    items_dir: &Path,
    track_id: &str,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
) -> Result<(), CaptureBaselineError> {
    let err = |s: String| CaptureBaselineError(s);

    // Security: guard root directories.
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

    let baseline_filename = binding.baseline_file();

    // Security: validate track_id.
    let valid_track_id =
        domain::TrackId::try_new(track_id).map_err(|e| err(format!("invalid track_id: {e}")))?;

    let track_dir = items_dir.join(valid_track_id.as_ref());
    let baseline_path = track_dir.join(&baseline_filename);

    // Security: reject symlinks in path components below items_dir.
    reject_symlinks_below(&baseline_path, items_dir)
        .map_err(|e| err(format!("symlink guard: {e}")))?;

    // Idempotent: skip if baseline already exists as a regular file.
    // Validate the existing file's `format_version` to detect legacy `TypeBaseline`
    // JSON files left over from before the rustdoc migration. Such files will fail
    // `BaselineRustdocCodec::load` at signal-evaluation time, leaving the track
    // stuck. Surface a clear error here instead so the user knows to delete the
    // stale file and re-run.
    if baseline_path.is_file() {
        let existing = std::fs::read_to_string(&baseline_path).map_err(|e| {
            err(format!("cannot read existing baseline at {}: {e}", baseline_path.display()))
        })?;
        if let Err(e) = BaselineRustdocCodec::from_json(&existing) {
            return Err(err(format!(
                "{}: existing baseline failed rustdoc format validation: {e}. \
                 Delete the file and re-run to re-capture.",
                baseline_path.display()
            )));
        }
        println!(
            "[OK] baseline-capture (rustdoc): {baseline_filename} already exists for '{track_id}' (delete to re-capture)"
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
                "layer '{layer_id}' has {} schema_export.targets ({:?}), but multi-target export is not yet implemented.",
                multi.len(),
                multi
            )));
        }
    };

    // Run cargo +nightly rustdoc and get the output JSON path.
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let json_path = exporter.export_rustdoc_json_path(target_crate).map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)".to_owned()
        } else {
            String::new()
        };
        err(format!("failed to export rustdoc JSON: {e}{hint}"))
    })?;

    // Read the raw rustdoc JSON and validate format_version before writing.
    let json_content = std::fs::read_to_string(&json_path)
        .map_err(|e| err(format!("cannot read rustdoc JSON at {}: {e}", json_path.display())))?;

    // Validate format_version before persisting: reject JSON produced by a
    // mismatched nightly toolchain rather than silently writing an unusable
    // baseline that will fail later during signal evaluation.
    BaselineRustdocCodec::from_json(&json_content)
        .map_err(|e| err(format!("rustdoc JSON format_version validation failed: {e}")))?;

    atomic_write_file(&baseline_path, json_content.as_bytes())
        .map_err(|e| err(format!("cannot write {}: {e}", baseline_path.display())))?;

    println!("[OK] baseline-capture (rustdoc): wrote {baseline_filename} for layer '{layer_id}'");

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::verify::tddd_layers::parse_tddd_layers;

    fn domain_binding() -> TdddLayerBinding {
        let json = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true}}]}"#;
        parse_tddd_layers(json).unwrap().remove(0)
    }

    #[test]
    fn test_capture_worktree_flow_items_dir_outside_source_workspace_not_rejected() {
        // Regression: the `--source-workspace` worktree flow passes `workspace_root`
        // (the rustdoc source workspace, e.g. a `main` git worktree) while `items_dir`
        // lives in a different filesystem tree (the current branch). The function must
        // NOT reject this configuration — it is the documented use-case.
        //
        // Expected: the function reaches "track directory not found" (the track dir
        // does not exist in the tempdir), proving no spurious containment check fires.
        let source_workspace = tempfile::tempdir().unwrap(); // rustdoc source (configured base branch worktree — different tree)
        let items_workspace = tempfile::tempdir().unwrap(); // write target (current branch)
        let items_dir = items_workspace.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let binding = domain_binding();

        let result = capture_rustdoc_baseline_for_layer(
            &items_dir,
            "foo-2026-05-12",
            source_workspace.path(), // intentionally different from items_dir's root
            &binding,
        );

        let err = result.unwrap_err();
        assert!(
            err.0.contains("track directory not found"),
            "worktree flow must reach track-directory check, not fail on path containment; got: {}",
            err.0
        );
    }

    #[test]
    fn test_capture_standard_flow_fails_on_missing_track_dir() {
        // Standard flow: items_dir inside workspace_root also fails on the missing
        // track directory when no baseline exists yet.
        let workspace_root = tempfile::tempdir().unwrap();
        let items_dir = workspace_root.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let binding = domain_binding();

        let result = capture_rustdoc_baseline_for_layer(
            &items_dir,
            "foo-2026-05-12",
            workspace_root.path(),
            &binding,
        );

        let err = result.unwrap_err();
        assert!(
            err.0.contains("track directory not found"),
            "expected track-directory error, got: {}",
            err.0
        );
    }
}
