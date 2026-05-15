//! `RustdocBaselineCaptureAdapter` — infrastructure adapter for
//! [`domain::tddd::catalogue_v2::RustdocBaselineCapturePort`].
//!
//! Implements baseline capture using `cargo +nightly rustdoc` by reusing the
//! same logic as `baseline_capture.rs` but accepting domain-layer
//! `TdddLayerBinding` directly, avoiding a two-way type conversion between
//! the domain and infrastructure binding types.

use std::path::Path;

use domain::schema::SchemaExportError;
use domain::tddd::catalogue_v2::{
    BaselineCaptureIoError, RustdocBaselineCapturePort, TdddLayerBinding,
};

use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// RustdocBaselineCaptureAdapter
// ---------------------------------------------------------------------------

/// Stateless adapter implementing [`RustdocBaselineCapturePort`].
///
/// Runs `cargo +nightly rustdoc` against `rustdoc_workspace` and writes the
/// result to `<items_dir>/<track_id>/<layer>-types-baseline.json`. Accepts
/// the domain-level `TdddLayerBinding` directly.
///
/// Injected into `BaselineCaptureInteractor` at the `apps/cli` composition root.
#[derive(Debug, Clone, Default)]
pub struct RustdocBaselineCaptureAdapter;

impl RustdocBaselineCaptureAdapter {
    /// Creates a new adapter instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl RustdocBaselineCapturePort for RustdocBaselineCaptureAdapter {
    /// Captures the rustdoc-format baseline for a single layer binding.
    ///
    /// When `force` is `false`, the operation is idempotent: an existing valid
    /// baseline file causes an immediate `Ok(())` return.
    /// When `force` is `true`, an existing baseline is overwritten.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineCaptureIoError`] on security guard rejection, missing
    /// track directory, rustdoc export failure, format validation failure, or
    /// file write failure.
    fn capture(
        &self,
        items_dir: &Path,
        track_id: &str,
        rustdoc_workspace: &Path,
        binding: &TdddLayerBinding,
        force: bool,
    ) -> Result<(), BaselineCaptureIoError> {
        capture_baseline_inner(items_dir, track_id, rustdoc_workspace, binding, force)
    }
}

// ---------------------------------------------------------------------------
// Inner implementation
// ---------------------------------------------------------------------------

fn capture_baseline_inner(
    items_dir: &Path,
    track_id: &str,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    force: bool,
) -> Result<(), BaselineCaptureIoError> {
    let err = |s: String| BaselineCaptureIoError(s);

    // Security: guard root directories (mirrors baseline_capture.rs).
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

    // Security: validate track_id via domain newtype.
    let valid_track_id =
        domain::TrackId::try_new(track_id).map_err(|e| err(format!("invalid track_id: {e}")))?;

    let baseline_filename = &binding.baseline_file;
    let track_dir = items_dir.join(valid_track_id.as_ref());
    let baseline_path = track_dir.join(baseline_filename.as_str());

    // Security: reject symlinks in path components below items_dir.
    reject_symlinks_below(&baseline_path, items_dir)
        .map_err(|e| err(format!("symlink guard: {e}")))?;

    // Idempotent: skip if baseline already exists as a regular file (unless force).
    if !force && baseline_path.is_file() {
        let existing = std::fs::read_to_string(&baseline_path).map_err(|e| {
            err(format!("cannot read existing baseline at {}: {e}", baseline_path.display()))
        })?;
        if let Err(e) = BaselineRustdocCodec::from_json(&existing) {
            return Err(err(format!(
                "{}: existing baseline failed rustdoc format validation: {e}. \
                 Delete the file and re-run, or use `--force` to overwrite it.",
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

    let layer_id = &binding.layer_id;

    // Resolve the target crate for schema export from the binding.
    let target_crate = match binding.targets.as_slice() {
        [single] => single.as_str(),
        [] => {
            return Err(err(format!(
                "schema_export.targets is empty for layer '{layer_id}'; check architecture-rules.json"
            )));
        }
        multi => {
            return Err(err(format!(
                "layer '{layer_id}' has {} schema_export.targets ({:?}), \
                 but multi-target export is not yet implemented.",
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

    // Read and validate the rustdoc JSON before writing.
    let json_content = std::fs::read_to_string(&json_path)
        .map_err(|e| err(format!("cannot read rustdoc JSON at {}: {e}", json_path.display())))?;

    BaselineRustdocCodec::from_json(&json_content)
        .map_err(|e| err(format!("rustdoc JSON format_version validation failed: {e}")))?;

    atomic_write_file(&baseline_path, json_content.as_bytes())
        .map_err(|e| err(format!("cannot write {}: {e}", baseline_path.display())))?;

    if force {
        println!(
            "[OK] baseline-capture (rustdoc): overwrote {baseline_filename} for layer '{layer_id}'"
        );
    } else {
        println!(
            "[OK] baseline-capture (rustdoc): wrote {baseline_filename} for layer '{layer_id}'"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rustdoc_types::FORMAT_VERSION;

    use super::*;

    fn domain_binding(layer_id: &str) -> TdddLayerBinding {
        TdddLayerBinding {
            layer_id: layer_id.to_owned(),
            catalogue_file: format!("{layer_id}-types.json"),
            baseline_file: format!("{layer_id}-types-baseline.json"),
            targets: vec![layer_id.to_owned()],
        }
    }

    #[test]
    fn test_capture_adapter_fails_on_missing_track_dir() {
        let adapter = RustdocBaselineCaptureAdapter::new();
        let workspace = tempfile::tempdir().unwrap();
        let items_dir = workspace.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let binding = domain_binding("domain");

        let result =
            adapter.capture(&items_dir, "test-track-2026-01-01", workspace.path(), &binding, false);

        let err = result.unwrap_err();
        assert!(
            err.0.contains("track directory not found") || err.0.contains("symlink guard"),
            "expected track-directory or symlink error, got: {}",
            err.0
        );
    }

    #[test]
    fn test_capture_adapter_force_false_skips_existing_valid_baseline() {
        let workspace = tempfile::tempdir().unwrap();
        let items_dir = workspace.path().join("track/items");
        let track_dir = items_dir.join("test-track-2026-01-01");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write a valid rustdoc baseline so the idempotency check triggers.
        let minimal_json = format!(
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
        );
        std::fs::write(track_dir.join("domain-types-baseline.json"), &minimal_json).unwrap();

        let adapter = RustdocBaselineCaptureAdapter::new();
        let binding = domain_binding("domain");

        // force = false: existing baseline → idempotent skip → Ok(())
        let result =
            adapter.capture(&items_dir, "test-track-2026-01-01", workspace.path(), &binding, false);
        assert!(result.is_ok(), "existing valid baseline must be skipped: {result:?}");
    }

    #[test]
    fn test_capture_adapter_invalid_track_id_is_rejected() {
        let adapter = RustdocBaselineCaptureAdapter::new();
        let workspace = tempfile::tempdir().unwrap();
        let items_dir = workspace.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let binding = domain_binding("domain");

        let result = adapter.capture(&items_dir, "../evil", workspace.path(), &binding, false);

        let err = result.unwrap_err();
        assert!(
            err.0.contains("invalid track_id") || err.0.contains("invalid track id"),
            "expected invalid track_id error, got: {}",
            err.0
        );
    }
}
