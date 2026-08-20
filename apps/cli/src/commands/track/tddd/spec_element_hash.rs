//! `sotp track spec-element-hash` — emit canonical SHA-256 hashes for spec.json elements.
//!
//! Thin CLI adapter: delegates all orchestration to the composition root in `cli_composition`.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackItemsDirectoryInput, TrackSpecAnchorInput, TrackTdddInput, TrackTdddSpecElementHashInput,
};

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
    let track_id = track_id
        .parse::<TrackIdInput>()
        .map_err(|error| CliError::Message(format!("invalid track id: {error}")))?;
    let items_dir = TrackItemsDirectoryInput::try_new(items_dir)
        .map_err(|error| CliError::Message(error.to_string()))?;
    let anchor = anchor
        .map(TrackSpecAnchorInput::try_new)
        .transpose()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let outcome =
        TrackCompositionRoot::new().track_tddd_driver().handle(TrackTdddInput::SpecElementHash(
            TrackTdddSpecElementHashInput { track_id: Some(track_id), items_dir, anchor },
        ));
    super::super::state_ops::track_driver_outcome_to_result(outcome)
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

    fn list_relative_files(root: &std::path::Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir).expect("read fixture dir") {
                let path = entry.expect("fixture dir entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(
                        path.strip_prefix(root)
                            .expect("relative fixture path")
                            .display()
                            .to_string(),
                    );
                }
            }
        }
        files.sort();
        files
    }

    fn spec_element_hash_call_site_outcome(
        root: &std::path::Path,
        track_id: &str,
        anchor: Option<&str>,
    ) -> cli_driver::CommandOutcome {
        let track_id = track_id.parse::<TrackIdInput>().expect("track id is valid");
        let items_dir = TrackItemsDirectoryInput::try_new(root.join("track/items"))
            .expect("items directory is valid");
        let anchor = anchor
            .map(|value| TrackSpecAnchorInput::try_new(value.to_owned()))
            .transpose()
            .expect("anchor is valid");
        TrackCompositionRoot::new().track_tddd_driver().handle(TrackTdddInput::SpecElementHash(
            TrackTdddSpecElementHashInput { track_id: Some(track_id), items_dir, anchor },
        ))
    }

    #[test]
    fn test_track_spec_element_hash_call_site_preserves_cli_contract_across_migration() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let root = workspace.path();
        let (items_dir, track_dir) = setup_track(root, "hash-track");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).expect("spec document is written");
        let before = list_relative_files(root);

        let argv_items_dir = items_dir.clone();
        let argv_track_id = "hash-track".to_owned();
        let argv_anchor = Some("GL-01".to_owned());
        let cli_exit = execute_spec_element_hash(
            argv_items_dir.clone(),
            argv_track_id.clone(),
            argv_anchor.clone(),
        )
        .expect("legacy CLI argv must remain accepted");
        assert_eq!(cli_exit, ExitCode::from(0));
        assert_eq!(argv_items_dir, root.join("track/items"));
        assert_eq!(argv_track_id, "hash-track");
        assert_eq!(argv_anchor.as_deref(), Some("GL-01"));

        let outcome = spec_element_hash_call_site_outcome(root, "hash-track", Some("GL-01"));
        assert_eq!(outcome.exit_code, 0);
        let hash = outcome.stdout.expect("single-anchor lookup writes stdout");
        assert_eq!(hash.len(), 64, "single-anchor output must be one SHA-256 hash");
        assert!(hash.chars().all(|character| character.is_ascii_hexdigit()));
        assert_eq!(outcome.stderr, None, "successful lookup must not write stderr");
        assert_eq!(
            list_relative_files(root),
            before,
            "spec-element-hash must not persist extra files"
        );
    }
}
