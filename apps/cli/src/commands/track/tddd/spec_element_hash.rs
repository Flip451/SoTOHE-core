//! `sotp track spec-element-hash` — emit canonical SHA-256 hashes for
//! spec.json elements so type-designer can author `spec_refs[].hash`
//! values for catalogue entries (Cat-Spec Blue promotion).
//!
//! Thin CLI wrapper: validates the track id, delegates all I/O (including
//! the `symlink_metadata` guard on `items_dir`) to
//! `infrastructure::track::spec_element_hash::compute_spec_element_hashes`,
//! then maps the result to stdout output plus an exit code.

use std::path::PathBuf;
use std::process::ExitCode;

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
    // Validate the track_id without importing domain::TrackId (CN-01 / AC-03).
    crate::commands::track::validate_track_id_str(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Delegate all I/O (symlink guard on items_dir, spec.json load, hash computation)
    // to the infrastructure layer. CLI is wiring + output formatting only.
    let hashes = infrastructure::track::spec_element_hash::compute_spec_element_hashes(
        items_dir,
        &track_id,
        anchor.as_deref(),
    )
    .map_err(|e| CliError::Message(e.0))?;

    match anchor {
        Some(anchor_id) => {
            // compute_spec_element_hashes already validated the anchor; the map has exactly
            // one entry when an anchor is requested and succeeded.
            if let Some(hash) = hashes.get(&anchor_id) {
                println!("{hash}");
                Ok(ExitCode::SUCCESS)
            } else {
                // Should not be reached (infrastructure returns Err on missing anchor).
                Err(CliError::Message(format!("anchor '{anchor_id}' not found in spec.json")))
            }
        }
        None => {
            let json = serde_json::to_string_pretty(&hashes)
                .map_err(|e| CliError::Message(format!("JSON encode error: {e}")))?;
            println!("{json}");
            Ok(ExitCode::SUCCESS)
        }
    }
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

    // Note: the raw-hash computation test (formerly test_compute_hashes_from_raw_*) has
    // been moved to infrastructure::track::spec_element_hash tests, where the logic lives.

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
        assert!(msg.contains("spec.json"), "error should mention spec.json read failure: {msg}");
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
