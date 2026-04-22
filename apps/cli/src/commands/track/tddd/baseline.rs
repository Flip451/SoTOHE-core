//! `sotp track baseline-capture` — capture TypeGraph snapshot as baseline.
//!
//! Generates `<layer>-types-baseline.json` from the current TypeGraph.
//! Always idempotent: if the baseline file already exists it is kept as-is.
//! Re-capturing the baseline after implementation has started would overwrite
//! the pre-implementation snapshot with the current state, collapsing the
//! signal semantics (new `add` entries become `AddButAlreadyInBaseline` noise).
//! If a genuine re-capture is required, delete the stale
//! `<layer>-types-baseline.json` file manually first.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_type_graph;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::{baseline_builder, baseline_codec};
use infrastructure::track::atomic_write::atomic_write_file;
use infrastructure::track::symlink_guard::reject_symlinks_below;
use infrastructure::verify::tddd_layers::TdddLayerBinding;

use crate::CliError;
use crate::commands::track::tddd::signals::resolve_layers;

/// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
///
/// Steps:
/// 1. Resolve the set of TDDD-enabled layers to process (all enabled, or just the
///    specified `--layer`).
/// 2. For each layer binding, check if the baseline already exists (skip if so).
/// 3. Export the target crate schema via rustdoc JSON.
/// 4. Build TypeGraph and convert to TypeBaseline.
/// 5. Encode and write to `<layer>-types-baseline.json`.
///
/// When `--layer` is omitted, all TDDD-enabled layers are processed in `layers[]` order.
///
/// Always idempotent: existing baseline files are preserved. To re-capture, delete
/// the stale file manually first.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, rustdoc export fails,
/// or the file write fails.
pub fn execute_baseline_capture(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    // Resolve the set of TDDD-enabled layers to process. When
    // `architecture-rules.json` is absent we fall back to the legacy
    // single-`domain` binding so older tracks keep working. When `--layer`
    // is supplied we fail-closed on an unknown or disabled layer id.
    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;

    // Fail-closed when no layers are enabled: returning SUCCESS with no
    // work done would silently mask a misconfigured `architecture-rules.json`
    // (e.g. all layers have `tddd.enabled = false`).
    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json; \
             nothing to capture"
                .to_owned(),
        ));
    }

    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Security: verify the items_dir root itself is not a symlink before using it as the
    // trusted anchor. reject_symlinks_below only checks components *below* the trusted_root,
    // so a symlinked items_dir would pass through undetected.
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at items_dir: {}",
                items_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat items_dir {}: {e}",
                items_dir.display()
            )));
        }
    }

    for binding in &bindings {
        capture_baseline_for_layer(&items_dir, &track_id, &workspace_root, binding)?;
    }

    Ok(ExitCode::SUCCESS)
}

/// Capture the baseline for a single TDDD layer binding.
///
/// Handles the skip-if-exists logic, symlink guards, rustdoc export,
/// TypeGraph build, and atomic write for a single layer.
fn capture_baseline_for_layer(
    items_dir: &std::path::Path,
    track_id: &str,
    workspace_root: &std::path::Path,
    binding: &TdddLayerBinding,
) -> Result<(), CliError> {
    let catalogue_filename = binding.catalogue_file();
    let baseline_filename = binding.baseline_file();

    let track_dir = items_dir.join(track_id);
    let baseline_path = track_dir.join(&baseline_filename);

    // Security: reject symlinks in path components below items_dir.
    reject_symlinks_below(&baseline_path, items_dir)
        .map_err(|e| CliError::Message(format!("symlink guard: {e}")))?;

    // Idempotent: skip if baseline already exists as a regular file.
    // Use is_file() rather than exists() so that a directory or other non-file node at
    // that path does not silently produce a spurious success — it falls through and will
    // fail at the write step with a meaningful error instead.
    if baseline_path.is_file() {
        println!(
            "[OK] baseline-capture: {baseline_filename} already exists for '{track_id}' (delete the file manually to re-capture)"
        );
        return Ok(());
    }

    // Fail fast if the track directory does not exist.
    if !track_dir.is_dir() {
        return Err(CliError::Message(format!(
            "track directory not found: {} (did you mean an existing track ID?)",
            track_dir.display()
        )));
    }

    // Read the configured catalogue file to extract typestate names for
    // `build_type_graph`. Security: guard the leaf path too — a symlinked
    // catalogue file inside the track directory could redirect reads
    // outside the trusted tree.
    let catalogue_path = track_dir.join(catalogue_filename);
    reject_symlinks_below(&catalogue_path, items_dir)
        .map_err(|e| CliError::Message(format!("symlink guard: {e}")))?;
    // Read the catalogue file to extract typestate names. The catalogue is
    // optional — it may not yet exist for a brand-new track. When it is
    // absent we fall back to an empty set (conservative: treats all types as
    // non-typestate, which is safe for baseline capture). When the catalogue
    // is present but malformed we fail-closed to prevent silently capturing
    // an incorrect baseline.
    let typestate_names: std::collections::HashSet<String> =
        match std::fs::read_to_string(&catalogue_path) {
            Ok(json) => infrastructure::tddd::catalogue_codec::decode(&json)
                .map(|doc| doc.typestate_names())
                .map_err(|e| {
                    CliError::Message(format!(
                        "{} is malformed; fix or delete it before capturing baseline: {e}",
                        catalogue_path.display()
                    ))
                })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::collections::HashSet::new(),
            Err(e) => {
                return Err(CliError::Message(format!(
                    "cannot read {}: {e}",
                    catalogue_path.display()
                )));
            }
        };

    // Resolve the target crate for schema export from the binding. Multi-target
    // layers are modeled in `architecture-rules.json` (`schema_export.targets`)
    // but full merge of multiple per-crate schema exports is not yet
    // implemented. Fail-closed when more than one target is configured so that
    // the caller is not silently given a baseline snapshot computed from only
    // the first crate — that would drop types/traits from the remaining crates
    // and produce false undeclared/Red results on later signal evaluation.
    let layer_id = binding.layer_id();
    let target_crate = match binding.targets() {
        [single] => single,
        [] => {
            return Err(CliError::Message(format!(
                "schema_export.targets is empty for layer '{layer_id}'; check architecture-rules.json"
            )));
        }
        multi => {
            return Err(CliError::Message(format!(
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
        CliError::Message(format!("failed to export schema: {e}{hint}"))
    })?;

    // Build TypeGraph and convert to TypeBaseline.
    let graph = build_type_graph(&schema, &typestate_names);
    let captured_at = infrastructure::timestamp_now()
        .map_err(|e| CliError::Message(format!("timestamp error: {e}")))?;
    let baseline = baseline_builder::build_baseline(&graph, captured_at);

    // Encode and write.
    let encoded = baseline_codec::encode(&baseline)
        .map_err(|e| CliError::Message(format!("baseline encode error: {e}")))?;

    atomic_write_file(&baseline_path, format!("{encoded}\n").as_bytes())
        .map_err(|e| CliError::Message(format!("cannot write {}: {e}", baseline_path.display())))?;

    let type_count = baseline.types().len();
    let trait_count = baseline.traits().len();
    println!(
        "[OK] baseline-capture: wrote {baseline_filename} ({type_count} types, {trait_count} traits)"
    );

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_capture_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result =
            execute_baseline_capture(items_dir, "../evil".to_owned(), dir.path().into(), None);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_baseline_capture_skips_when_baseline_exists() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write a dummy baseline file (domain layer — default when no architecture-rules.json).
        std::fs::write(track_dir.join("domain-types-baseline.json"), "{}").unwrap();

        let result =
            execute_baseline_capture(items_dir, "test-track".to_owned(), dir.path().into(), None);
        assert!(result.is_ok(), "should skip existing baseline without error");
    }

    #[test]
    fn test_baseline_capture_with_usecase_layer_dispatches_to_usecase_binding() {
        // Proves that --layer usecase dispatches to the usecase binding specifically:
        // a pre-existing `usecase-types-baseline.json` (not domain) triggers the skip
        // path, returning Ok(SUCCESS). If the command were dispatching to the domain
        // binding, it would NOT see the usecase baseline and would proceed to export
        // (failing with a schema export error instead of Ok).
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write architecture-rules.json with usecase enabled.
        let rules_json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            {
              "crate": "usecase",
              "tddd": {
                "enabled": true,
                "catalogue_file": "usecase-types.json",
                "schema_export": { "method": "rustdoc", "targets": ["usecase"] }
              }
            }
          ]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        // Write usecase-types-baseline.json but NOT domain-types-baseline.json.
        // The skip check in capture_baseline_for_layer checks usecase-types-baseline.json
        // (derived from the usecase catalogue_file stem). Finding it must trigger the
        // skip path → Ok(SUCCESS). If the dispatch went to domain, the baseline
        // file checked would be domain-types-baseline.json, which is absent, so the
        // command would proceed to export and fail — proving the dispatch is wrong.
        std::fs::write(track_dir.join("usecase-types-baseline.json"), "{}").unwrap();

        let result = execute_baseline_capture(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            Some("usecase".to_owned()),
        );

        // Ok(SUCCESS) proves skip triggered for usecase-types-baseline.json.
        assert!(
            result.is_ok(),
            "dispatch to usecase binding must find existing baseline and skip, got: {result:?}"
        );
    }

    #[test]
    fn test_baseline_capture_no_layer_filter_iterates_all_enabled_bindings() {
        // Regression guard: when --layer is omitted, the loop must iterate ALL enabled
        // bindings, not stop after the first one.
        //
        // Setup: domain and usecase both enabled. The domain baseline already exists,
        // so the domain binding triggers the skip path (Ok return from
        // capture_baseline_for_layer). The usecase baseline does NOT exist — the loop
        // must continue past the domain skip and attempt the usecase export.
        //
        // Expected: Err (usecase export fails — nightly unavailable in test env).
        // If the loop stopped after the domain skip and returned Ok(SUCCESS), a
        // regression that silently processes only the first binding would pass
        // undetected. This test catches that.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let rules_json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            {
              "crate": "usecase",
              "tddd": {
                "enabled": true,
                "catalogue_file": "usecase-types.json",
                "schema_export": { "method": "rustdoc", "targets": ["usecase"] }
              }
            }
          ]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        // domain baseline exists → skip; usecase baseline absent → proceeds to export → fails.
        std::fs::write(track_dir.join("domain-types-baseline.json"), "{}").unwrap();
        // do NOT write usecase-types-baseline.json

        let result =
            execute_baseline_capture(items_dir, "test-track".to_owned(), dir.path().into(), None);

        assert!(
            result.is_err(),
            "loop must continue past domain skip to usecase and fail at export; \
             Ok(SUCCESS) would mean the loop stopped after the first binding"
        );
    }
}
