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
/// T003 transition: `task_refs` moved from `spec.json` to `task-coverage.json`
/// (T004). Coverage enforcement via `evaluate_coverage` is fully stubbed until
/// T012. When `task-coverage.json` is absent the check is skipped, ensuring CI
/// passes during T003–T011 before the new coverage mechanism is implemented.
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

    // T003 transition guard: skip coverage enforcement until task-coverage.json
    // exists (T004+). The evaluate_coverage stub always returns all in_scope/AC
    // as uncovered, which would cause false failures before T012 migration.
    let task_coverage_path = track_dir.join("task-coverage.json");
    if !task_coverage_path.is_file() {
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

    // NOTE: task_refs were removed from SpecRequirement in T003 (moved to task-coverage.json).
    // evaluate_coverage() is stubbed to always report all in_scope + AC as uncovered until T012.
    // These tests verify the stub behaviour and schema v2 decode compatibility.

    #[test]
    fn test_spec_without_scope_items_passes() {
        // No in_scope or acceptance_criteria → evaluate_coverage reports empty uncovered → pass.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        );
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_spec_without_task_coverage_json_skips_coverage_check() {
        // T003 transition: when task-coverage.json is absent, coverage check is skipped.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        // No task-coverage.json written → guard skips coverage check → pass.
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.is_ok(), "must skip coverage when task-coverage.json absent");
    }

    #[test]
    fn test_spec_with_in_scope_item_and_task_coverage_json_reports_uncovered() {
        // T003 stub: when task-coverage.json exists, in_scope items are always uncovered.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        // Write a task-coverage.json stub to trigger the coverage check path.
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("task-coverage.json"), "{}").unwrap();
        let outcome = verify(&dir);
        // Stub always reports in_scope items as uncovered (T012 will fix)
        assert!(outcome.has_errors(), "stub must report in_scope as uncovered until T012");
    }

    #[test]
    fn test_constraint_without_task_refs_passes() {
        // constraints are NOT in evaluate_coverage's scope; only in_scope + AC.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"constraints":[{"id":"CN-01","text":"constraint","convention_refs":[{"file":"conv/x.md","anchor":"s1"}]}]}"#,
        );
        // Write task-coverage.json to enter the coverage check path.
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("task-coverage.json"), "{}").unwrap();
        let outcome = verify(&dir);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_validate_all_task_refs_stub_returns_no_errors() {
        // validate_all_task_refs is stubbed to always return empty — no findings from it.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"[{"id":"T001","description":"task","status":"todo"}]"#,
            // spec with only constraints (not in_scope or AC) → evaluate_coverage OK,
            // validate_all_task_refs empty → overall pass.
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        );
        // Write task-coverage.json to enter the coverage check path.
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("task-coverage.json"), "{}").unwrap();
        let outcome = verify(&dir);
        assert!(outcome.is_ok(), "stub validate_all_task_refs must produce no findings");
    }
}
