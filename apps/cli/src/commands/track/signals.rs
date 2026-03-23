//! `sotp track signals` — evaluate spec.md source tags and store results in metadata.json.

use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::track::atomic_write::atomic_write_file;
use infrastructure::track::codec;
use infrastructure::verify::frontmatter::parse_yaml_frontmatter;
use infrastructure::verify::spec_signals::evaluate;

use crate::CliError;

/// Evaluate spec.md source tags, store the result in metadata.json `spec_signals`,
/// and print a summary.
///
/// # Errors
///
/// Returns `CliError` when the file cannot be read, the track cannot be loaded,
/// or the write fails.
pub fn execute_signals(items_dir: PathBuf, track_id: String) -> Result<ExitCode, CliError> {
    // Validate track_id to prevent path traversal
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Read spec.md
    let track_dir = items_dir.join(&track_id);
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

    // Read metadata.json, update spec_signals, write back
    let metadata_path = track_dir.join("metadata.json");
    let json_content = std::fs::read_to_string(&metadata_path)
        .map_err(|e| CliError::Message(format!("cannot read {}: {e}", metadata_path.display())))?;

    let (track, mut meta) =
        codec::decode(&json_content).map_err(|e| CliError::Message(format!("{e}")))?;

    // Update timestamp
    meta.updated_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Store spec_signals in the extra map (backward-compatible, no schema bump)
    let signals_value = serde_json::json!({
        "blue": counts.blue(),
        "yellow": counts.yellow(),
        "red": counts.red()
    });
    meta.extra.insert("spec_signals".to_owned(), signals_value);

    let encoded = codec::encode(&track, &meta).map_err(|e| CliError::Message(format!("{e}")))?;

    atomic_write_file(&metadata_path, format!("{encoded}\n").as_bytes())
        .map_err(|e| CliError::Message(format!("cannot write {}: {e}", metadata_path.display())))?;

    let total = counts.total();
    println!(
        "[OK] Signals: blue={} yellow={} red={} (total={total})",
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

    fn setup_track(dir: &std::path::Path, spec_content: &str) -> (PathBuf, String) {
        let items_dir = dir.join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write spec.md
        std::fs::write(track_dir.join("spec.md"), spec_content).unwrap();

        // Write minimal metadata.json
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

        (items_dir, track_id.to_owned())
    }

    #[test]
    fn test_execute_signals_writes_spec_signals_to_metadata() {
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

        // Verify metadata.json contains spec_signals
        let metadata_content =
            std::fs::read_to_string(items_dir.join(&track_id).join("metadata.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&metadata_content).unwrap();
        let signals = &doc["spec_signals"];
        assert_eq!(signals["blue"], 1);
        assert_eq!(signals["yellow"], 1);
        assert_eq!(signals["red"], 0);
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
}
