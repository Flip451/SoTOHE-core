//! `sotp track baseline-capture` — capture TypeGraph snapshot as baseline.
//!
//! Thin CLI adapter: delegates all orchestration to [`cli_composition::CliApp`].

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::CliApp;

use crate::CliError;

/// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
///
/// The operation is always idempotent: if the baseline file already exists it is
/// kept as-is. To re-capture, delete the baseline file first.
///
/// # Errors
///
/// Returns `CliError` when the underlying `CliApp` composition fails.
pub fn execute_baseline_capture(
    track_id: String,
    workspace_root: PathBuf,
    source_workspace: Option<PathBuf>,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let outcome = CliApp::new()
        .track_baseline_capture(Some(track_id), workspace_root, source_workspace, layer)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use rustdoc_types::FORMAT_VERSION;

    use super::*;

    /// Minimal valid rustdoc JSON used as a stand-in baseline for idempotency tests.
    /// Required because `BaselineRustdocCodec::load` validates `format_version`.
    fn minimal_rustdoc_json() -> String {
        format!(
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
        )
    }

    #[test]
    fn test_baseline_capture_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();

        let result = execute_baseline_capture("../evil".to_owned(), dir.path().into(), None, None);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_baseline_capture_with_missing_arch_rules_returns_error() {
        // The interactor derives items_dir as workspace_root/track/items.
        // When architecture-rules.json is absent, layer bindings load fails.
        let workspace = tempfile::tempdir().unwrap();

        let result =
            execute_baseline_capture("test-track".to_owned(), workspace.path().into(), None, None);
        // workspace has no architecture-rules.json → layer bindings load fails
        assert!(result.is_err(), "missing architecture-rules.json must cause error");
    }

    #[test]
    fn test_baseline_capture_source_workspace_in_different_tree_is_not_rejected_by_containment() {
        // The `--source-workspace` (git-worktree) flow: the rustdoc source workspace is
        // a different tree. The command should not reject this configuration.
        // The command then fails later on the missing architecture-rules.json.
        let workspace = tempfile::tempdir().unwrap();
        let source_workspace = tempfile::tempdir().unwrap();

        let result = execute_baseline_capture(
            "test-track".to_owned(),
            workspace.path().into(),
            Some(source_workspace.path().into()),
            None,
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        // Should fail with layer bindings error (missing architecture-rules.json),
        // NOT with a containment or symlink error.
        assert!(
            !msg.contains("outside workspace_root"),
            "worktree flow must not be rejected by the containment check; got: {msg}"
        );
    }

    /// Initialize a minimal git repository at `root` on branch `track/<track_id>`.
    ///
    /// `resolve_track_id_for_write` requires git discovery to succeed (fail-closed
    /// branch guard). This helper creates an isolated repo so unit tests that
    /// exercise WRITE paths work without depending on the CI/dev checkout branch.
    fn init_git_repo_on_track_branch(root: &std::path::Path, track_id: &str) {
        let branch_name = format!("track/{track_id}");

        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .expect("git init failed");
        assert!(status.success(), "git init must succeed");

        for (key, value) in
            [("user.email", "test@example.com"), ("user.name", "Test"), ("commit.gpgsign", "false")]
        {
            let status = std::process::Command::new("git")
                .args(["config", key, value])
                .current_dir(root)
                .status()
                .expect("git config failed");
            assert!(status.success(), "git config {key} must succeed");
        }

        let status = std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-q", "-m", "init", "--no-gpg-sign"])
            .current_dir(root)
            .status()
            .expect("git commit failed");
        assert!(status.success(), "initial git commit must succeed");

        let status = std::process::Command::new("git")
            .args(["branch", "-m", &branch_name])
            .current_dir(root)
            .status()
            .expect("git branch -m failed");
        assert!(status.success(), "git branch -m must succeed");
    }

    #[test]
    fn test_baseline_capture_skips_when_baseline_exists() {
        let dir = tempfile::tempdir().unwrap();
        // The interactor derives items_dir as workspace_root/track/items.
        let track_dir = dir.path().join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // architecture-rules.json is required by FsTdddLayerBindingsAdapter.
        let rules_json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } }
          ]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        // Write a minimal valid rustdoc baseline so the interactor finds it and skips.
        // Idempotency now validates `format_version`, so an empty `{}` would be rejected.
        std::fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
            .unwrap();

        // resolve_track_id_for_write requires a git repository (fail-closed branch guard).
        // Bootstrap an isolated repo on the matching track branch.
        init_git_repo_on_track_branch(dir.path(), "test-track");

        let result =
            execute_baseline_capture("test-track".to_owned(), dir.path().into(), None, None);
        assert!(result.is_ok(), "should skip existing baseline without error");
    }

    #[test]
    fn test_baseline_capture_with_usecase_layer_dispatches_to_usecase_binding() {
        let dir = tempfile::tempdir().unwrap();
        // The interactor derives items_dir as workspace_root/track/items.
        let track_dir = dir.path().join("track/items/test-track");
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

        std::fs::write(track_dir.join("usecase-types-baseline.json"), minimal_rustdoc_json())
            .unwrap();

        // resolve_track_id_for_write requires a git repository (fail-closed branch guard).
        // Bootstrap an isolated repo on the matching track branch.
        init_git_repo_on_track_branch(dir.path(), "test-track");

        let result = execute_baseline_capture(
            "test-track".to_owned(),
            dir.path().into(),
            None,
            Some("usecase".to_owned()),
        );

        assert!(
            result.is_ok(),
            "dispatch to usecase binding must find existing baseline and skip, got: {result:?}"
        );
    }

    #[test]
    fn test_baseline_capture_no_layer_filter_iterates_all_enabled_bindings() {
        let dir = tempfile::tempdir().unwrap();
        // The interactor derives items_dir as workspace_root/track/items.
        let track_dir = dir.path().join("track/items/test-track");
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
        std::fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
            .unwrap();

        let result =
            execute_baseline_capture("test-track".to_owned(), dir.path().into(), None, None);

        assert!(
            result.is_err(),
            "loop must continue past domain skip to usecase and fail at export; \
             Ok(SUCCESS) would mean the loop stopped after the first binding"
        );
    }
}
