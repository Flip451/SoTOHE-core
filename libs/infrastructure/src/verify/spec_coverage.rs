//! Verify requirement-to-task coverage for a specific track.
//!
//! CI gate: every in_scope and acceptance_criteria requirement must have
//! at least one valid task_ref, and all task_refs must reference valid tasks
//! in metadata.json.
//!
//! Track resolution is the caller's responsibility (CLI resolves from branch
//! name or explicit `--track-id`). This module does not guess "latest track".

use std::collections::HashSet;
use std::path::Path;

use domain::TaskId;
use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::spec::codec as spec_codec;
use crate::track::codec as track_codec;

/// Run spec coverage verification for the given track directory.
///
/// The `track_dir` must point to a directory containing `spec.json` and
/// `metadata.json` (e.g., `track/items/<id>`).
///
/// Skips with pass if the track has no `spec.json`.
///
/// # Errors
///
/// Returns error findings when coverage is incomplete or task_refs
/// reference non-existent tasks.
pub fn verify(track_dir: &Path) -> VerifyOutcome {
    let spec_json_path = track_dir.join("spec.json");
    if !spec_json_path.is_file() {
        return VerifyOutcome::pass();
    }

    let metadata_path = track_dir.join("metadata.json");
    if !metadata_path.is_file() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "spec.json found but metadata.json missing in {}",
            track_dir.display()
        ))]);
    }

    // Load spec.json
    let spec_content = match std::fs::read_to_string(&spec_json_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read spec.json: {e}"
            ))]);
        }
    };
    let spec_doc = match spec_codec::decode(&spec_content) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot parse spec.json: {e}"
            ))]);
        }
    };

    // Load metadata.json to get task IDs
    let meta_content = match std::fs::read_to_string(&metadata_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read metadata.json: {e}"
            ))]);
        }
    };
    let (track_meta, _doc_meta) = match track_codec::decode(&meta_content) {
        Ok(m) => m,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot parse metadata.json: {e}"
            ))]);
        }
    };

    // Collect valid task IDs from metadata
    let valid_task_ids: HashSet<TaskId> =
        track_meta.tasks().iter().map(|t| t.id().clone()).collect();

    // Evaluate coverage (in_scope + acceptance_criteria enforcement)
    let result = spec_doc.evaluate_coverage(&valid_task_ids);

    let mut findings: Vec<VerifyFinding> = Vec::new();

    for text in result.uncovered() {
        findings.push(VerifyFinding::error(format!("Requirement missing task_refs: \"{text}\"")));
    }

    // Referential integrity for ALL sections (domain logic)
    for ref_id in spec_doc.validate_all_task_refs(&valid_task_ids) {
        findings.push(VerifyFinding::error(format!(
            "task_ref \"{ref_id}\" does not exist in metadata.json tasks"
        )));
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    const TRACK_ITEMS_DIR: &str = "track/items";

    fn setup_track(root: &Path, name: &str, tasks_json: &str, spec_json: &str) {
        let dir = root.join(TRACK_ITEMS_DIR).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let meta = format!(
            r#"{{
  "schema_version": 3,
  "id": "{name}",
  "title": "Test",
  "status": "in_progress",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "branch": "track/{name}",
  "tasks": {tasks_json},
  "plan": {{ "summary": [], "sections": [{{ "id": "S1", "title": "Section", "description": [], "task_ids": ["T001"] }}] }}
}}"#
        );
        std::fs::write(dir.join("metadata.json"), meta).unwrap();
        if !spec_json.is_empty() {
            std::fs::write(dir.join("spec.json"), spec_json).unwrap();
        }
    }

    fn track_dir(root: &Path, name: &str) -> std::path::PathBuf {
        root.join(TRACK_ITEMS_DIR).join(name)
    }

    #[test]
    fn test_no_spec_json_passes() {
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            "",
        );
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_nonexistent_track_dir_passes() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(&tmp.path().join("nonexistent"));
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_fully_covered_passes() {
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"item","sources":["PRD"],"task_refs":["T001"]}],"out_of_scope":[]},"acceptance_criteria":[{"text":"AC","sources":["PRD"],"task_refs":["T001"]}]}"#,
        );
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_uncovered_requirement_fails() {
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"missing refs","sources":["PRD"]}],"out_of_scope":[]}}"#,
        );
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_invalid_task_ref_fails() {
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"item","sources":["PRD"],"task_refs":["T999"]}],"out_of_scope":[]}}"#,
        );
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_constraint_without_task_refs_passes() {
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"constraints":[{"text":"constraint","sources":["convention"]}]}"#,
        );
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_omitted_task_refs_defaults_to_empty() {
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"no refs field","sources":["PRD"]}],"out_of_scope":[]}}"#,
        );
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.has_errors());
    }
}
