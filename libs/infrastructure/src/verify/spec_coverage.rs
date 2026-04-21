//! Verify requirement-to-task coverage for a specific track.
//!
//! CI gate: every in_scope and acceptance_criteria requirement must have
//! at least one valid task_ref in `task-coverage.json`, and all task_refs
//! must reference valid tasks in `impl-plan.json`.
//!
//! Track resolution is the caller's responsibility (CLI resolves from branch
//! name or explicit `--track-id`). This module does not guess "latest track".
//!
//! T007: coverage is now read from `task-coverage.json` (not spec.json
//! task_refs, which were removed in T003). Valid task IDs are loaded from
//! `impl-plan.json` (replacing the old metadata.json path from T005+).

use std::collections::HashSet;
use std::path::Path;

use domain::TaskId;
use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::spec::codec as spec_codec;

/// Run spec coverage verification for the given track directory.
///
/// The `track_dir` must point to a directory containing `spec.json`,
/// `task-coverage.json`, and optionally `impl-plan.json`
/// (e.g., `track/items/<id>`).
///
/// Skips with pass if the track has no `spec.json`.
/// Skips with pass if the track has no `task-coverage.json` (pre-T004 tracks).
///
/// When both exist, checks:
/// - Coverage enforcement: all `in_scope` and `acceptance_criteria` requirement
///   IDs from spec.json must appear as keys in the corresponding
///   `task-coverage.json` sections with at least one task_ref.
/// - Referential integrity (all 4 sections): any task_ref present in any of
///   the four `task-coverage.json` sections (`in_scope`, `acceptance_criteria`,
///   `out_of_scope`, `constraints`) must reference a valid task in
///   `impl-plan.json`, including stale/extra entries not in spec.json.
///   Skipped if `impl-plan.json` absent; error if it exists but cannot
///   be read or decoded — fail-closed; always enforced when present, even if
///   the decoded task list is empty.
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

    // Skip coverage enforcement until task-coverage.json exists (pre-T004 tracks).
    let task_coverage_path = track_dir.join("task-coverage.json");
    if !task_coverage_path.is_file() {
        return VerifyOutcome::pass();
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

    // Load task-coverage.json
    let task_coverage_content = match std::fs::read_to_string(&task_coverage_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read task-coverage.json: {e}"
            ))]);
        }
    };
    let task_coverage_doc = match crate::task_coverage_codec::decode(&task_coverage_content) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot parse task-coverage.json: {e}"
            ))]);
        }
    };

    // T007: load valid_task_ids from ImplPlanDocument (replaces metadata.json path).
    // impl-plan.json is optional (planning-only tracks may not have it yet).
    //
    // None  → absent: skip referential integrity entirely (no valid-id set available).
    // Some(Ok(ids)) → present and decoded: always run referential integrity, even if
    //                  `ids` is empty (empty plan → every task_ref is invalid).
    // Some(Err(e))  → present but unreadable/malformed: fail closed.
    let impl_plan_ids: Option<HashSet<TaskId>> = match load_impl_plan_task_ids(track_dir) {
        None => None,
        Some(Ok(ids)) => Some(ids),
        Some(Err(e)) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot load impl-plan.json for referential-integrity check: {e}"
            ))]);
        }
    };

    let mut findings: Vec<VerifyFinding> = Vec::new();

    // Coverage enforcement: every in_scope requirement from spec must have
    // at least one task_ref in task-coverage.json's in_scope section.
    for req in spec_doc.scope().in_scope() {
        let covered =
            task_coverage_doc.in_scope().get(req.id()).is_some_and(|refs| !refs.is_empty());
        if !covered {
            findings.push(VerifyFinding::error(format!(
                "Requirement missing task_refs: \"{}\"",
                req.text()
            )));
        }
    }

    // Coverage enforcement: every acceptance_criteria requirement from spec must
    // have at least one task_ref in task-coverage.json's acceptance_criteria section.
    for req in spec_doc.acceptance_criteria() {
        let covered = task_coverage_doc
            .acceptance_criteria()
            .get(req.id())
            .is_some_and(|refs| !refs.is_empty());
        if !covered {
            findings.push(VerifyFinding::error(format!(
                "Requirement missing task_refs: \"{}\"",
                req.text()
            )));
        }
    }

    // Referential integrity for all four sections: any task_ref present in
    // task-coverage.json (including stale keys not in spec.json) must point to
    // a valid impl-plan task when impl-plan.json is present.
    //
    // Iterating the coverage maps directly (rather than the spec requirements)
    // ensures that stale entries in task-coverage.json are also validated.
    // ADR 2026-04-19-1242 §D1.4: all 4 sections, "記述があれば" (if present).
    if let Some(ref valid_task_ids) = impl_plan_ids {
        for (section_name, section_map) in [
            ("in_scope", task_coverage_doc.in_scope()),
            ("acceptance_criteria", task_coverage_doc.acceptance_criteria()),
            ("out_of_scope", task_coverage_doc.out_of_scope()),
            ("constraints", task_coverage_doc.constraints()),
        ] {
            for (req_id, task_refs) in section_map {
                let id_str = req_id.as_ref();
                for task_ref in task_refs {
                    if !valid_task_ids.contains(task_ref) {
                        findings.push(VerifyFinding::error(format!(
                            "task_ref \"{task_ref}\" in {section_name}[\"{id_str}\"] does not exist in impl-plan.json"
                        )));
                    }
                }
            }
        }
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

/// Load task IDs from `impl-plan.json` in the track directory.
///
/// Returns `None` when `impl-plan.json` is absent (planning-only tracks).
/// The caller skips referential integrity entirely in this case.
///
/// Returns `Some(Ok(ids))` when the file is present and decoded successfully.
/// The caller must validate all task_refs against the returned set (even if
/// it is empty — an empty plan means every task_ref is invalid).
///
/// Returns `Some(Err(message))` when the file is present but cannot be read
/// or decoded. The caller must surface this as an error (fail-closed).
fn load_impl_plan_task_ids(track_dir: &Path) -> Option<Result<HashSet<TaskId>, String>> {
    let path = track_dir.join("impl-plan.json");
    if !path.is_file() {
        return None;
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return Some(Err(format!("cannot read {}: {e}", path.display()))),
    };
    let doc = match crate::impl_plan_codec::decode(&content) {
        Ok(d) => d,
        Err(e) => return Some(Err(format!("cannot decode {}: {e}", path.display()))),
    };
    Some(Ok(doc.tasks().iter().map(|t| t.id().clone()).collect()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    const TRACK_ITEMS_DIR: &str = "track/items";

    /// Write a minimal v4 identity-only metadata.json (T005: no tasks/plan).
    fn setup_track(root: &Path, name: &str, spec_json: &str) {
        let dir = root.join(TRACK_ITEMS_DIR).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let meta = format!(
            r#"{{
  "schema_version": 4,
  "id": "{name}",
  "title": "Test",
  "status": "in_progress",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "branch": "track/{name}"
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

    /// Minimal impl-plan.json with one T001 task (task is referenced in a section).
    fn impl_plan_with_t001() -> &'static str {
        r#"{
  "schema_version": 1,
  "tasks": [{"id": "T001", "description": "task", "status": "todo"}],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T001"]}
    ]
  }
}"#
    }

    /// task-coverage.json with IN-01 → [T001].
    fn task_coverage_in01_t001() -> &'static str {
        r#"{
  "schema_version": 1,
  "in_scope": {"IN-01": ["T001"]},
  "out_of_scope": {},
  "constraints": {},
  "acceptance_criteria": {}
}"#
    }

    /// task-coverage.json with empty sections.
    fn task_coverage_empty() -> &'static str {
        r#"{
  "schema_version": 1,
  "in_scope": {},
  "out_of_scope": {},
  "constraints": {},
  "acceptance_criteria": {}
}"#
    }

    #[test]
    fn test_no_spec_json_passes() {
        let tmp = TempDir::new().unwrap();
        setup_track(tmp.path(), "active-track", "");
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
    fn test_spec_without_scope_items_passes() {
        // No in_scope or acceptance_criteria → nothing to enforce → pass.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        );
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("task-coverage.json"), task_coverage_empty()).unwrap();
        let outcome = verify(&dir);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_spec_without_task_coverage_json_skips_coverage_check() {
        // When task-coverage.json is absent, coverage check is skipped.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        // No task-coverage.json written → guard skips coverage check → pass.
        let outcome = verify(&track_dir(tmp.path(), "active-track"));
        assert!(outcome.is_ok(), "must skip coverage when task-coverage.json absent");
    }

    #[test]
    fn test_spec_with_in_scope_item_not_in_task_coverage_reports_uncovered() {
        // IN-01 exists in spec but not in task-coverage.json → uncovered → error.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("task-coverage.json"), task_coverage_empty()).unwrap();
        let outcome = verify(&dir);
        assert!(outcome.has_errors(), "IN-01 not in task-coverage → must report error");
    }

    #[test]
    fn test_spec_with_in_scope_item_covered_by_task_passes() {
        // IN-01 has task_refs [T001] in task-coverage.json + impl-plan has T001 → pass.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("impl-plan.json"), impl_plan_with_t001()).unwrap();
        std::fs::write(dir.join("task-coverage.json"), task_coverage_in01_t001()).unwrap();
        let outcome = verify(&dir);
        assert!(outcome.is_ok(), "{:?}", outcome);
    }

    #[test]
    fn test_task_ref_pointing_to_nonexistent_task_reports_error() {
        // IN-01 → [T999] in task-coverage but T999 not in impl-plan → referential error.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("impl-plan.json"), impl_plan_with_t001()).unwrap();
        let bad_coverage = r#"{
  "schema_version": 1,
  "in_scope": {"IN-01": ["T999"]},
  "out_of_scope": {},
  "constraints": {},
  "acceptance_criteria": {}
}"#;
        std::fs::write(dir.join("task-coverage.json"), bad_coverage).unwrap();
        let outcome = verify(&dir);
        assert!(outcome.has_errors(), "T999 not in impl-plan → referential error");
        assert!(
            outcome.findings()[0].message().contains("T999"),
            "error must mention T999: {}",
            outcome.findings()[0].message()
        );
    }

    #[test]
    fn test_constraint_without_coverage_passes() {
        // constraints are NOT enforced; only in_scope + AC.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"constraints":[{"id":"CN-01","text":"constraint","convention_refs":[{"file":"conv/x.md","anchor":"s1"}]}]}"#,
        );
        let dir = track_dir(tmp.path(), "active-track");
        std::fs::write(dir.join("task-coverage.json"), task_coverage_empty()).unwrap();
        let outcome = verify(&dir);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_coverage_without_impl_plan_skips_referential_integrity() {
        // When impl-plan.json absent, task_refs in task-coverage are not validated.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        let dir = track_dir(tmp.path(), "active-track");
        // No impl-plan.json → referential integrity skipped.
        let coverage_with_fake_task = r#"{
  "schema_version": 1,
  "in_scope": {"IN-01": ["T999"]},
  "out_of_scope": {},
  "constraints": {},
  "acceptance_criteria": {}
}"#;
        std::fs::write(dir.join("task-coverage.json"), coverage_with_fake_task).unwrap();
        let outcome = verify(&dir);
        // IN-01 is covered (non-empty task_refs) and no referential integrity check → pass.
        assert!(
            outcome.is_ok(),
            "must skip referential integrity when impl-plan absent: {:?}",
            outcome
        );
    }

    #[test]
    fn test_malformed_impl_plan_fails_closed() {
        // When impl-plan.json is present but contains invalid JSON, verify must
        // fail with an error rather than silently skipping referential integrity.
        let tmp = TempDir::new().unwrap();
        setup_track(
            tmp.path(),
            "active-track",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"item","adr_refs":[{"file":"adr/x.md","anchor":"D1"}]}],"out_of_scope":[]}}"#,
        );
        let dir = track_dir(tmp.path(), "active-track");
        // Write an invalid impl-plan.json (not valid JSON).
        std::fs::write(dir.join("impl-plan.json"), "not valid json").unwrap();
        let coverage = r#"{
  "schema_version": 1,
  "in_scope": {"IN-01": ["T999"]},
  "out_of_scope": {},
  "constraints": {},
  "acceptance_criteria": {}
}"#;
        std::fs::write(dir.join("task-coverage.json"), coverage).unwrap();
        let outcome = verify(&dir);
        // Malformed impl-plan.json must fail closed — not silently skip.
        assert!(
            outcome.has_errors(),
            "malformed impl-plan.json must fail closed, got: {:?}",
            outcome
        );
    }
}
