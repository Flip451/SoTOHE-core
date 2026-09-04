//! `RustdocBaselineCaptureAdapter` — infrastructure adapter for
//! [`domain::tddd::catalogue_v2::RustdocBaselineCapturePort`].
//!
//! Implements baseline capture using `cargo +nightly rustdoc` by reusing the
//! same logic as `baseline_capture.rs` but accepting domain-layer
//! `TdddLayerBinding` directly, avoiding a two-way type conversion between
//! the domain and infrastructure binding types.

use std::path::Path;

use domain::TrackId;
use domain::schema::SchemaExportError;
use domain::tddd::catalogue_v2::{
    BaselineCaptureIoError, RustdocBaselineCapturePort, TdddLayerBinding,
};
use domain::tddd::{CargoFeatureName, catalogue_v2::CrateName};

use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes;
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
    /// The operation is always idempotent: an existing valid baseline file causes
    /// an immediate `Ok(())` return. To re-capture, delete the baseline file first.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineCaptureIoError`] on security guard rejection, missing
    /// track directory, rustdoc export failure, format validation failure, or
    /// file write failure.
    fn capture(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
        rustdoc_workspace: &Path,
        binding: &TdddLayerBinding,
        features: &[CargoFeatureName],
    ) -> Result<(), BaselineCaptureIoError> {
        capture_baseline_inner(items_dir, track_id, rustdoc_workspace, binding, features)
    }
}

// ---------------------------------------------------------------------------
// Inner implementation
// ---------------------------------------------------------------------------

fn capture_baseline_inner(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
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

    let baseline_filename = &binding.baseline_file;
    let track_dir = items_dir.join(track_id.as_ref());
    let baseline_path = track_dir.join(baseline_filename.as_str());

    // Security: reject symlinks in path components below items_dir.
    reject_symlinks_below(&baseline_path, items_dir)
        .map_err(|e| err(format!("symlink guard: {e}")))?;

    // Idempotent: skip if baseline already exists as a regular file. Keep
    // this path bounded and no-follow just like the fresh rustdoc snapshot.
    if let Some(existing_bytes) =
        read_optional_regular_file_bytes(&baseline_path, Some(items_dir), 64 * 1024 * 1024)
            .map_err(|e| {
                err(format!("cannot read existing baseline at {}: {e}", baseline_path.display()))
            })?
    {
        let existing = String::from_utf8(existing_bytes)
            .map_err(|e| err(format!("existing baseline is not UTF-8: {e}")))?;
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
    let target_crate = CrateName::new(target_crate.to_owned()).map_err(|error| {
        err(format!("invalid schema-export target for layer '{layer_id}': {error}"))
    })?;
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let (_, json_bytes) = exporter.capture_rustdoc_json(&target_crate, features).map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)".to_owned()
        } else {
            String::new()
        };
        err(format!("failed to export rustdoc JSON: {e}{hint}"))
    })?;

    // Validate the immutable bytes returned from the locked capture before writing.
    let json_content = String::from_utf8(json_bytes)
        .map_err(|e| err(format!("rustdoc JSON is not UTF-8: {e}")))?;

    BaselineRustdocCodec::from_json(&json_content)
        .map_err(|e| err(format!("rustdoc JSON format_version validation failed: {e}")))?;

    atomic_write_file(&baseline_path, json_content.as_bytes())
        .map_err(|e| err(format!("cannot write {}: {e}", baseline_path.display())))?;

    println!("[OK] baseline-capture (rustdoc): wrote {baseline_filename} for layer '{layer_id}'");

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

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value.to_owned()).unwrap()
    }

    #[test]
    fn test_capture_adapter_fails_on_missing_track_dir() {
        let adapter = RustdocBaselineCaptureAdapter::new();
        let workspace = tempfile::tempdir().unwrap();
        let items_dir = workspace.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let binding = domain_binding("domain");

        let result = adapter.capture(
            &items_dir,
            &track_id("test-track-2026-01-01"),
            workspace.path(),
            &binding,
            &[],
        );

        let err = result.unwrap_err();
        assert!(
            err.0.contains("track directory not found") || err.0.contains("symlink guard"),
            "expected track-directory or symlink error, got: {}",
            err.0
        );
    }

    #[test]
    fn test_capture_adapter_with_declared_feature_skips_existing_valid_baseline() {
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

        // existing baseline → idempotent skip → Ok(())
        let features = [CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()];
        let result = adapter.capture(
            &items_dir,
            &track_id("test-track-2026-01-01"),
            workspace.path(),
            &binding,
            &features,
        );
        assert!(result.is_ok(), "existing valid baseline must be skipped: {result:?}");
    }

    #[test]
    fn test_capture_adapter_with_declared_features_writes_feature_gated_baseline() {
        const CHILD_STATE_ENV: &str = "TDDD_CAPTURE_TEST_STATE";
        const TEST_NAME: &str = concat!(
            "tddd::rustdoc_baseline_capture_adapter::tests::",
            "test_capture_adapter_with_declared_features_writes_feature_gated_baseline"
        );

        if let Some(state_dir) = std::env::var_os(CHILD_STATE_ENV) {
            let state_dir = Path::new(&state_dir);
            let items_dir = state_dir.join("track/items");
            let track = track_id("test-track-2026-01-01");
            let binding = domain_binding("infrastructure");
            let features = [CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()];
            let workspace_root =
                Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).unwrap();

            RustdocBaselineCaptureAdapter::new()
                .capture(&items_dir, &track, workspace_root, &binding, &features)
                .unwrap();
            return;
        }

        let state = tempfile::tempdir().unwrap();
        let commands_dir = state.path().join("commands");
        let items_dir = state.path().join("track/items");
        let target_dir = state.path().join("target");
        let track = track_id("test-track-2026-01-01");
        std::fs::create_dir_all(&commands_dir).unwrap();
        std::fs::create_dir_all(items_dir.join(track.as_ref())).unwrap();

        let rustup = commands_dir.join("rustup");
        std::fs::write(&rustup, "#!/bin/sh\nexit 0\n").unwrap();
        let cargo = commands_dir.join("cargo");
        std::fs::write(
            &cargo,
            format!(
                r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
    printf '{{"packages":[{{"name":"infrastructure","targets":[{{"kind":["lib"],"name":"infrastructure"}}]}}],"target_directory":"%s"}}\n' "$CARGO_TARGET_DIR"
    exit 0
fi
printf '%s\n' "$*" > "$(dirname "$0")/rustdoc-args"
if [ "$*" != "+nightly rustdoc -p infrastructure --lib --no-default-features --features semantic-dup -- -Z unstable-options --output-format json --document-hidden-items" ]; then
    exit 1
fi
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{"0":{{"crate_id":0,"path":["infrastructure","semantic_dup","fragment_extractor_adapter","CodeFragmentExtractorAdapter"],"kind":"struct"}}}},"external_crates":{{}},"format_version":{FORMAT_VERSION},"target":{{"triple":"","target_features":[]}}}}' > "$CARGO_TARGET_DIR/doc/infrastructure.json"
"#
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            for command in [&rustup, &cargo] {
                let mut permissions = std::fs::metadata(command).unwrap().permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(command, permissions).unwrap();
            }
        }

        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        let mut path_entries = vec![commands_dir];
        path_entries.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
        command
            .args(["--exact", TEST_NAME, "--nocapture"])
            .env(CHILD_STATE_ENV, state.path())
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("PATH", std::env::join_paths(path_entries).unwrap());
        assert!(command.status().unwrap().success());

        let baseline = String::from_utf8(
            read_optional_regular_file_bytes(
                &items_dir.join(track.as_ref()).join("infrastructure-types-baseline.json"),
                None,
                64 * 1024 * 1024,
            )
            .unwrap()
            .unwrap(),
        )
        .unwrap();
        let rustdoc = BaselineRustdocCodec::from_json(&baseline).unwrap();
        assert_eq!(rustdoc.format_version, FORMAT_VERSION);
        assert!(
            baseline.contains("CodeFragmentExtractorAdapter"),
            "the feature-gated public adapter must be present in the captured baseline"
        );
        assert_eq!(
            std::fs::read_to_string(state.path().join("commands/rustdoc-args")).unwrap().trim(),
            "+nightly rustdoc -p infrastructure --lib --no-default-features --features semantic-dup -- -Z unstable-options --output-format json --document-hidden-items"
        );
    }

    #[test]
    fn test_track_id_with_invalid_value_is_rejected_before_adapter_capture() {
        assert!(TrackId::try_new("../evil".to_owned()).is_err());
    }
}
