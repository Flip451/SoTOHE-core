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

use crate::CliError;
use crate::commands::track::tddd::signals::resolve_layers;

/// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
///
/// Steps:
/// 1. Resolve the set of TDDD-enabled layers to process (all enabled, or just the
///    specified `--layer`).
/// 2. For each layer binding, check if the baseline already exists (skip if so,
///    unless `force` is true).
/// 3. Export the target crate schema via rustdoc JSON from `source_workspace`
///    (defaults to `workspace_root` when not supplied).
/// 4. Write raw rustdoc JSON to `<layer>-types-baseline.json`.
///
/// When `--layer` is omitted, all TDDD-enabled layers are processed in `layers[]` order.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, rustdoc export fails,
/// or the file write fails.
pub fn execute_baseline_capture(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    source_workspace: Option<PathBuf>,
    layer: Option<String>,
    force: bool,
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

    // Security: confine `items_dir` to `workspace_root` (the current workspace — the
    // root used for `architecture-rules.json` resolution, not the optional rustdoc
    // `source_workspace`). Without this, a user/config-supplied `--items-dir ../outside`
    // would let the derived `<track_dir>/<layer>-types-baseline.json` write target escape
    // the workspace. Compare canonicalized forms so `..` traversal and symlinked path
    // components cannot bypass the check. `source_workspace` is intentionally NOT used as
    // the containment root: in the git-worktree capture flow it points at a different tree
    // (e.g. a `main` worktree) while `items_dir` still lives under the current workspace.
    let canonical_items_dir = items_dir.canonicalize().map_err(|e| {
        CliError::Message(format!("cannot canonicalize items_dir {}: {e}", items_dir.display()))
    })?;
    let canonical_workspace_root = workspace_root.canonicalize().map_err(|e| {
        CliError::Message(format!(
            "cannot canonicalize workspace_root {}: {e}",
            workspace_root.display()
        ))
    })?;
    if !canonical_items_dir.starts_with(&canonical_workspace_root) {
        return Err(CliError::Message(format!(
            "items_dir {} is outside workspace_root {}; only paths under the workspace are allowed",
            canonical_items_dir.display(),
            canonical_workspace_root.display()
        )));
    }

    // Resolve the rustdoc source workspace (defaults to workspace_root when not supplied).
    let rustdoc_workspace = source_workspace.as_deref().unwrap_or(&workspace_root);

    for binding in &bindings {
        if force {
            infrastructure::tddd::baseline_capture::force_capture_rustdoc_baseline_for_layer(
                &items_dir,
                &track_id,
                rustdoc_workspace,
                binding,
            )
            .map_err(|e| CliError::Message(e.0))?;
        } else {
            infrastructure::tddd::baseline_capture::capture_rustdoc_baseline_for_layer(
                &items_dir,
                &track_id,
                rustdoc_workspace,
                binding,
            )
            .map_err(|e| CliError::Message(e.0))?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rustdoc_types::FORMAT_VERSION;

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
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_baseline_capture(
            items_dir,
            "../evil".to_owned(),
            dir.path().into(),
            None,
            None,
            false,
        );
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_baseline_capture_with_items_dir_outside_workspace_root_returns_error() {
        // Regression for the Codex PR#132 finding: `--items-dir` pointing at a path
        // outside `workspace_root` must be rejected so the derived baseline write
        // target cannot escape the workspace.
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap(); // sibling tree, not under `workspace`
        let outside_items = outside.path().join("track/items");
        std::fs::create_dir_all(&outside_items).unwrap();

        let result = execute_baseline_capture(
            outside_items,
            "test-track".to_owned(),
            workspace.path().into(),
            None,
            None,
            false,
        );
        let err = result.unwrap_err();
        assert!(
            matches!(&err, CliError::Message(m) if m.contains("outside workspace_root")),
            "items_dir outside workspace_root must be rejected; got: {err:?}"
        );
    }

    #[test]
    fn test_baseline_capture_source_workspace_in_different_tree_is_not_rejected_by_containment() {
        // The `--source-workspace` (git-worktree) flow: `items_dir` lives under
        // `workspace_root`, but the rustdoc source workspace is a different tree.
        // The containment check confines `items_dir` to `workspace_root` only — it must
        // not reject this configuration. The command then fails later on the missing
        // track directory (proving the containment check passed, not on a containment error).
        let workspace = tempfile::tempdir().unwrap();
        let items_dir = workspace.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let source_workspace = tempfile::tempdir().unwrap(); // different tree

        let result = execute_baseline_capture(
            items_dir,
            "test-track".to_owned(),
            workspace.path().into(),
            Some(source_workspace.path().into()),
            None,
            false,
        );
        let err = result.unwrap_err();
        assert!(
            matches!(&err, CliError::Message(m) if m.contains("track directory not found")),
            "worktree flow must not be rejected by the containment check; got: {err:?}"
        );
    }

    #[test]
    fn test_baseline_capture_skips_when_baseline_exists() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write a minimal valid rustdoc baseline (domain layer — default when no
        // architecture-rules.json). Idempotency now validates `format_version`, so an empty
        // `{}` is rejected before the skip path is reached.
        std::fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
            .unwrap();

        let result = execute_baseline_capture(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            None,
            None,
            false,
        );
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
        std::fs::write(track_dir.join("usecase-types-baseline.json"), minimal_rustdoc_json())
            .unwrap();

        let result = execute_baseline_capture(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            None,
            Some("usecase".to_owned()),
            false,
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
        std::fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
            .unwrap();
        // do NOT write usecase-types-baseline.json

        let result = execute_baseline_capture(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            None,
            None,
            false,
        );

        assert!(
            result.is_err(),
            "loop must continue past domain skip to usecase and fail at export; \
             Ok(SUCCESS) would mean the loop stopped after the first binding"
        );
    }
}
