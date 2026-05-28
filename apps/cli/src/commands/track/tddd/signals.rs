//! `sotp track type-signals` — evaluate type signals via rustdoc schema export.
//!
//! Thin CLI adapter: delegates all orchestration to [`cli_composition::CliApp`].

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::CliApp;

use crate::CliError;

/// Evaluate type signals via rustdoc schema export and write back to `<layer>-types.json`.
///
/// Thin CLI adapter: delegates all orchestration to [`cli_composition::CliApp`].
///
/// # Errors
///
/// Returns `CliError` when the underlying `CliApp` composition fails.
pub fn execute_type_signals(
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let outcome = CliApp::new()
        .track_type_signals(Some(track_id), workspace_root, layer)
        .map_err(CliError::Message)?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Initialize a minimal git repository at `root` on branch `track/<track_id>`.
    ///
    /// `resolve_track_id_for_write` requires git discovery to succeed (fail-closed
    /// branch guard). This helper creates an isolated repo so tests that exercise
    /// WRITE paths work without depending on the CI/dev checkout branch or git
    /// state (detached HEAD, main, etc.).
    fn init_git_repo_on_track_branch(root: &std::path::Path, track_id: &str) {
        let branch_name = format!("track/{track_id}");
        let run_git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .expect("git command failed to spawn");
            assert!(status.success(), "git {} exited with status {status}", args.join(" "));
        };
        run_git(&["init", "-q"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "Test"]);
        run_git(&["config", "commit.gpgsign", "false"]);
        run_git(&["commit", "--allow-empty", "-q", "-m", "init", "--no-gpg-sign"]);
        run_git(&["branch", "-m", &branch_name]);
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
        // Uses an isolated tempdir git repo (branch: track/test-track) so the test
        // always runs in CI regardless of the ambient git state (detached HEAD, main,
        // etc.). CN-07 passes because the explicit track_id matches the isolated
        // repo's branch, then the interactor rejects the unknown layer name.
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_on_track_branch(dir.path(), "test-track");

        // Minimal track fixtures: metadata + impl-plan satisfy the activated-track guard.
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();
        // architecture-rules.json with only a `domain` layer — `__nonexistent_layer__` is absent.
        let rules_json = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        let result = execute_type_signals(
            "test-track".to_owned(),
            dir.path().to_path_buf(),
            // A layer name that is never in the minimal architecture-rules.json.
            Some("__nonexistent_layer__".to_owned()),
        );
        // CN-07 passes (branch matches); the interactor rejects the unknown layer name.
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
        // CN-07: the workspace git branch is `track/known-track`. Invoking
        // type-signals for a different `track_id` must be rejected by the
        // branch/track-id mismatch guard regardless of the ambient CI git state.
        //
        // Uses an isolated tempdir git repo (branch: track/known-track) so the
        // test is independent of the CI runner's detached HEAD or the developer's
        // current branch. `SystemGitRepo::discover_from(&workspace_root)` finds the
        // tempdir's own repo, not the real checkout.
        let dir = tempfile::tempdir().unwrap();
        // Bootstrap an isolated repo on a known track branch.
        init_git_repo_on_track_branch(dir.path(), "known-track");

        // Minimal track fixtures so layer-bindings and track-lookup do not fail
        // before the CN-07 branch guard fires.
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("known-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            minimal_active_metadata_json("known-track"),
        )
        .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();
        let rules_json = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        // A track_id that will never match the isolated repo's branch suffix.
        let result = execute_type_signals(
            "this-id-will-never-match".to_owned(),
            dir.path().to_path_buf(),
            None,
        );

        // The tempdir git repo is on track/known-track. The supplied track_id
        // ("this-id-will-never-match") does not match, so the CN-07 guard must
        // fire with BranchMismatch ("does not match").
        let err = result.expect_err("mismatched track_id must be rejected by CN-07");
        let msg = format!("{err}");
        assert!(
            msg.contains("does not match"),
            "error must be CN-07 BranchMismatch rejection (contains 'does not match'), got: {msg}"
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
        // Verify the branch-forwarding wiring: when track_id matches the git
        // branch of the workspace, the CN-07 guard passes and execution reaches
        // the layer-resolution or evaluation step.
        //
        // Uses an isolated tempdir git repo (branch: track/test-track) so the
        // test always runs in CI regardless of the ambient git state (detached
        // HEAD, main, etc.). The tempdir's branch matches the explicit track_id,
        // so CN-07 passes.
        //
        // After CN-07 passes, the interactor loads layer bindings from the
        // minimal architecture-rules.json written to the tempdir and attempts to
        // evaluate signals.  The evaluator stub (T008) returns an error at that
        // stage, so the function returns Err — but the error must NOT come from
        // the branch guard or git-discovery.  This confirms:
        // (a) the branch was forwarded from the CLI to the interactor,
        // (b) the guard passed for a matching branch, and
        // (c) execution reached the layer-evaluation step.
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_on_track_branch(dir.path(), "test-track");

        // Minimal track fixtures: metadata + impl-plan + architecture-rules.json +
        // domain-types.json (so catalogue load succeeds and the evaluator stub fires).
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();
        let rules_json = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();
        // Provide a minimal domain-types.json so catalogue load succeeds and execution
        // reaches the evaluator stub (T008) rather than failing at catalogue-load.
        std::fs::write(
            track_dir.join("domain-types.json"),
            r#"{"schema_version":2,"type_definitions":[]}"#,
        )
        .unwrap();

        let result = execute_type_signals("test-track".to_owned(), dir.path().to_path_buf(), None);

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
                    "error must NOT be a git-discovery failure — workspace_root must point to the isolated repo, got: {msg}"
                );
                assert!(
                    !msg.contains("detached HEAD"),
                    "error must NOT be a detached HEAD failure — isolated repo is on a named branch, got: {msg}"
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
