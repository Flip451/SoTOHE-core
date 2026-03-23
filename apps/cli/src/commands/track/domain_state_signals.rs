//! `sotp track domain-state-signals` — evaluate domain state signals by scanning domain code.
//!
//! Reads `spec.json`, scans domain code with the syn AST scanner, evaluates
//! per-state confidence signals, writes the results back into `spec.json` (SSoT),
//! and re-renders `spec.md`.
//!
//! If `spec.json` does not exist, the command returns an error rather than
//! falling back to legacy `spec.md`.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::{ConfidenceSignal, evaluate_domain_state_signals};
use infrastructure::spec::codec as spec_codec;
use infrastructure::spec::domain_scanner::scan_domain_directory;
use infrastructure::track::atomic_write::atomic_write_file;

use crate::CliError;

/// Evaluate domain state signals by scanning domain code and store the results in `spec.json`.
///
/// Steps:
/// 1. Read `spec.json` from `{items_dir}/{track_id}/spec.json`.
/// 2. Scan `domain_dir` using `scan_domain_directory`.
/// 3. Call `evaluate_domain_state_signals(doc.domain_states(), &scan)`.
/// 4. Store the results on the doc via `doc.set_domain_state_signals(signals)`.
/// 5. Write the updated `spec.json` back atomically.
/// 6. Re-render `spec.md` from the updated doc.
/// 7. Print a per-state summary.
///
/// # Errors
///
/// Returns `CliError` when `spec.json` is missing, cannot be decoded, the domain
/// directory cannot be scanned, or any write fails.
pub fn execute_domain_state_signals(
    items_dir: PathBuf,
    track_id: String,
    domain_dir: PathBuf,
) -> Result<ExitCode, CliError> {
    // Validate track_id to prevent path traversal.
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    let track_dir = items_dir.join(&track_id);
    let spec_json_path = track_dir.join("spec.json");

    if !spec_json_path.is_file() {
        return Err(CliError::Message(format!(
            "spec.json not found at {}; domain-state-signals requires spec.json (no legacy fallback)",
            spec_json_path.display()
        )));
    }

    // Read and decode spec.json.
    let json_content = std::fs::read_to_string(&spec_json_path)
        .map_err(|e| CliError::Message(format!("cannot read {}: {e}", spec_json_path.display())))?;

    let mut doc = spec_codec::decode(&json_content)
        .map_err(|e| CliError::Message(format!("spec.json decode error: {e}")))?;

    // Scan domain code.
    let scan = scan_domain_directory(&domain_dir).map_err(|e| {
        CliError::Message(format!("cannot scan domain directory {}: {e}", domain_dir.display()))
    })?;

    // Evaluate per-state signals.
    let signals = evaluate_domain_state_signals(doc.domain_states(), &scan);

    // Write results back into the doc.
    doc.set_domain_state_signals(signals.clone());

    // Encode and write spec.json.
    let encoded = spec_codec::encode(&doc)
        .map_err(|e| CliError::Message(format!("spec.json encode error: {e}")))?;

    atomic_write_file(&spec_json_path, format!("{encoded}\n").as_bytes()).map_err(|e| {
        CliError::Message(format!("cannot write {}: {e}", spec_json_path.display()))
    })?;

    // Re-render spec.md from the updated doc.
    let rendered_spec = infrastructure::spec::render::render_spec(&doc);
    let spec_md_path = track_dir.join("spec.md");
    atomic_write_file(&spec_md_path, rendered_spec.as_bytes())
        .map_err(|e| CliError::Message(format!("cannot write {}: {e}", spec_md_path.display())))?;

    // Print per-state summary.
    for sig in &signals {
        let signal_icon = signal_icon(sig.signal());
        let type_mark = if sig.found_type() { "✓" } else { "✗" };
        let transition_detail = format_transition_detail(sig);
        println!(
            "[domain-state-signals] {}: {signal_icon} (type: {type_mark}{transition_detail})",
            sig.state_name(),
        );
    }

    // Print aggregate summary.
    let blue_count = signals.iter().filter(|s| s.signal() == ConfidenceSignal::Blue).count();
    let yellow_count = signals.iter().filter(|s| s.signal() == ConfidenceSignal::Yellow).count();
    let red_count = signals.iter().filter(|s| s.signal() == ConfidenceSignal::Red).count();
    println!("[domain-state-signals] Summary: 🔵 {blue_count}  🟡 {yellow_count}  🔴 {red_count}");

    Ok(ExitCode::SUCCESS)
}

/// Returns the emoji icon for a confidence signal.
fn signal_icon(signal: ConfidenceSignal) -> &'static str {
    match signal {
        ConfidenceSignal::Blue => "🔵",
        ConfidenceSignal::Yellow => "🟡",
        ConfidenceSignal::Red => "🔴",
        // ConfidenceSignal is #[non_exhaustive]; future variants fall back to "?".
        _ => "?",
    }
}

/// Formats transition detail for display.
///
/// Returns an empty string if the type was not found (no meaningful transition info).
/// Otherwise formats found/missing transitions.
fn format_transition_detail(sig: &domain::DomainStateSignal) -> String {
    if !sig.found_type() {
        return String::new();
    }

    let found = sig.found_transitions();
    let missing = sig.missing_transitions();

    if found.is_empty() && missing.is_empty() {
        // Terminal state or no declared transitions.
        return String::new();
    }

    let mut parts: Vec<String> = Vec::new();

    for t in found {
        parts.push(format!("{t} ✓"));
    }
    for t in missing {
        parts.push(format!("{t} ✗"));
    }

    if parts.is_empty() { String::new() } else { format!(", transitions: {}", parts.join(", ")) }
}

// ---------------------------------------------------------------------------
// Tests (TDD Red → Green → Refactor)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Helper: create a minimal spec.json with domain_states
    // ---------------------------------------------------------------------------

    fn write_spec_json_with_states(track_dir: &std::path::Path, spec_json: &str) {
        std::fs::write(track_dir.join("spec.json"), spec_json).unwrap();
    }

    fn setup_track(dir: &std::path::Path, spec_json: &str) -> (PathBuf, String, PathBuf) {
        let items_dir = dir.join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        write_spec_json_with_states(&track_dir, spec_json);

        // Create an empty domain_dir (no .rs files → all types missing)
        let domain_dir = dir.join("domain_src");
        std::fs::create_dir_all(&domain_dir).unwrap();

        (items_dir, track_id.to_owned(), domain_dir)
    }

    // ---------------------------------------------------------------------------
    // T005-T001: spec.json not found returns error
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_domain_state_signals_with_missing_spec_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let domain_dir = dir.path().join("domain_src");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&domain_dir).unwrap();

        let result = execute_domain_state_signals(items_dir, "test-track".to_owned(), domain_dir);

        assert!(result.is_err(), "missing spec.json must return error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("spec.json not found") || err_msg.contains("not found"),
            "error should mention spec.json: {err_msg}"
        );
    }

    // ---------------------------------------------------------------------------
    // T005-T002: path traversal track_id is rejected
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_domain_state_signals_rejects_path_traversal_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let domain_dir = dir.path().join("domain_src");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&domain_dir).unwrap();

        let result = execute_domain_state_signals(items_dir, "../evil".to_owned(), domain_dir);

        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    // ---------------------------------------------------------------------------
    // T005-T003: signals are written into spec.json
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_domain_state_signals_writes_signals_to_spec_json() {
        let dir = tempfile::tempdir().unwrap();

        let spec_json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature X",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "domain_states": [
    {"name": "Draft", "description": "Initial state", "transitions_to": null}
  ]
}"#;
        let (items_dir, track_id, domain_dir) = setup_track(dir.path(), spec_json);

        // Write a .rs file with the Draft type in domain_dir so signal is Yellow
        // (type found, transitions_to is null → Yellow)
        std::fs::write(domain_dir.join("states.rs"), "pub struct Draft;").unwrap();

        let result = execute_domain_state_signals(items_dir.clone(), track_id.clone(), domain_dir);

        assert!(result.is_ok(), "execute must succeed: {result:?}");

        let updated_json =
            std::fs::read_to_string(items_dir.join(&track_id).join("spec.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&updated_json).unwrap();

        // domain_state_signals must be present in the updated spec.json
        let dss = &parsed["domain_state_signals"];
        assert!(dss.is_array(), "domain_state_signals must be an array in spec.json");
        let first = &dss[0];
        assert_eq!(first["state_name"].as_str().unwrap(), "Draft");
        // Draft type found, transitions_to=null → Yellow
        assert_eq!(first["signal"].as_str().unwrap(), "yellow");
    }

    // ---------------------------------------------------------------------------
    // T005-T004: spec.md is regenerated after signal update
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_domain_state_signals_regenerates_spec_md() {
        let dir = tempfile::tempdir().unwrap();

        let spec_json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Y",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "domain_states": [
    {"name": "Active", "description": "Active state", "transitions_to": []}
  ]
}"#;
        let (items_dir, track_id, domain_dir) = setup_track(dir.path(), spec_json);

        // Write Active type so signal is Blue (found, terminal)
        std::fs::write(domain_dir.join("active.rs"), "pub struct Active;").unwrap();

        execute_domain_state_signals(items_dir.clone(), track_id.clone(), domain_dir).unwrap();

        let spec_md_path = items_dir.join(&track_id).join("spec.md");
        assert!(spec_md_path.exists(), "spec.md must be generated");

        let spec_md = std::fs::read_to_string(&spec_md_path).unwrap();
        assert!(
            spec_md.contains("<!-- Generated from spec.json"),
            "spec.md must be a generated view"
        );
        assert!(spec_md.contains("Feature Y"), "spec.md must contain the title");
    }

    // ---------------------------------------------------------------------------
    // T005-T005: Red signal when type not in domain code
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_domain_state_signals_red_when_type_missing_from_code() {
        let dir = tempfile::tempdir().unwrap();

        let spec_json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Z",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "domain_states": [
    {"name": "Ghost", "description": "Missing state", "transitions_to": null}
  ]
}"#;
        // domain_dir is empty (no .rs files), so Ghost type won't be found → Red
        let (items_dir, track_id, domain_dir) = setup_track(dir.path(), spec_json);

        execute_domain_state_signals(items_dir.clone(), track_id.clone(), domain_dir).unwrap();

        let updated_json =
            std::fs::read_to_string(items_dir.join(&track_id).join("spec.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&updated_json).unwrap();

        let dss = &parsed["domain_state_signals"];
        assert!(dss.is_array());
        let first = &dss[0];
        assert_eq!(first["state_name"].as_str().unwrap(), "Ghost");
        assert_eq!(first["signal"].as_str().unwrap(), "red");
    }

    // ---------------------------------------------------------------------------
    // T005-T006: Blue signal when type found and all declared transitions present
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_domain_state_signals_blue_when_type_and_transitions_found() {
        let dir = tempfile::tempdir().unwrap();

        // Published must also be listed in domain_states (codec validates transitions reference
        // known states in the spec).
        let spec_json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Blue",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "domain_states": [
    {"name": "Draft", "description": "Draft state", "transitions_to": ["Published"]},
    {"name": "Published", "description": "Published state", "transitions_to": []}
  ]
}"#;
        let (items_dir, track_id, domain_dir) = setup_track(dir.path(), spec_json);

        // Write types + transition function so signal is Blue
        std::fs::write(
            domain_dir.join("states.rs"),
            r#"
pub struct Draft;
pub struct Published;
pub struct Error;
impl Draft {
    pub fn publish(self) -> Result<Published, Error> { todo!() }
}
"#,
        )
        .unwrap();

        execute_domain_state_signals(items_dir.clone(), track_id.clone(), domain_dir).unwrap();

        let updated_json =
            std::fs::read_to_string(items_dir.join(&track_id).join("spec.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&updated_json).unwrap();

        let dss = &parsed["domain_state_signals"];
        assert!(dss.is_array());
        let first = &dss[0];
        assert_eq!(first["state_name"].as_str().unwrap(), "Draft");
        assert_eq!(first["signal"].as_str().unwrap(), "blue");
    }

    // ---------------------------------------------------------------------------
    // T005-T007: malformed spec.json returns error
    // ---------------------------------------------------------------------------

    #[test]
    fn test_execute_domain_state_signals_with_malformed_spec_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(track_dir.join("spec.json"), "{not valid json}").unwrap();

        let domain_dir = dir.path().join("domain_src");
        std::fs::create_dir_all(&domain_dir).unwrap();

        let result = execute_domain_state_signals(items_dir, track_id.to_owned(), domain_dir);

        assert!(result.is_err(), "malformed spec.json must return error");
    }
}
