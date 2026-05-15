//! `sotp track baseline-capture` — capture TypeGraph snapshot as baseline.
//!
//! Generates `<layer>-types-baseline.json` from the current TypeGraph.
//! Idempotent by default: if the baseline file already exists it is kept as-is.
//! Re-capturing the baseline after implementation has started would overwrite
//! the pre-implementation snapshot with the current state, collapsing the
//! signal semantics (new `add` entries become `AddButAlreadyInBaseline` noise).
//! Use `--force` only when explicitly migrating from an older baseline format.
//!
//! `--source-workspace` lets you capture from a different Cargo workspace (e.g.
//! a git worktree at `main`) while writing baseline files into the current track dir.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use infrastructure::FsSymlinkGuard;
use infrastructure::tddd::rustdoc_baseline_capture_adapter::RustdocBaselineCaptureAdapter;
use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use usecase::baseline_capture::{
    BaselineCaptureInteractor, BaselineCaptureRequest, BaselineCaptureService,
};

use crate::CliError;

/// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
///
/// Thin CLI adapter: constructs the concrete infrastructure adapters, wires up
/// `BaselineCaptureInteractor`, and delegates all orchestration to the usecase layer.
///
/// The track items directory is derived from `workspace_root` as
/// `<workspace_root>/track/items` inside the interactor.
/// Symlink guards are applied inside the interactor via the injected
/// [`domain::SymlinkGuardPort`] (`FsSymlinkGuard`).
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, rustdoc export fails,
/// or the file write fails.
pub fn execute_baseline_capture(
    track_id: String,
    workspace_root: PathBuf,
    source_workspace: Option<PathBuf>,
    layer: Option<String>,
    force: bool,
) -> Result<ExitCode, CliError> {
    let symlink_guard = Arc::new(FsSymlinkGuard::new());
    let layer_bindings = Arc::new(FsTdddLayerBindingsAdapter::new());
    let capture = Arc::new(RustdocBaselineCaptureAdapter::new());

    let interactor = BaselineCaptureInteractor::new(symlink_guard, layer_bindings, capture);

    interactor
        .run(BaselineCaptureRequest { track_id, workspace_root, source_workspace, layer, force })
        .map_err(|e| CliError::Message(e.to_string()))?;

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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

        let result =
            execute_baseline_capture("../evil".to_owned(), dir.path().into(), None, None, false);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_baseline_capture_with_missing_arch_rules_returns_error() {
        // The interactor derives items_dir as workspace_root/track/items.
        // When architecture-rules.json is absent, layer bindings load fails.
        let workspace = tempfile::tempdir().unwrap();

        let result = execute_baseline_capture(
            "test-track".to_owned(),
            workspace.path().into(),
            None,
            None,
            false,
        );
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
            false,
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

        let result =
            execute_baseline_capture("test-track".to_owned(), dir.path().into(), None, None, false);
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

        let result = execute_baseline_capture(
            "test-track".to_owned(),
            dir.path().into(),
            None,
            Some("usecase".to_owned()),
            false,
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
            execute_baseline_capture("test-track".to_owned(), dir.path().into(), None, None, false);

        assert!(
            result.is_err(),
            "loop must continue past domain skip to usecase and fail at export; \
             Ok(SUCCESS) would mean the loop stopped after the first binding"
        );
    }
}
