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
use usecase::TrackId;

use crate::CliError;
use crate::commands::track::tddd::signals::resolve_layers;

/// Per-layer refresh entry point.
///
/// Track id validation and format check are performed here; the branch-based
/// WRITE guard (AC-18 / D7) is enforced at the CLI dispatch layer via
/// `resolve_track_id_from_root_for_write` in `mod.rs` before this function is
/// called. Architecture-rules.json resolution is fail-closed via `resolve_layers`.
///
/// # Errors
///
/// Returns `CliError` when the track id is invalid, the layer filter is unknown,
/// any per-layer `<layer>-types.json` is missing or fails to decode, or the atomic
/// write fails.
pub fn execute_catalogue_spec_signals(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id format by attempting to parse it (CN-01 / AC-03).
    // This must happen before any filesystem access so that path-traversal ids
    // are rejected before touching the filesystem.
    TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID '{track_id}': {e}")))?;

    // Branch guard is enforced at the CLI dispatch layer (mod.rs) via
    // `resolve_track_id_from_root_for_write` (D7 / AC-18 / CN-02 / CN-03).
    // Inline duplication removed per T016.

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
            // Track directory absent — the layer/catalogue reads below will produce
            // a clear error message. Don't short-circuit here.
        }
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat track directory {}: {e}",
                track_dir.display()
            )));
        }
    }

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use infrastructure::tddd::catalogue_spec_signals_codec;

    use super::*;

    fn run_git(repo: &Path, args: &[&str]) {
        let output = Command::new("git").args(args).current_dir(repo).output().unwrap();
        assert!(
            output.status.success(),
            "git {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_git_repo_on_track_branch(workspace_root: &Path, track_id: &str) {
        run_git(workspace_root, &["init", "-q"]);
        run_git(workspace_root, &["config", "user.email", "test@example.invalid"]);
        run_git(workspace_root, &["config", "user.name", "Test User"]);
        let branch = format!("track/{track_id}");
        run_git(workspace_root, &["checkout", "-q", "-b", branch.as_str()]);
        run_git(workspace_root, &["commit", "--allow-empty", "-q", "-m", "init"]);
    }

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

    fn write_done_impl_plan(track_dir: &Path) {
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
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } },
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
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } }
                },
                "YellowType": {
                    "action": "add",
                    "role": "ValueObject",
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } },
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

    fn setup_matching_branch_fixture(
        workspace_root: &Path,
        track_id: &str,
        done_track: bool,
    ) -> (PathBuf, PathBuf) {
        init_git_repo_on_track_branch(workspace_root, track_id);
        let items_dir = workspace_root.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata(&track_dir, track_id);
        if done_track {
            write_done_impl_plan(&track_dir);
        } else {
            write_impl_plan(&track_dir);
        }
        write_architecture_rules(workspace_root);
        write_catalogue(&track_dir);
        (items_dir, track_dir)
    }

    #[test]
    fn refresh_rejects_path_traversal_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let items_dir = ws.join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = execute_catalogue_spec_signals(items_dir, "../evil".to_owned(), ws, None);
        // Verify the PATH-TRAVERSAL guard specifically rejected the id, not some
        // later filesystem error. domain::TrackId::try_new validates the id before any
        // git / metadata I/O occurs, so the error message always mentions the failure.
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid track ID") || err.contains("invalid"),
            "expected path-traversal rejection, got: {err}"
        );
    }

    // Note: branch mismatch rejection for catalogue-spec-signals is now enforced at the CLI
    // dispatch layer (mod.rs `resolve_track_id_from_root_for_write`) rather than inline in
    // this function (T016 — inline guard removed, duplication eliminated). Branch mismatch
    // coverage is provided by the `resolve_track_id_from_root_for_write` unit tests in mod.rs.

    /// Branch-based guard: catalogue-spec-signals on a `done` track must be allowed
    /// when the current branch is the matching track branch. The frozen/done status
    /// no longer blocks writes — only branch mismatch does (CN-04 replacement).
    #[test]
    fn refresh_allows_done_track_on_current_branch() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "done-track-2026";
        let (items_dir, track_dir) = setup_matching_branch_fixture(dir.path(), track_id, true);

        let result = execute_catalogue_spec_signals(
            items_dir,
            track_id.to_owned(),
            dir.path().to_path_buf(),
            None,
        );

        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
        assert!(track_dir.join("test_layer-catalogue-spec-signals.json").is_file());
    }

    #[test]
    fn refresh_writes_signals_for_all_entries_on_current_branch() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "test-track";
        let (items_dir, track_dir) = setup_matching_branch_fixture(dir.path(), track_id, false);

        let result = execute_catalogue_spec_signals(
            items_dir,
            track_id.to_owned(),
            dir.path().to_path_buf(),
            None,
        );

        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
        let signal_path = track_dir.join("test_layer-catalogue-spec-signals.json");
        let content = fs::read_to_string(&signal_path).unwrap();
        let decoded = catalogue_spec_signals_codec::decode(&content).unwrap();
        assert_eq!(decoded.schema_version(), 1);
        assert_eq!(decoded.signals.len(), 3);

        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        let signals = json.get("signals").and_then(serde_json::Value::as_array).unwrap();
        let signal_for = |type_name: &str| {
            signals
                .iter()
                .find(|entry| {
                    entry.get("type_name").and_then(serde_json::Value::as_str) == Some(type_name)
                })
                .and_then(|entry| entry.get("signal").and_then(serde_json::Value::as_str))
        };
        assert_eq!(signal_for("BlueType"), Some("blue"));
        assert_eq!(signal_for("YellowType"), Some("yellow"));
        assert_eq!(signal_for("RedType"), Some("red"));
    }

    #[test]
    fn refresh_rejects_unknown_layer_filter() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "test-track";
        let (items_dir, _track_dir) = setup_matching_branch_fixture(dir.path(), track_id, false);

        let result = execute_catalogue_spec_signals(
            items_dir,
            track_id.to_owned(),
            dir.path().to_path_buf(),
            Some("__missing_layer__".to_owned()),
        );

        let err = result.expect_err("unknown layer must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("not tddd.enabled"),
            "error must come from layer resolution, got: {msg}"
        );
    }
}
