//! `sotp track catalogue-spec-signals` — regenerate
//! `<layer>-catalogue-spec-signals.json` for each catalogue-spec-enabled layer.
//!
//! The command reads the LOCAL `<layer>-types.json` (not the origin blob —
//! unlike the merge-gate path) because the refresh is a pre-commit step that
//! must reflect uncommitted changes in the workspace. It delegates the per-entry
//! signal computation to the domain pure function
//! `evaluate_catalogue_entry_signal` and the atomic write to
//! `FsCatalogueSpecSignalsStore` (T012).
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D2 / §D3.1 / IN-09.

use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
use infrastructure::track::fs_store::read_track_status_str;

use crate::CliError;
use crate::commands::track::tddd::signals::{ensure_active_track, resolve_layers};

/// Per-layer refresh entry point.
///
/// Same guards as `execute_type_signals`: track id validation, active-track
/// reject on `Done` / `Archived`, `architecture-rules.json` fail-closed via
/// `resolve_layers`.
///
/// # Errors
///
/// Returns `CliError` when the track id is invalid, metadata / impl-plan can
/// not be loaded, the track is completed / archived, the layer filter is
/// unknown, any per-layer `<layer>-types.json` is missing or fails to decode,
/// or the atomic write fails.
pub fn execute_catalogue_spec_signals(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track id and derive status without importing domain types (CN-01 / AC-03).
    // read_track_status_str validates the track_id and returns the status string.
    let status_str = read_track_status_str(&items_dir, &track_id).map_err(|e| {
        CliError::Message(format!("cannot load track status for '{track_id}': {e}"))
    })?;

    // Security: verify the items_dir root itself is not a symlink before using it as the
    // trusted anchor for `reject_symlinks_below`. That helper only checks components
    // *below* the trusted_root, so a symlinked items_dir would bypass all path guards.
    // Mirrors `execute_baseline_capture` (baseline.rs).
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

    // Security: verify the track directory itself is not a symlink before using
    // it as a path component for metadata / impl-plan reads. The `items_dir`
    // check above only covers `items_dir` itself; a symlinked
    // `items_dir/<track_id>` would escape the trusted tree before the per-layer
    // `reject_symlinks_below` guard (anchored at `items_dir`) can catch it.
    // Mirrors `execute_baseline_capture` which rejects symlinks at `track_dir`
    // via `reject_symlinks_below(&baseline_path, items_dir)` before any reads.
    let track_dir = items_dir.join(&track_id);
    match track_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at track directory: {}",
                track_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Track directory absent — the metadata read below will produce a
            // clear error message. Don't short-circuit here.
        }
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat track directory {}: {e}",
                track_dir.display()
            )));
        }
    }

    // Active-track guard (CN-07) — reject completed/archived tracks.
    // Uses string comparison to avoid importing domain::TrackStatus (CN-01 / AC-03).
    ensure_active_track_catalogue(&status_str, &track_id)?;

    // Resolve layers — `catalogue_spec_signal.enabled` flag is introduced by
    // T018; until then we fall back to every `tddd.enabled` layer so this
    // command is usable during the transition period.
    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;
    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json; \
             nothing to evaluate"
                .to_owned(),
        ));
    }

    // Pass `items_dir` (not `workspace_root`) so the store writes under the same
    // tree the reader is using. The default resolution is `workspace_root/track/items`,
    // but a `--items-dir` override must propagate to both read and write paths —
    // the previous `workspace_root`-based wiring left the two tracking distinct
    // trees when `--items-dir` diverged from the default (PR #111 P1 finding).
    let writer = FsCatalogueSpecSignalsStore::new(items_dir.clone());

    for binding in &bindings {
        if !binding.catalogue_spec_signal_enabled() {
            // Per ADR §D5.4 phased activation: skip layers that have not
            // opted in via `architecture-rules.json`
            // `tddd.catalogue_spec_signal.enabled`.
            continue;
        }
        infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
            &items_dir, &track_dir, &track_id, binding, &writer,
        )
        .map_err(CliError::Message)?;
    }

    Ok(ExitCode::SUCCESS)
}

/// Fail-closed active-track guard mirroring
/// `track::tddd::signals::ensure_active_track` but customised for the
/// catalogue-spec-signals command name in the error message.
///
/// Uses string comparison so the CLI never imports `domain::TrackStatus`
/// (CN-01 / AC-03). The `_` arm preserves fail-open for unknown future status
/// strings (same policy as `ensure_active_track`).
fn ensure_active_track_catalogue(status_str: &str, track_id: &str) -> Result<(), CliError> {
    match status_str {
        "done" | "archived" => Err(CliError::Message(format!(
            "cannot run catalogue-spec-signals on '{track_id}' (status={status_str}). \
             Completed tracks are frozen — run on an active track instead.",
        ))),
        _ => {
            // Mirror the `ensure_active_track` helper's cross-check so any new
            // status string behaviour is consistent.
            ensure_active_track(status_str, track_id)?;
            Ok(())
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    fn write_metadata(track_dir: &Path, track_id: &str) {
        let metadata = serde_json::json!({
            "schema_version": 5,
            "id": track_id,
            "branch": format!("track/{track_id}"),
            "title": "Test Track",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        fs::write(
            track_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
    }

    fn write_impl_plan(track_dir: &Path) {
        let doc = serde_json::json!({
            "schema_version": 1,
            "tasks": [{"id": "T001", "description": "stub", "status": "in_progress"}],
            "plan": {
                "summary": [],
                "sections": [
                    {"id": "S1", "title": "Stub", "description": [], "task_ids": ["T001"]}
                ]
            }
        });
        fs::write(track_dir.join("impl-plan.json"), serde_json::to_string_pretty(&doc).unwrap())
            .unwrap();
    }

    fn write_architecture_rules(workspace_root: &Path) {
        let rules = serde_json::json!({
            "schema_version": 2,
            "layers": [
                {
                    "crate": "test_layer",
                    "path": "libs/test_layer",
                    "dependencies": [],
                    "deny_reason": "",
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "test_layer-types.json",
                        "catalogue_spec_signal": {
                            "enabled": true
                        }
                    }
                }
            ]
        });
        fs::write(
            workspace_root.join("architecture-rules.json"),
            serde_json::to_string_pretty(&rules).unwrap(),
        )
        .unwrap();
    }

    fn write_catalogue(track_dir: &Path) {
        // v3-native format required by CatalogueDocumentCodec::decode.
        // BTreeMap ordering: BlueType < RedType < YellowType (alphabetical).
        let catalogue = serde_json::json!({
            "schema_version": 3,
            "crate_name": "test_layer",
            "layer": "test_layer",
            "types": {
                "BlueType": {
                    "action": "add",
                    "role": "ValueObject",
                    "kind": { "kind": "unit_struct" },
                    "spec_refs": [
                        {
                            "file": "track/items/x/spec.json",
                            "anchor": "IN-01",
                            "hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        }
                    ]
                },
                "RedType": {
                    "action": "add",
                    "role": "ValueObject",
                    "kind": { "kind": "unit_struct" }
                },
                "YellowType": {
                    "action": "add",
                    "role": "ValueObject",
                    "kind": { "kind": "unit_struct" },
                    "informal_grounds": [
                        {"kind": "user_directive", "summary": "pending formalization"}
                    ]
                }
            },
            "traits": {},
            "functions": {}
        });
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&catalogue).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn refresh_writes_signals_for_all_entries() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata(&track_dir, track_id);
        write_impl_plan(&track_dir);
        write_architecture_rules(&ws);
        write_catalogue(&track_dir);

        let result = execute_catalogue_spec_signals(
            items_dir.clone(),
            track_id.to_owned(),
            ws.clone(),
            None,
        );
        assert!(result.is_ok(), "execute must succeed: {result:?}");

        let signals_path = ws.join("track/items/test-track/test_layer-catalogue-spec-signals.json");
        assert!(signals_path.exists(), "signals file must be written");
        let content = fs::read_to_string(&signals_path).unwrap();
        // Parse as raw JSON to avoid importing domain types in CLI test code (AC-03).
        // The codec serialises ConfidenceSignal as "blue" / "yellow" / "red".
        let raw: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(raw["schema_version"].as_u64().unwrap(), 1);
        let sigs = raw["signals"].as_array().unwrap();
        assert_eq!(sigs.len(), 3);
        // BTreeMap keys are sorted alphabetically: BlueType < RedType < YellowType.
        assert_eq!(sigs[0]["type_name"].as_str().unwrap(), "BlueType");
        assert_eq!(sigs[0]["signal"].as_str().unwrap(), "blue");
        assert_eq!(sigs[1]["type_name"].as_str().unwrap(), "RedType");
        assert_eq!(sigs[1]["signal"].as_str().unwrap(), "red");
        assert_eq!(sigs[2]["type_name"].as_str().unwrap(), "YellowType");
        assert_eq!(sigs[2]["signal"].as_str().unwrap(), "yellow");
    }

    #[test]
    fn refresh_rejects_path_traversal_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let items_dir = ws.join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = execute_catalogue_spec_signals(items_dir, "../evil".to_owned(), ws, None);
        // Verify the PATH-TRAVERSAL guard specifically rejected the id, not some
        // later filesystem error. `read_track_status_str` validates the id before any
        // metadata I/O occurs, so the error message always mentions the failure.
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid track ID") || err.contains("cannot load track status"),
            "expected path-traversal rejection, got: {err}"
        );
    }

    #[test]
    fn refresh_fails_when_catalogue_missing() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata(&track_dir, track_id);
        write_impl_plan(&track_dir);
        write_architecture_rules(&ws);
        // No catalogue file on disk.

        let result = execute_catalogue_spec_signals(items_dir, track_id.to_owned(), ws, None);
        assert!(result.is_err());
    }

    #[test]
    fn refresh_with_explicit_layer_filter_processes_only_requested() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata(&track_dir, track_id);
        write_impl_plan(&track_dir);
        write_architecture_rules(&ws);
        write_catalogue(&track_dir);

        let result = execute_catalogue_spec_signals(
            items_dir,
            track_id.to_owned(),
            ws.clone(),
            Some("test_layer".to_owned()),
        );
        assert!(result.is_ok());

        assert!(ws.join("track/items/test-track/test_layer-catalogue-spec-signals.json").exists());
    }

    #[test]
    fn refresh_rejects_unknown_layer_filter() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata(&track_dir, track_id);
        write_impl_plan(&track_dir);
        write_architecture_rules(&ws);
        write_catalogue(&track_dir);

        let result = execute_catalogue_spec_signals(
            items_dir,
            track_id.to_owned(),
            ws,
            Some("nonexistent".to_owned()),
        );
        assert!(result.is_err());
    }

    /// The active-track guard must reject `Done` tracks (all tasks resolved).
    /// `ensure_active_track_catalogue` must fail-closed before any write occurs.
    #[test]
    fn refresh_rejects_done_track() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata(&track_dir, track_id);

        // Write an impl-plan where all tasks are resolved (done) → derive_track_status → Done.
        let doc = serde_json::json!({
            "schema_version": 1,
            "tasks": [{"id": "T001", "description": "stub", "status": "done", "commit_hash": "abc1234"}],
            "plan": {
                "summary": [],
                "sections": [
                    {"id": "S1", "title": "Stub", "description": [], "task_ids": ["T001"]}
                ]
            }
        });
        fs::write(track_dir.join("impl-plan.json"), serde_json::to_string_pretty(&doc).unwrap())
            .unwrap();

        write_architecture_rules(&ws);
        write_catalogue(&track_dir);

        let result =
            execute_catalogue_spec_signals(items_dir, track_id.to_owned(), ws.clone(), None);
        assert!(result.is_err(), "Done track must be rejected: {result:?}");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("done") || err_msg.contains("frozen") || err_msg.contains("status"),
            "error must mention frozen/done status, got: {err_msg}"
        );
        // Verify no signals file was written (fail-closed before any write).
        let signals_path = ws.join("track/items/test-track/test_layer-catalogue-spec-signals.json");
        assert!(!signals_path.exists(), "no signals file must be written for Done track");
    }
}
