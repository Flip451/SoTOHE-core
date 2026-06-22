//! `sotp track spec-element-hash` — emit canonical SHA-256 hashes for spec.json elements.
//!
//! Thin CLI adapter: delegates all orchestration to [`cli_composition::CliApp`].

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::CliApp;

use crate::CliError;

/// Print canonical SHA-256 hashes for spec.json elements.
///
/// When `anchor` is `Some`, prints the single hash on stdout (or returns an
/// error if the anchor is absent). When `anchor` is `None`, prints a JSON
/// object mapping every element id to its hash, sorted by id.
///
/// # Errors
///
/// Returns `CliError` when the underlying `CliApp` composition fails.
pub fn execute_spec_element_hash(
    items_dir: PathBuf,
    track_id: String,
    anchor: Option<String>,
) -> Result<ExitCode, CliError> {
    let outcome = CliApp::new()
        .track_spec_element_hash(items_dir, Some(track_id), anchor)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
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
        // Error text is the domain form: "track id '...' must be a lowercase slug".
        // Accept either the domain form or legacy "invalid track id" prefix (behaviour: rejection).
        assert!(
            msg.contains("must be a lowercase slug")
                || msg.to_ascii_lowercase().contains("invalid track id"),
            "error should mention path-traversal rejection: {msg}"
        );
    }
}
