//! `sotp verify catalogue-spec-refs` — binary gate for SoT Chain ② integrity
//! (dangling anchor / stale signals).
//!
//! Thin CLI wrapper: delegates to
//! `cli_composition::verify::execute_catalogue_spec_refs` and maps the
//! `CommandOutcome` result to a process exit code.
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D1.5 / §D3.2 / §D3.6 / IN-10.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::CliError;

/// Entry point for `sotp verify catalogue-spec-refs`.
///
/// Used by integration tests only — production dispatch is handled by
/// `cli_composition::CliApp::verify_catalogue_spec_refs`.
///
/// Delegates to `cli_composition::verify::execute_catalogue_spec_refs` and maps
/// the `CommandOutcome` exit code to an `ExitCode`, forwarding any stdout/stderr
/// to the appropriate streams.
///
/// # Errors
///
/// Returns `CliError` when the track id is invalid or any fatal I/O /
/// configuration error occurs. Integrity violations are NOT reported via
/// `Err` — they are printed to stderr and reflected in the exit code
/// (non-zero on any finding).
pub fn execute_verify_catalogue_spec_refs(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    skip_stale: bool,
) -> Result<ExitCode, CliError> {
    let outcome = cli_composition::verify::execute_catalogue_spec_refs(
        items_dir,
        track_id,
        workspace_root,
        skip_stale,
    )
    .map_err(|e| CliError::Message(e.to_string()))?;

    if let Some(stdout) = &outcome.stdout {
        println!("{stdout}");
    }
    if let Some(stderr) = &outcome.stderr {
        eprintln!("{stderr}");
    }
    if outcome.exit_code == 0 { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    fn write_architecture_rules(root: &Path) {
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
            root.join("architecture-rules.json"),
            serde_json::to_string_pretty(&rules).unwrap(),
        )
        .unwrap();
    }

    fn write_spec_json(track_dir: &Path) {
        let spec = serde_json::json!({
            "schema_version": 2,
            "version": "1.0",
            "title": "Test",
            "scope": {
                "in_scope": [{"id": "IN-01", "text": "Requirement A"}],
                "out_of_scope": []
            }
        });
        fs::write(track_dir.join("spec.json"), serde_json::to_string_pretty(&spec).unwrap())
            .unwrap();
    }

    fn write_catalogue_with_dangling(track_dir: &Path) {
        // v5-native format required by CatalogueDocumentCodec::decode.
        let cat = serde_json::json!({
            "schema_version": 5,
            "crate_name": "test_layer",
            "layer": "test_layer",
            "types": {
                "BadType": {
                    "action": "add",
                    "role": { "ValueObject": {} },
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } },
                    "spec_refs": [
                        {
                            "file": "track/items/x/spec.json",
                            "anchor": "IN-99"
                        }
                    ]
                }
            },
            "traits": {},
            "functions": {}
        });
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn verify_exits_0_when_no_catalogue_entries() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);
        // Catalogue has no entries → no findings.
        let cat = serde_json::json!({
            "schema_version": 5,
            "crate_name": "test_layer",
            "layer": "test_layer",
            "types": {},
            "traits": {},
            "functions": {}
        });
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
    }

    #[test]
    fn verify_exits_failure_when_dangling_anchor_present() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);
        write_catalogue_with_dangling(&track_dir);

        let result = execute_verify_catalogue_spec_refs(
            items_dir,
            track_id.to_owned(),
            ws,
            true, // skip stale to isolate dangling detection
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::FAILURE);
    }

    #[test]
    fn verify_rejects_path_traversal_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let items_dir = ws.join("track/items");
        fs::create_dir_all(&items_dir).unwrap();
        write_architecture_rules(&ws);

        let result = execute_verify_catalogue_spec_refs(items_dir, "../evil".to_owned(), ws, true);
        assert!(result.is_err());
    }

    // Fail-closed regression guard: a non-existent track directory (typo or
    // stale CI variable) must NOT be silently swallowed by the Phase 0/1
    // catalogue-absent gate. Without an explicit existence check, every
    // catalogue path under the missing directory would resolve as absent and
    // `any_enabled_catalogue_present` would return false, producing a false
    // PASS. The verifier must surface a clear error instead.
    #[test]
    fn verify_fails_when_track_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let items_dir = ws.join("track/items");
        fs::create_dir_all(&items_dir).unwrap();
        write_architecture_rules(&ws);
        // Deliberately do NOT create the track directory.

        let result =
            execute_verify_catalogue_spec_refs(items_dir, "no-such-track".to_owned(), ws, true);
        assert!(result.is_err(), "non-existent track directory must fail-closed: {result:?}");
    }

    // ADR D2.3: catalogue absent + spec.json absent → silent PASS (Phase 0/1).
    // No catalogue means SoT Chain ② is not yet active, so the missing
    // spec.json is not a violation. Mirrors the `validate_track_snapshots`
    // file-existence-driven phase model.
    #[test]
    fn verify_passes_when_catalogue_absent_and_spec_absent() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        // No spec.json AND no catalogue → Phase 0/1 state.

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(
            result.is_ok(),
            "Phase 0/1 (no catalogue, no spec.json) must produce silent PASS: {result:?}"
        );
        assert_eq!(result.unwrap(), ExitCode::SUCCESS, "Phase 0/1 must produce zero findings");
    }

    // ADR D2.3: catalogue present + spec.json absent → FAIL (SoT Chain ②).
    // The catalogue's spec_refs[] cite anchor ids in spec.json — without
    // spec.json, ref integrity cannot be verified. Treat as a hard error to
    // surface the contract violation rather than silently bypassing.
    #[test]
    fn verify_fails_when_catalogue_present_and_spec_absent() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        // Catalogue present (any non-empty entry forces the spec.json read path).
        write_catalogue_with_dangling(&track_dir);
        // Deliberately no spec.json.

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(
            result.is_err(),
            "catalogue present + spec.json absent must FAIL (SoT Chain ② violation)"
        );
    }

    // Absent catalogue file for a layer must be silently skipped (lenient CI path).
    // This is distinct from an empty catalogue: the file does not exist at all.
    #[test]
    fn verify_exits_0_when_catalogue_file_absent_for_layer() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);
        // Deliberately do NOT write `test_layer-types.json`.

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(result.is_ok(), "absent catalogue file must not be an error: {result:?}");
        assert_eq!(
            result.unwrap(),
            ExitCode::SUCCESS,
            "absent catalogue file must produce zero findings"
        );
    }

    /// Build a temp workspace with an empty catalogue and a stale signals file.
    ///
    /// Returns `(dir, items_dir, track_id, ws)` where `dir` is the [`tempfile::TempDir`]
    /// that must be kept alive for the duration of the test.  The workspace contains:
    /// - `architecture-rules.json` (single `test_layer` with catalogue-spec-signal enabled)
    /// - `spec.json` under the track directory
    /// - An empty `test_layer-types.json` (no spec_refs → no dangling anchor findings)
    /// - `test_layer-catalogue-spec-signals.json` with an all-zero
    ///   `catalogue_declaration_hash`, guaranteed to mismatch the actual hash
    fn setup_empty_catalogue_with_stale_signals() -> (tempfile::TempDir, PathBuf, String, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track".to_owned();
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(&track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);

        // Empty catalogue — no spec_refs → no dangling anchor findings regardless of signals.
        let cat = serde_json::json!({
            "schema_version": 5,
            "crate_name": "test_layer",
            "layer": "test_layer",
            "types": {},
            "traits": {},
            "functions": {}
        });
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();

        // Stale signals file: all-zero catalogue_declaration_hash guarantees a
        // StaleSignals finding when the signals file is consulted.
        let stale_signals = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "signals": []
        });
        fs::write(
            track_dir.join("test_layer-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&stale_signals).unwrap(),
        )
        .unwrap();

        (dir, items_dir, track_id, ws)
    }

    // `--skip-stale` must prevent reading `<layer>-catalogue-spec-signals.json`
    // even when that file exists.  A stale-signals finding from the domain layer
    // would be the only finding if the signals file were read — so EXIT_SUCCESS
    // with skip_stale=true proves the signals file was not consulted.
    #[test]
    fn verify_skip_stale_bypasses_signals_read() {
        let (_dir, items_dir, track_id, ws) = setup_empty_catalogue_with_stale_signals();

        // With skip_stale=true, the signals file must NOT be read → no stale finding.
        let result = execute_verify_catalogue_spec_refs(items_dir, track_id, ws, true);
        assert!(result.is_ok(), "skip_stale must not error: {result:?}");
        assert_eq!(
            result.unwrap(),
            ExitCode::SUCCESS,
            "skip_stale=true must bypass signals read and produce zero findings"
        );
    }

    // When skip_stale=false and the signals file exists with a mismatched
    // declaration_hash, a StaleSignals finding must be produced (exit FAILURE).
    #[test]
    fn verify_exits_failure_when_stale_signals_detected() {
        let (_dir, items_dir, track_id, ws) = setup_empty_catalogue_with_stale_signals();

        // With skip_stale=false the signals file IS read → stale hash → FAILURE.
        let result = execute_verify_catalogue_spec_refs(items_dir, track_id, ws, false);
        assert!(result.is_ok(), "stale signals must not error, just return FAILURE: {result:?}");
        assert_eq!(
            result.unwrap(),
            ExitCode::FAILURE,
            "stale catalogue-spec-signals must produce a finding and exit FAILURE"
        );
    }

    // format_finding tests moved to infrastructure::verify::catalogue_spec_refs.
    // The CLI now delegates formatting to the infrastructure helper.
}
