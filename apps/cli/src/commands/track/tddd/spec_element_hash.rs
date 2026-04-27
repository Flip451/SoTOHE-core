//! `sotp track spec-element-hash` — emit canonical SHA-256 hashes for
//! spec.json elements so type-designer can author `spec_refs[].hash`
//! values for catalogue entries (Cat-Spec Blue promotion).
//!
//! Reuses `infrastructure::verify::plan_artifact_refs::build_element_map`
//! + `canonical_json_sha256` so the digests match the format consumed by
//!   `sotp verify catalogue-spec-refs`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::track::symlink_guard::reject_symlinks_below;

use crate::CliError;

/// Print canonical SHA-256 hashes for spec.json elements.
///
/// When `anchor` is `Some`, prints the single hash on stdout (or returns an
/// error if the anchor is absent). When `anchor` is `None`, prints a JSON
/// object mapping every element id to its hash, sorted by id.
///
/// # Errors
///
/// Returns `CliError` when the spec.json is absent, fails to parse, or the
/// requested anchor is not present.
pub fn execute_spec_element_hash(
    items_dir: PathBuf,
    track_id: String,
    anchor: Option<String>,
) -> Result<ExitCode, CliError> {
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Security: verify the items_dir root itself is not a symlink before using it as the
    // trusted anchor for `reject_symlinks_below`. That helper only checks components
    // *below* the trusted_root, so a symlinked items_dir would bypass all path guards.
    // Mirrors `execute_catalogue_spec_signals` (catalogue_spec_signals.rs).
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

    // Security: verify the track directory itself is not a symlink before joining
    // spec.json beneath it. A symlinked track directory would escape the trusted tree
    // before `reject_symlinks_below` (anchored at `items_dir`) can catch it.
    // Mirrors `execute_catalogue_spec_signals` (catalogue_spec_signals.rs).
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
            // Track directory absent — the spec.json read below will produce a
            // clear error message. Don't short-circuit here.
        }
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat track directory {}: {e}",
                track_dir.display()
            )));
        }
    }

    let spec_path = track_dir.join("spec.json");

    // Security: reject symlinks at spec.json and every ancestor below items_dir.
    reject_symlinks_below(&spec_path, &items_dir).map_err(|e| {
        CliError::Message(format!(
            "symlink guard: refusing to read spec.json at '{}': {e}",
            spec_path.display()
        ))
    })?;

    let text = std::fs::read_to_string(&spec_path).map_err(|e| {
        CliError::Message(format!("cannot read spec.json at '{}': {e}", spec_path.display()))
    })?;

    // Validate schema first so malformed spec.json fails closed instead of
    // emitting a partial hash map (mirrors verify_catalogue_spec_refs.rs).
    infrastructure::spec::codec::decode(&text).map_err(|e| {
        CliError::Message(format!("spec.json schema error at '{}': {e}", spec_path.display()))
    })?;

    let raw: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| CliError::Message(format!("spec.json JSON parse error: {e}")))?;

    let hashes = compute_hashes_from_raw(&raw);

    match anchor {
        Some(anchor_id) => match hashes.get(&anchor_id) {
            Some(hash) => {
                println!("{hash}");
                Ok(ExitCode::SUCCESS)
            }
            None => Err(CliError::Message(format!(
                "anchor '{anchor_id}' not found in spec.json (available: {})",
                hashes.keys().cloned().collect::<Vec<_>>().join(", ")
            ))),
        },
        None => {
            let json = serde_json::to_string_pretty(&hashes)
                .map_err(|e| CliError::Message(format!("JSON encode error: {e}")))?;
            println!("{json}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Build a sorted map from spec element id to canonical SHA-256 hex.
///
/// Extracted for unit-test access: callers can verify both the keys and the
/// 64-char hex format of every value without capturing stdout.
fn compute_hashes_from_raw(raw: &serde_json::Value) -> BTreeMap<String, String> {
    let element_map = infrastructure::verify::plan_artifact_refs::build_element_map(raw);
    let mut hashes: BTreeMap<String, String> = BTreeMap::new();
    for (id, canonical) in element_map {
        let hash = infrastructure::verify::plan_artifact_refs::canonical_json_sha256(&canonical);
        hashes.insert(id, hash);
    }
    hashes
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use super::*;

    /// Minimal valid spec.json (schema version 2) with one goal element and one
    /// in-scope element. Used across multiple test cases.
    ///
    /// Element IDs must match `<UPPER>{2,}-<digits>+` (e.g. GL-01, IN-01).
    const VALID_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Test Spec",
  "goal": [
    {"id": "GL-01", "text": "First goal"}
  ],
  "scope": {
    "in_scope": [
      {"id": "IN-01", "text": "In scope item"}
    ],
    "out_of_scope": []
  }
}"#;

    fn setup_track(base: &std::path::Path, track_id: &str) -> (PathBuf, PathBuf) {
        let items_dir = base.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        (items_dir, track_dir)
    }

    #[test]
    fn test_compute_hashes_from_raw_produces_64_char_lowercase_hex_per_element() {
        // Verifies that every value in the output map is a 64-char lowercase hex string
        // (i.e., the output of canonical_json_sha256) and that both spec sections are
        // covered (goal + scope.in_scope).
        let raw: serde_json::Value = serde_json::from_str(VALID_SPEC_JSON).unwrap();
        let hashes = compute_hashes_from_raw(&raw);

        // VALID_SPEC_JSON has GL-01 (goal) and IN-01 (in_scope).
        assert_eq!(hashes.len(), 2, "expected two elements: GL-01 and IN-01");
        assert!(hashes.contains_key("GL-01"), "GL-01 must be present");
        assert!(hashes.contains_key("IN-01"), "IN-01 must be present");

        for (id, hex) in &hashes {
            assert_eq!(hex.len(), 64, "hash for '{id}' must be 64 chars, got {}: {hex}", hex.len());
            assert!(
                hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
                "hash for '{id}' must be lowercase hex: {hex}"
            );
        }
    }

    #[test]
    fn test_execute_spec_element_hash_with_no_anchor_returns_success() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).unwrap();

        let result = execute_spec_element_hash(items_dir, "my-track-2026-04-26".to_owned(), None);
        assert!(result.is_ok(), "should succeed with no anchor: {result:?}");
    }

    #[test]
    fn test_execute_spec_element_hash_with_anchor_returns_success() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).unwrap();

        let result = execute_spec_element_hash(
            items_dir,
            "my-track-2026-04-26".to_owned(),
            Some("GL-01".to_owned()),
        );
        assert!(result.is_ok(), "should succeed for known anchor: {result:?}");
    }

    #[test]
    fn test_execute_spec_element_hash_with_missing_anchor_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).unwrap();

        let result = execute_spec_element_hash(
            items_dir,
            "my-track-2026-04-26".to_owned(),
            Some("NONEXISTENT".to_owned()),
        );
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("NONEXISTENT"), "error should mention missing anchor: {msg}");
    }

    #[test]
    fn test_execute_spec_element_hash_with_missing_spec_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, _track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        // No spec.json written.

        let result = execute_spec_element_hash(items_dir, "my-track-2026-04-26".to_owned(), None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("cannot read spec.json"), "error should mention read failure: {msg}");
    }

    #[test]
    fn test_execute_spec_element_hash_with_schema_invalid_spec_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        // schema_version 1 is not accepted by spec::codec::decode.
        fs::write(
            track_dir.join("spec.json"),
            r#"{"schema_version": 1, "version": "1.0", "title": "X",
                "scope": {"in_scope": [], "out_of_scope": []}}"#,
        )
        .unwrap();

        let result = execute_spec_element_hash(items_dir, "my-track-2026-04-26".to_owned(), None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("spec.json schema error"),
            "error should mention schema failure: {msg}"
        );
    }

    #[test]
    fn test_execute_spec_element_hash_rejects_path_traversal_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = execute_spec_element_hash(items_dir, "../evil".to_owned(), None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("invalid track ID"),
            "error should mention path-traversal rejection: {msg}"
        );
    }
}
