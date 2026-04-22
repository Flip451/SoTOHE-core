//! `sotp track signals` — evaluate spec source tags and store results.
//!
//! When `spec.json` exists, signals are evaluated via `SpecDocument::evaluate_signals()`
//! and written back into `spec.json` (SSoT path).
//!
//! When only `spec.md` exists (legacy mode), signals are evaluated from the markdown body
//! and stored in `metadata.json` `spec_signals` (backward-compatible path).

use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::spec::codec as spec_codec;
use infrastructure::track::atomic_write::atomic_write_file;
use infrastructure::verify::frontmatter::parse_yaml_frontmatter;
use infrastructure::verify::spec_signals::evaluate;

use crate::CliError;

/// Evaluate spec source tags, store the result, and print a summary.
///
/// When `spec.json` is present the signals are written back into `spec.json`.
/// When only `spec.md` is present (legacy), they are stored in `metadata.json`.
///
/// # Errors
///
/// Returns `CliError` when the file cannot be read, the track cannot be loaded,
/// or the write fails.
pub fn execute_signals(items_dir: PathBuf, track_id: String) -> Result<ExitCode, CliError> {
    // Validate track_id to prevent path traversal
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    let track_dir = items_dir.join(&track_id);
    let spec_json_path = track_dir.join("spec.json");

    if spec_json_path.is_file() {
        execute_signals_via_spec_json(&track_dir, &spec_json_path)
    } else {
        execute_signals_legacy(&track_dir)
    }
}

/// New path: evaluate signals from spec.json and write them back into spec.json.
fn execute_signals_via_spec_json(
    track_dir: &std::path::Path,
    spec_json_path: &std::path::Path,
) -> Result<ExitCode, CliError> {
    let json_content = std::fs::read_to_string(spec_json_path)
        .map_err(|e| CliError::Message(format!("cannot read {}: {e}", spec_json_path.display())))?;

    let mut doc = spec_codec::decode(&json_content)
        .map_err(|e| CliError::Message(format!("spec.json decode error: {e}")))?;

    let counts = doc.evaluate_signals();
    doc.set_signals(counts);

    let encoded = spec_codec::encode(&doc)
        .map_err(|e| CliError::Message(format!("spec.json encode error: {e}")))?;

    atomic_write_file(spec_json_path, format!("{encoded}\n").as_bytes()).map_err(|e| {
        CliError::Message(format!("cannot write {}: {e}", spec_json_path.display()))
    })?;

    // Regenerate spec.md from the updated spec.json — do this before printing [OK]
    // so a write failure returns Err without having already claimed success.
    let rendered_spec = infrastructure::spec::render::render_spec(&doc);
    let spec_md_path = track_dir.join("spec.md");
    atomic_write_file(&spec_md_path, rendered_spec.as_bytes())
        .map_err(|e| CliError::Message(format!("cannot write {}: {e}", spec_md_path.display())))?;

    let total = counts.total();
    println!(
        "[OK] Signals (spec.json): blue={} yellow={} red={} (total={total})",
        counts.blue(),
        counts.yellow(),
        counts.red()
    );

    Ok(ExitCode::SUCCESS)
}

/// Legacy path: evaluate signals from spec.md and store in metadata.json `spec_signals`.
fn execute_signals_legacy(track_dir: &std::path::Path) -> Result<ExitCode, CliError> {
    let spec_path = track_dir.join("spec.md");
    let content = std::fs::read_to_string(&spec_path)
        .map_err(|e| CliError::Message(format!("cannot read {}: {e}", spec_path.display())))?;

    // Parse frontmatter to get body start
    let fm = parse_yaml_frontmatter(&content).ok_or_else(|| {
        CliError::Message(format!("{}: missing or invalid YAML frontmatter", spec_path.display()))
    })?;

    // Evaluate body
    let lines: Vec<&str> = content.lines().collect();
    let body_lines = lines.get(fm.body_start..).unwrap_or_default();
    let body = body_lines.join("\n");
    let counts = evaluate(&body);

    // Spec signals are not stored in metadata.json; they are computed on demand.
    let total = counts.total();
    println!(
        "[OK] Signals (legacy): blue={} yellow={} red={} (total={total})",
        counts.blue(),
        counts.yellow(),
        counts.red()
    );

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Shared helpers
    // ---------------------------------------------------------------------------

    fn write_metadata(track_dir: &std::path::Path, track_id: &str) {
        let metadata = serde_json::json!({
            "schema_version": 3,
            "id": track_id,
            "branch": null,
            "title": "Test Track",
            "status": "planned",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "tasks": [{"id": "T001", "description": "Test task", "status": "todo"}],
            "plan": {
                "summary": ["Test"],
                "sections": [{"id": "s1", "title": "S1", "description": [], "task_ids": ["T001"]}]
            }
        });
        std::fs::write(
            track_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
    }

    /// Legacy setup: creates spec.md + metadata.json (no spec.json).
    fn setup_track(dir: &std::path::Path, spec_content: &str) -> (PathBuf, String) {
        let items_dir = dir.join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(track_dir.join("spec.md"), spec_content).unwrap();
        write_metadata(&track_dir, track_id);

        (items_dir, track_id.to_owned())
    }

    /// New-path setup: creates spec.json + metadata.json (no spec.md).
    fn setup_track_with_spec_json(dir: &std::path::Path, spec_json: &str) -> (PathBuf, String) {
        let items_dir = dir.join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(track_dir.join("spec.json"), spec_json).unwrap();
        write_metadata(&track_dir, track_id);

        (items_dir, track_id.to_owned())
    }

    // ---------------------------------------------------------------------------
    // Legacy (spec.md) tests — kept as-is
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_signals_writes_spec_signals_to_metadata() {
        // spec_signals are not written to metadata.json; the legacy path only prints to stdout.
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_id) = setup_track(
            dir.path(),
            concat!(
                "---\nstatus: draft\nversion: \"1.0\"\n---\n",
                "## Scope\n",
                "- item one [source: PRD §1]\n",
                "- item two [source: inference — guess]\n",
            ),
        );

        let result = execute_signals(items_dir.clone(), track_id.clone());
        assert!(result.is_ok());

        // spec_signals are computed and printed but not persisted to metadata.json.
        let metadata_content =
            std::fs::read_to_string(items_dir.join(&track_id).join("metadata.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&metadata_content).unwrap();
        assert!(
            doc.get("spec_signals").is_none(),
            "spec_signals must not be written to metadata.json in schema_version 4"
        );
    }

    #[test]
    fn test_execute_signals_with_nonexistent_spec_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_signals(items_dir, "nonexistent".to_owned());
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_signals_with_missing_frontmatter_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_id) = setup_track(dir.path(), "## Scope\n- item [source: PRD §1]\n");

        // Overwrite spec.md without frontmatter
        std::fs::write(
            items_dir.join(&track_id).join("spec.md"),
            "## Scope\n- item [source: PRD §1]\n",
        )
        .unwrap();

        let result = execute_signals(items_dir, track_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_signals_rejects_path_traversal_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_signals(items_dir, "../evil".to_owned());
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    // ---------------------------------------------------------------------------
    // New-path (spec.json) tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_signals_via_spec_json_writes_signals_into_spec_json() {
        let dir = tempfile::tempdir().unwrap();
        // Schema v2: two in-scope requirements — one Blue (adr_ref), one Yellow (informal)
        let spec_json = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature X",
  "scope": {
    "in_scope": [
      {"id": "IN-01", "text": "Req A", "adr_refs": [{"file": "adr/x.md", "anchor": "D1"}]},
      {"id": "IN-02", "text": "Req B", "informal_grounds": [{"kind": "discussion", "summary": "agreed"}]}
    ],
    "out_of_scope": []
  }
}"#;
        let (items_dir, track_id) = setup_track_with_spec_json(dir.path(), spec_json);

        let result = execute_signals(items_dir.clone(), track_id.clone());
        assert!(result.is_ok(), "execute_signals via spec.json must succeed");

        // spec.json must contain signals field
        let updated_json =
            std::fs::read_to_string(items_dir.join(&track_id).join("spec.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&updated_json).unwrap();
        let signals = &parsed["signals"];
        assert_eq!(signals["blue"], 1, "adr_ref should be blue");
        assert_eq!(signals["yellow"], 1, "informal should be yellow");
        assert_eq!(signals["red"], 0);
    }

    #[test]
    fn test_execute_signals_via_spec_json_also_generates_spec_md() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature Y",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;
        let (items_dir, track_id) = setup_track_with_spec_json(dir.path(), spec_json);

        execute_signals(items_dir.clone(), track_id.clone()).unwrap();

        let spec_md_path = items_dir.join(&track_id).join("spec.md");
        assert!(spec_md_path.exists(), "spec.md must be generated after spec.json signal update");

        let spec_md = std::fs::read_to_string(&spec_md_path).unwrap();
        assert!(spec_md.contains("<!-- Generated from spec.json"));
        assert!(spec_md.contains("Feature Y"));
    }

    #[test]
    fn test_execute_signals_prefers_spec_json_over_spec_md_when_both_exist() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature Z",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;
        let (items_dir, track_id) = setup_track_with_spec_json(dir.path(), spec_json);

        // Also write a legacy spec.md with frontmatter
        std::fs::write(
            items_dir.join(&track_id).join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\n- item [source: PRD §1]\n",
        )
        .unwrap();

        execute_signals(items_dir.clone(), track_id.clone()).unwrap();

        // metadata.json must NOT have spec_signals (spec.json path was taken)
        let metadata_content =
            std::fs::read_to_string(items_dir.join(&track_id).join("metadata.json")).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&metadata_content).unwrap();
        assert!(
            meta.get("spec_signals").is_none(),
            "spec_signals must NOT be written to metadata.json when spec.json is present"
        );

        // spec.json must have signals
        let updated_json =
            std::fs::read_to_string(items_dir.join(&track_id).join("spec.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&updated_json).unwrap();
        assert!(parsed.get("signals").is_some(), "signals must be written into spec.json");
    }

    #[test]
    fn test_execute_signals_via_spec_json_with_malformed_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(track_dir.join("spec.json"), "{not valid}").unwrap();
        write_metadata(&track_dir, track_id);

        let result = execute_signals(items_dir, track_id.to_owned());
        assert!(result.is_err(), "malformed spec.json must return an error");
    }
}
