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

    // Validate the track_id without importing domain::TrackId (CN-01 / AC-03).
    crate::commands::track::validate_track_id_str(&track_id)
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
        infrastructure::tddd::baseline_capture::capture_baseline_for_layer(
            &items_dir,
            &track_id,
            &workspace_root,
            binding,
        )
        .map_err(|e| CliError::Message(e.0))?;
    }

    Ok(ExitCode::SUCCESS)
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
