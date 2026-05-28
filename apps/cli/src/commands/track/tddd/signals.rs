//! `sotp track type-signals` — evaluate type signals via rustdoc schema export.
//!
//! Reads `<layer>-types.json` from the track directory, exports the target crate's
//! public API via rustdoc JSON, evaluates signals for each declared type, and writes
//! the updated document back to `<layer>-types.json`.
//!
//! `resolve_layers` remains in this module as a shared helper for sibling CLI commands.
//! `execute_type_signals_lenient_with_bindings` stays here to allow
//! `commands/make.rs` to share a single architecture-rules.json parse (TOCTOU
//! prevention).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use infrastructure::tddd::type_signals_evaluator::{
    EvaluateSignalsError, execute_type_signals_for_layer,
};
use infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
use infrastructure::verify::tddd_layers::{
    LoadTdddLayersError, TdddLayerBinding, load_tddd_layers,
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
/// - When `architecture-rules.json` is absent, returns an error (fail-closed).
pub(crate) fn resolve_layers(
    workspace_root: &Path,
    layer_filter: Option<&str>,
) -> Result<Vec<TdddLayerBinding>, CliError> {
    let rules_path = workspace_root.join("architecture-rules.json");
    // Delegate symlink handling to the shared infrastructure helper (fail-closed).
    // CLI stays a thin composition layer; it only maps the infra error variants
    // into `CliError` and applies the CLI-level layer filter.
    let bindings = load_tddd_layers(&rules_path, workspace_root).map_err(|e| match e {
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
/// 2. Guard the active track by branch match (reject when the current branch
///    does not equal `track/<id>`).
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

    // Resolve the current git branch (CN-07 active-track guard requires it),
    // rooted at workspace_root so `--workspace-root` is always honoured.
    let branch = SystemGitRepo::discover_from(&workspace_root)
        .map_err(|e| CliError::Message(format!("cannot discover git repo: {e}")))?
        .current_branch()
        .map_err(|e| CliError::Message(format!("cannot read current branch: {e}")))?
        .ok_or_else(|| {
            CliError::Message(
                "cannot read current branch: git rev-parse --abbrev-ref HEAD returned non-zero"
                    .to_owned(),
            )
        })?;

    let layer_bindings = Arc::new(FsTdddLayerBindingsAdapter::new());
    let executor = Arc::new(TypeSignalsExecutorAdapter::new());

    let interactor = TypeSignalsInteractor::new(layer_bindings, executor);

    interactor
        .run(TypeSignalsRequest {
            items_dir,
            track_id,
            branch,
            workspace_root,
            layer,
            lenient: false,
        })
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
/// The active-track branch guard is enforced by the dispatch layer
/// (`resolve_track_id_from_root_for_write`) before this function is called.
/// Only `architecture-rules.json` fail-closed via `resolve_layers` and
/// empty-bindings fail-closed are checked here.
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
/// loop. The active-track branch guard is enforced in the dispatch layer
/// before this function is called; no duplicate check is performed here.
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
    // The active-track branch guard (CN-07: branch must match `track/<track_id>`)
    // is enforced by the dispatch layer via `resolve_track_id_from_root_for_write`
    // before this function is called. No inline guard is performed here to avoid
    // duplicating the guard semantics outside the centralized dispatch path.

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

    /// Returns the current git branch's track-id suffix (the part after `track/`)
    /// if the working directory is on a `track/<id>` branch, or `None` otherwise
    /// (e.g. detached HEAD, `main`, non-track branches).
    ///
    /// Tests that require the branch guard to *pass* use this helper to derive
    /// the track_id at runtime, making them independent of which specific branch
    /// name is checked out when the test suite is run.
    fn current_track_id_suffix() -> Option<String> {
        let repo = SystemGitRepo::discover().ok()?;
        let branch = repo.current_branch().ok()??;
        branch.strip_prefix("track/").map(|s| s.to_owned())
    }

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
    /// a valid `metadata.json` (activated, branch set), a minimal `impl-plan.json`,
    /// and a minimal `architecture-rules.json` so the fail-closed
    /// `FsTdddLayerBindingsAdapter::new()` resolves layer bindings before reaching
    /// the catalogue/evaluator path.
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
        // architecture-rules.json is required by FsTdddLayerBindingsAdapter::new()
        // (fail-closed). Without it the interactor fails before reaching the
        // catalogue/evaluator path that each caller test is asserting on.
        let rules_json = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;
        std::fs::write(workspace_root.join("architecture-rules.json"), rules_json).unwrap();
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
        // architecture-rules.json is required by FsTdddLayerBindingsAdapter::new()
        // (fail-closed). Without it the interactor fails on layer-bindings load
        // rather than on the missing catalogue path this test exercises.
        let rules_json = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();
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
        // Verify that the `--layer` argument is forwarded through the CLI into the
        // usecase interactor and that an unknown layer is rejected by the interactor's
        // layer-resolution step.
        //
        // Reads the current branch at runtime so that the track_id used to pass CN-07
        // is not hard-coded: the test is skipped when the checkout is not on a
        // `track/<id>` branch (detached HEAD, `main`, CI branches), preventing
        // environment-dependent failures.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root from CARGO_MANIFEST_DIR")
            .to_path_buf();

        // Derive the track_id from the ambient branch at test runtime.
        let Some(track_id) = current_track_id_suffix() else {
            // Not on a track/ branch — skip this test.
            return;
        };

        let result = execute_type_signals(
            // Runtime-derived track_id — CN-07 passes because branch matches.
            track_id,
            workspace_root,
            // A layer name that is never in architecture-rules.json.
            Some("__nonexistent_layer__".to_owned()),
        );
        // CN-07 passes; the interactor rejects the unknown layer name.
        let err = result.expect_err("unknown layer must be rejected by the interactor");
        let msg = format!("{err}");
        assert!(
            msg.contains("not tddd.enabled") || msg.contains("not found"),
            "error must be an unknown-layer rejection from the interactor, got: {msg}"
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

    // --- Integration test: execute_type_signals CN-07 branch guard ---

    #[test]
    fn test_execute_type_signals_rejects_non_matching_track_id() {
        // CN-07: the current git branch is `track/<some-id>`. Invoking
        // type-signals for a different `track_id` must be rejected by the
        // branch/track-id mismatch guard (regardless of track status).
        //
        // Use the real workspace root (derived from CARGO_MANIFEST_DIR) so the
        // workspace-alignment guard passes; the CN-07 guard then fires because
        // the supplied track_id does not match the current branch suffix.
        // The error message must contain CN-07 text to pin the branch-forwarding
        // wiring from the CLI into the interactor.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root from CARGO_MANIFEST_DIR")
            .to_path_buf();

        // A track_id that will never match the current branch suffix.
        let result = execute_type_signals(
            "this-id-will-never-match-the-real-branch".to_owned(),
            workspace_root,
            None,
        );

        // SystemGitRepo::discover() finds the actual git branch (workspace CWD).
        // The current branch does not match the supplied track_id, so the usecase
        // CN-07 guard must fire with BranchTrackMismatch ("does not match track_id")
        // or NonActiveTrack ("not an active track branch" on main/detached HEAD).
        let err = result.expect_err("mismatched track_id must be rejected by CN-07");
        let msg = format!("{err}");
        assert!(
            msg.contains("does not match") || msg.contains("not an active track branch"),
            "error must be CN-07 branch guard rejection (BranchTrackMismatch or NonActiveTrack), got: {msg}"
        );
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

    #[test]
    fn test_execute_type_signals_branch_guard_passes_for_current_track() {
        // Verify the branch-forwarding wiring: when track_id matches the current git
        // branch suffix, the CN-07 guard passes and execution reaches the layer-resolution
        // or evaluation step.
        //
        // Reads the current branch at runtime to derive the track_id, so this test is
        // independent of which specific branch name is checked out (not hard-coded to a
        // particular track). Skipped on non-track/ branches (detached HEAD, main, CI).
        //
        // After CN-07 passes, the interactor loads layer bindings from the real
        // architecture-rules.json and then attempts to evaluate signals.  The
        // evaluator stub (T008) returns an error at that stage, so the function
        // returns Err — but the error must come from the evaluation step (containing
        // "evaluation failed" or "evaluator") not from the branch guard. This
        // confirms: (a) the branch was forwarded from the CLI to the interactor,
        // (b) the guard passed for a matching branch, and (c) execution reached the
        // layer-evaluation step.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root from CARGO_MANIFEST_DIR")
            .to_path_buf();

        // Derive the track_id from the ambient branch at test runtime.
        let Some(track_id) = current_track_id_suffix() else {
            // Not on a track/ branch (detached HEAD, main, CI) — skip.
            return;
        };

        let result = execute_type_signals(track_id, workspace_root, None);

        // Verify the branch guard passed: the function must NOT return a CN-07 rejection or
        // a git-discovery failure.  The function may succeed (evaluation reached and passed)
        // or fail at the evaluation stage — both outcomes confirm the guard passed.
        match &result {
            Ok(_exit) => {
                // Guard passed and evaluation succeeded — no further assertion needed.
            }
            Err(err) => {
                let msg = format!("{err}");
                assert!(
                    !msg.contains("does not match") && !msg.contains("not an active track branch"),
                    "error must NOT be a CN-07 branch guard rejection — guard should have passed, got: {msg}"
                );
                assert!(
                    !msg.contains("cannot discover git repo"),
                    "error must NOT be a git-discovery failure — CWD must point to the real workspace, got: {msg}"
                );
                // The error must originate from the evaluation stage (after CN-07 + layer resolution).
                assert!(
                    msg.contains("evaluation failed")
                        || msg.contains("evaluator")
                        || msg.contains("EvaluationFailed"),
                    "error must come from the evaluation stage (confirming interactor was reached), got: {msg}"
                );
            }
        }
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
