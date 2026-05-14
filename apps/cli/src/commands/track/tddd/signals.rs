//! `sotp track type-signals` — evaluate type signals via rustdoc schema export.
//!
//! Reads `<layer>-types.json` from the track directory, exports the target crate's
//! public API via rustdoc JSON, evaluates signals for each declared type, and writes
//! the updated document back to `<layer>-types.json`.
//!
//! `resolve_layers` and `ensure_active_track` remain in this module as shared
//! helpers for sibling CLI commands (`catalogue_spec_signals.rs`,
//! `contract_map.rs`) that have not yet been migrated to usecase interactors.
//! `execute_type_signals_lenient_with_bindings` stays here to allow
//! `commands/make.rs` to share a single architecture-rules.json parse (TOCTOU
//! prevention).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use infrastructure::tddd::type_signals_evaluator::{
    EvaluateSignalsError, execute_type_signals_for_layer,
};
use infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
use infrastructure::track::fs_store::read_track_status_str;
use infrastructure::track::track_status_reader_adapter::FsTrackStatusReaderAdapter;
use infrastructure::verify::tddd_layers::{
    LoadTdddLayersError, TdddLayerBinding, load_tddd_layers_from_path,
};
use usecase::type_signals::{TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService};

use crate::CliError;

/// Resolves the set of TDDD-enabled layers for this invocation.
///
/// - Reads `architecture-rules.json` from `workspace_root`.
/// - When `layer_filter` is `None`, returns every `tddd.enabled` layer in
///   `layers[]` order.
/// - When `layer_filter` is `Some(id)`, returns only the matching enabled
///   binding. An unknown or disabled layer id is fail-closed.
/// - When `architecture-rules.json` is absent, falls back to a single
///   synthetic `domain` binding so legacy tracks continue to work.
pub(crate) fn resolve_layers(
    workspace_root: &Path,
    layer_filter: Option<&str>,
) -> Result<Vec<TdddLayerBinding>, CliError> {
    let rules_path = workspace_root.join("architecture-rules.json");
    // Delegate symlink handling + legacy-fallback policy to the shared
    // infrastructure helper. CLI stays a thin composition layer; it only
    // maps the infra error variants into `CliError` and applies the
    // CLI-level layer filter.
    let bindings =
        load_tddd_layers_from_path(&rules_path, workspace_root).map_err(|e| match e {
            LoadTdddLayersError::Io { path, source } => {
                CliError::Message(format!("{}: {source}", path.display()))
            }
            LoadTdddLayersError::Parse(err) => {
                CliError::Message(format!("{}: {err}", rules_path.display()))
            }
        })?;

    if let Some(filter) = layer_filter {
        let Some(binding) = bindings.iter().find(|b| b.layer_id() == filter) else {
            return Err(CliError::Message(format!(
                "layer '{filter}' is not tddd.enabled in architecture-rules.json"
            )));
        };
        Ok(vec![binding.clone()])
    } else {
        Ok(bindings)
    }
}

/// Fail-closed active-track guard: rejects `Done` / `Archived` tracks.
///
/// Accepts the track status as a string (from `read_track_status_str`) so
/// the CLI layer never imports `domain::TrackStatus` directly (CN-01 / AC-03).
///
/// Explicit exhaustive match over all six known `TrackStatus` variants
/// (domain SSoT as of this writing):
/// - `done` / `archived` → frozen (reject)
/// - `planned` / `in_progress` / `blocked` / `cancelled` → active (allow)
/// - any other string → unknown (fail-closed: reject with guidance to update CLI)
///
/// Fail-closed for unknowns: an unrecognised status signals that the domain
/// added a new variant that the CLI has not classified yet. Blocking TDDD
/// commands in that case is safer than silently allowing runs on a track
/// that might be frozen.
pub(crate) fn ensure_active_track(status_str: &str, track_id: &str) -> Result<(), CliError> {
    match status_str {
        "done" | "archived" => Err(CliError::Message(format!(
            "cannot run type-signals on '{track_id}' (status={status_str}). \
             Completed tracks are frozen — run on an active track instead.",
        ))),
        "planned" | "in_progress" | "blocked" | "cancelled" => Ok(()),
        _ => Err(CliError::Message(format!(
            "cannot run type-signals on '{track_id}': unrecognised track status '{status_str}'. \
             Update the CLI frozen-track guard to classify the new status.",
        ))),
    }
}

/// Map `EvaluateSignalsError` from infrastructure to `CliError`.
fn map_eval_err(e: EvaluateSignalsError) -> CliError {
    CliError::Message(e.to_string())
}

/// Evaluate type signals via rustdoc schema export and write back to `<layer>-types.json`.
///
/// Thin CLI adapter: constructs the concrete infrastructure adapters, wires up
/// `TypeSignalsInteractor` with `lenient: false`, and delegates all orchestration
/// to the usecase layer.
///
/// The track items directory is always derived as `<workspace_root>/track/items`
/// inside the interactor, so callers only need to supply `workspace_root`.
///
/// Steps (inside the interactor):
/// 1. Validate the track ID format.
/// 2. Read and guard the track status (reject Done/Archived).
/// 3. Resolve the set of TDDD-enabled layers to process.
/// 4. For each layer binding, evaluate signals and write back to `<layer>-types.json`.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, the file cannot be read or
/// decoded, rustdoc export fails (e.g., nightly not installed), or the write fails.
pub fn execute_type_signals(
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    // Derive items_dir from workspace_root so the caller does not need to supply
    // both and so the interactor's items_dir == workspace_root/track/items check
    // is always satisfied regardless of how the caller invoked the command.
    let items_dir = workspace_root.join("track").join("items");

    let status_reader = Arc::new(FsTrackStatusReaderAdapter::new());
    // Use legacy-fallback mode so that legacy tracks without architecture-rules.json
    // continue to work (synthetic domain binding, same as load_tddd_layers_from_path).
    let layer_bindings = Arc::new(FsTdddLayerBindingsAdapter::new_with_legacy_fallback());
    let executor = Arc::new(TypeSignalsExecutorAdapter::new());

    let interactor = TypeSignalsInteractor::new(status_reader, layer_bindings, executor);

    interactor
        .run(TypeSignalsRequest { items_dir, track_id, workspace_root, layer, lenient: false })
        .map_err(|e| CliError::Message(e.to_string()))?;

    Ok(ExitCode::SUCCESS)
}

/// Pre-commit-flavored variant of [`execute_type_signals`] that treats a
/// missing per-layer catalogue file as "layer not yet initialized for this
/// track" and skips it silently, matching the CI
/// (`spec_states::evaluate_layer_catalogue`) and merge-gate semantics. This
/// resolves the asymmetry where the user-invoked `sotp track type-signals`
/// hard-fails on a missing catalogue (correct UX — the user explicitly asked
/// to evaluate), but the automated pre-commit hook must behave like the
/// verification gates (skip inactive layers, pass).
///
/// Same guards as `execute_type_signals`: active-track guard, `architecture-rules.json`
/// fail-closed via `resolve_layers`, empty-bindings fail-closed. Only the
/// per-layer NotFound handling differs.
///
/// # Errors
///
/// Returns `CliError` on the same paths as `execute_type_signals` EXCEPT the
/// per-layer catalogue NotFound, which is silently skipped here.
#[allow(dead_code)]
pub fn execute_type_signals_lenient(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;
    execute_type_signals_lenient_with_bindings(items_dir, track_id, workspace_root, &bindings)
}

/// Same semantics as [`execute_type_signals_lenient`] but accepts a
/// caller-supplied `bindings` snapshot so the caller can run its own
/// validation + classification against exactly the same binding set that
/// was processed here. Closes the TOCTOU window where
/// `architecture-rules.json` could be edited between a caller's pre-flight
/// parse and the internal `resolve_layers` read.
///
/// The pre-commit wiring in `dispatch_track_commit_message` uses this
/// variant to share one parsed binding snapshot across pre-flight
/// validation, recompute, and the post-recompute signal classification
/// loop.
///
/// # Errors
///
/// Same as [`execute_type_signals_lenient`].
pub fn execute_type_signals_lenient_with_bindings(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    bindings: &[TdddLayerBinding],
) -> Result<ExitCode, CliError> {
    // Validate track_id and derive status without importing domain types (CN-01 / AC-03).
    let status_str = read_track_status_str(&items_dir, &track_id).map_err(|e| {
        CliError::Message(format!("cannot load track status for '{track_id}': {e}"))
    })?;
    ensure_active_track(&status_str, &track_id)?;

    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json; nothing to evaluate"
                .to_owned(),
        ));
    }

    let track_dir = items_dir.join(&track_id);
    for binding in bindings {
        // Skip layers with multi-target `schema_export.targets`:
        // `execute_type_signals_for_layer` hard-fails on that configuration
        // (multi-target rustdoc merge is not implemented yet). The CI /
        // merge-gate paths read the persisted `<layer>-type-signals.json`
        // directly and do not re-export schema, so they detect stale
        // signals via `declaration_hash` comparison independently. Pre-commit
        // must NOT block the commit on that unsupported configuration —
        // that would create a hard regression for multi-target tracks
        // (PR #106 multi-target P1 finding).
        //
        // This skip is narrow by design: it only bypasses recompute for
        // configurations the strict evaluator cannot handle. It does NOT
        // short-circuit on "signal file already fresh" — code or baseline
        // changes without editing the declaration file would otherwise
        // let real regressions slip past pre-commit (PR #106 recompute-on-
        // hash-match P1 finding).
        if binding.targets().len() > 1 {
            continue;
        }

        let catalogue_path = track_dir.join(binding.catalogue_file());
        // Use `symlink_metadata` + explicit NotFound match so only a truly
        // absent declaration file is treated as "layer inactive". Symlinks,
        // directories, permission errors, and other `std::fs` failures
        // propagate as errors — matching the CI
        // (`evaluate_layer_catalogue`) fail-closed posture and preventing
        // the "pre-commit passes, verification fails later" divergence.
        match std::fs::symlink_metadata(&catalogue_path) {
            Ok(meta) if meta.file_type().is_file() => {
                execute_type_signals_for_layer(&items_dir, &track_id, &workspace_root, binding)
                    .map_err(map_eval_err)?;
            }
            Ok(_) => {
                // Regular-file check failed: symlink, directory, block
                // device, etc. Delegate to the strict evaluator so the
                // caller sees the same error as the CI / merge-gate path
                // (`reject_symlinks_below` / non-regular-file rejection).
                execute_type_signals_for_layer(&items_dir, &track_id, &workspace_root, binding)
                    .map_err(map_eval_err)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Declaration file is genuinely absent — layer not
                // TDDD-active for this track. Skip silently (symmetric with
                // `spec_states::evaluate_layer_catalogue` NotFound branch).
            }
            Err(e) => {
                return Err(CliError::Message(format!(
                    "pre-commit: cannot stat {}: {e}",
                    catalogue_path.display()
                )));
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Minimal valid `metadata.json` (schema v5) with a branch set (activated track).
    /// Callers must also write `impl-plan.json` (see [`minimal_impl_plan_json`]) to satisfy
    /// the activated-track guard in `execute_type_signals`.
    fn minimal_active_metadata_json(track_id: &str) -> String {
        format!(
            r#"{{
  "schema_version": 5,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test Track",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z"
}}
"#
        )
    }

    /// Minimal valid `impl-plan.json` content.  Required alongside any fixture that uses
    /// [`minimal_active_metadata_json`] (branch set) so the activated-track guard passes.
    fn minimal_impl_plan_json() -> &'static str {
        r#"{"schema_version":1,"tasks":[],"plan":{"summary":[],"sections":[]}}"#
    }

    /// Sets up a minimal track directory with the given `domain-types.json` content,
    /// a valid `metadata.json` (activated, branch set), and a minimal `impl-plan.json`
    /// so the activated-track guard in `execute_type_signals` passes.
    ///
    /// Returns `(workspace_root, track_id)` so callers can pass `workspace_root` directly
    /// to `execute_type_signals` (which derives `items_dir` internally).
    fn setup_track(dir: &std::path::Path, domain_types: &str) -> (PathBuf, String) {
        let workspace_root = dir.to_path_buf();
        let items_dir = workspace_root.join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("domain-types.json"), domain_types).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json(track_id))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();
        (workspace_root, track_id.to_owned())
    }

    #[test]
    fn test_execute_type_signals_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let workspace_root = dir.path().to_path_buf();

        let result = execute_type_signals("../evil".to_owned(), workspace_root, None);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_execute_type_signals_with_missing_domain_types_json_returns_error() {
        // T008: the old evaluator is removed and returns an error stub regardless of
        // whether domain-types.json is present. This test verifies the command is
        // fail-closed (returns Err) when invoked on a track without a catalogue file.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();
        let workspace_root = dir.path().to_path_buf();

        let result = execute_type_signals("test-track".to_owned(), workspace_root, None);
        assert!(result.is_err(), "type-signals must return error (evaluator removed in T008)");
    }

    #[test]
    fn test_execute_type_signals_with_malformed_domain_types_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (workspace_root, track_id) = setup_track(dir.path(), "{not valid json}");

        let result = execute_type_signals(track_id, workspace_root, None);
        assert!(result.is_err(), "malformed domain-types.json must return error");
    }

    #[test]
    fn test_execute_type_signals_with_unknown_layer_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();
        // Provide architecture-rules.json with only "domain" enabled so that
        // requesting layer "nonexistent" is a known-not-found error (not an IO error).
        let rules_json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } }
          ]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();
        let workspace_root = dir.path().to_path_buf();

        let result = execute_type_signals(
            "test-track".to_owned(),
            workspace_root,
            Some("nonexistent".to_owned()),
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("nonexistent"),
            "error must mention the unknown layer name, got: {msg}"
        );
        assert!(
            msg.contains("not tddd.enabled") || msg.contains("not found"),
            "error must mention tddd.enabled or not found, got: {msg}"
        );
    }

    #[test]
    fn test_execute_type_signals_with_usecase_layer_dispatches_to_usecase_catalogue() {
        // T008: the old evaluator is removed. This test verifies the command is fail-closed
        // (returns Err) when invoked with --layer usecase. The evaluator stub error is
        // returned regardless of which layer is targeted.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();

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

        let result = execute_type_signals(
            "test-track".to_owned(),
            dir.path().to_path_buf(),
            Some("usecase".to_owned()),
        );

        // T008: evaluator stub always returns Err — just verify fail-closed.
        assert!(result.is_err(), "type-signals must return error (evaluator removed in T008)");
    }

    // --- ensure_active_track tests ---

    #[test]
    fn test_ensure_active_track_rejects_done() {
        let result = ensure_active_track("done", "test-track");
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("status=done"), "msg should mention status=done: {msg}");
        assert!(msg.contains("Completed tracks are frozen"), "msg: {msg}");
        assert!(msg.contains("test-track"), "msg should mention track_id: {msg}");
    }

    #[test]
    fn test_ensure_active_track_rejects_archived() {
        let result = ensure_active_track("archived", "test-track");
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("status=archived"), "msg should mention status=archived: {msg}");
        assert!(msg.contains("Completed tracks are frozen"), "msg: {msg}");
    }

    #[test]
    fn test_ensure_active_track_allows_planned() {
        assert!(ensure_active_track("planned", "test-track").is_ok());
    }

    #[test]
    fn test_ensure_active_track_allows_in_progress() {
        assert!(ensure_active_track("in_progress", "test-track").is_ok());
    }

    #[test]
    fn test_ensure_active_track_allows_blocked() {
        assert!(ensure_active_track("blocked", "test-track").is_ok());
    }

    #[test]
    fn test_ensure_active_track_allows_cancelled() {
        assert!(ensure_active_track("cancelled", "test-track").is_ok());
    }

    // --- Integration test: execute_type_signals rejects done track via full path ---

    #[test]
    fn test_execute_type_signals_rejects_done_track() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-done-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // v5 metadata — no status field
        let done_metadata = r#"{
  "schema_version": 5,
  "id": "test-done-track",
  "branch": "track/test-done-track",
  "title": "Test Done Track",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z"
}"#;
        std::fs::write(track_dir.join("metadata.json"), done_metadata).unwrap();

        // impl-plan with all tasks done → derives TrackStatus::Done
        let done_impl_plan = r#"{
  "schema_version": 1,
  "tasks": [
    {
      "id": "T001",
      "description": "A completed task",
      "status": "done",
      "commit_hash": "0000000000000000000000000000000000000000"
    }
  ],
  "plan": {
    "summary": ["Test"],
    "sections": [{
      "id": "S001",
      "title": "Test",
      "description": ["Test"],
      "task_ids": ["T001"]
    }]
  }
}"#;
        std::fs::write(track_dir.join("impl-plan.json"), done_impl_plan).unwrap();

        let result =
            execute_type_signals("test-done-track".to_owned(), dir.path().to_path_buf(), None);

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("status=done"), "guard must mention status=done, got: {msg}");
        assert!(
            msg.contains("Completed tracks are frozen"),
            "guard must mention 'Completed tracks are frozen', got: {msg}"
        );
        assert!(msg.contains("test-done-track"), "guard must mention the track_id, got: {msg}");
    }

    #[test]
    fn test_execute_type_signals_rejects_archived_track_with_incomplete_tasks() {
        // Regression guard: schema v3 metadata (which carries status="archived") is
        // rejected at the codec decode step — schema v5 is required. This ensures that
        // legacy archived tracks (stored under v3) cannot have type-signals run on them,
        // because the metadata load fails before the active-track guard is reached.
        //
        // The `ensure_active_track` unit tests cover the archived → rejection path.
        // This integration test verifies the full path: v3 metadata decode failure → error propagation.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-archived-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // v3 metadata with status="archived" — codec::decode rejects schema_version < 5
        let archived_v3_metadata = r#"{
  "schema_version": 3,
  "id": "test-archived-track",
  "branch": "track/test-archived-track",
  "title": "Test Archived Track",
  "status": "archived",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z",
  "tasks": [],
  "plan": { "summary": [], "sections": [] }
}"#;
        std::fs::write(track_dir.join("metadata.json"), archived_v3_metadata).unwrap();

        let result =
            execute_type_signals("test-archived-track".to_owned(), dir.path().to_path_buf(), None);

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("test-archived-track"), "error must mention the track_id, got: {msg}");
    }

    #[test]
    fn test_execute_type_signals_no_layer_filter_iterates_all_enabled_bindings() {
        // T008: the old evaluator is removed and always returns Err. This test verifies
        // that invoking without --layer filter still returns an error (evaluator stub fires
        // on the first binding — domain — and propagates immediately).
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();

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

        let domain_types_json = r#"{"schema_version":2,"type_definitions":[]}"#;
        std::fs::write(track_dir.join("domain-types.json"), domain_types_json).unwrap();

        let result = execute_type_signals("test-track".to_owned(), dir.path().to_path_buf(), None);

        // T008: evaluator stub always returns Err — just verify fail-closed.
        assert!(result.is_err(), "type-signals must return error (evaluator removed in T008)");
    }

    /// Success-path integration test.  Requires nightly toolchain for `cargo +nightly rustdoc`.
    /// Run with: `cargo test --package cli -- --ignored`
    #[test]
    #[ignore]
    fn test_execute_type_signals_success_path_writes_signals() {
        let domain_types_json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true, "expected_methods": [] }
  ]
}"#;
        // Use the actual workspace root (CARGO_MANIFEST_DIR/../..) for the nightly
        // `cargo rustdoc` step, which must compile a real crate in the workspace.
        // Write all track fixtures (catalogue, metadata, impl-plan, baseline) under
        // that same workspace root so the interactor's derived
        // `workspace_root/track/items` path resolves to the same directory that
        // `setup_track` populated.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let items_dir = workspace_root.join("track/items");
        let track_id = "test-track-success-path-ignored";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("domain-types.json"), domain_types_json).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json(track_id))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();
        let baseline_json = r#"{
  "schema_version": 2,
  "captured_at": "2026-01-01T00:00:00Z",
  "types": {},
  "traits": {}
}"#;
        std::fs::write(track_dir.join("domain-types-baseline.json"), baseline_json).unwrap();

        let result = execute_type_signals(track_id.to_owned(), workspace_root, None);
        assert!(result.is_ok(), "success path must return Ok: {result:?}");

        let updated =
            std::fs::read_to_string(items_dir.join(track_id).join("domain-types.json")).unwrap();
        assert!(updated.contains("\"signals\""), "signals must be written to domain-types.json");

        let md_path = items_dir.join(track_id).join("domain-types.md");
        assert!(md_path.exists(), "domain-types.md must be generated");

        // Clean up workspace-level fixtures written by this ignored test.
        let _ = std::fs::remove_dir_all(&track_dir);
    }
}
