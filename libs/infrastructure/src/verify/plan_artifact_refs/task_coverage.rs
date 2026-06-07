//! Task-coverage enforcement and referential integrity checks.
//!
//! Validates `task-coverage.json` (T011):
//! - Coverage: every `in_scope` and `acceptance_criteria` requirement must have task_refs.
//! - Referential integrity (spec elements): keys in task-coverage must resolve to spec elements.
//! - Referential integrity (task ids): task_refs must exist in `impl-plan.json`.

use std::collections::HashSet;
use std::path::Path;

use domain::verify::VerifyFinding;

use crate::track::symlink_guard;

/// Run coverage enforcement + referential integrity checks for `task-coverage.json`.
///
/// Called from `verify()` when `task-coverage.json` is confirmed present and
/// passes the symlink guard.
///
/// Checks:
/// 1. Coverage: every `in_scope` and `acceptance_criteria` requirement in `spec.json`
///    must have at least one task_ref entry in `task-coverage.json`.
/// 2. Referential integrity (spec elements): every SpecElementId key present in any
///    section of `task-coverage.json` must resolve to an element in `spec.json`.
/// 3. Referential integrity (task ids): every TaskId value in any section must exist
///    in `impl-plan.json`. If `impl-plan.json` is absent, this check fails closed:
///    an error finding is emitted because dangling task references would otherwise
///    pass silently.
pub(crate) fn verify_task_coverage(
    track_dir: &Path,
    task_coverage_path: &Path,
    spec_doc: &domain::SpecDocument,
    trusted_root: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    // Load task-coverage.json
    let task_coverage_content = match std::fs::read_to_string(task_coverage_path) {
        Ok(c) => c,
        Err(e) => {
            findings.push(VerifyFinding::error(format!("Cannot read task-coverage.json: {e}")));
            return;
        }
    };
    let task_coverage_doc = match crate::task_coverage_codec::decode(&task_coverage_content) {
        Ok(d) => d,
        Err(e) => {
            findings.push(VerifyFinding::error(format!("Cannot parse task-coverage.json: {e}")));
            return;
        }
    };

    // -----------------------------------------------------------------------
    // 5a. Coverage enforcement: in_scope requirements
    // -----------------------------------------------------------------------
    for req in spec_doc.scope().in_scope() {
        let covered =
            task_coverage_doc.in_scope().get(req.id()).is_some_and(|refs| !refs.is_empty());
        if !covered {
            findings.push(VerifyFinding::error(format!(
                "coverage violation: in_scope requirement \"{}\" (id: {}) has no task_refs in task-coverage.json",
                req.text(),
                req.id()
            )));
        }
    }

    // -----------------------------------------------------------------------
    // 5b. Coverage enforcement: acceptance_criteria requirements
    // -----------------------------------------------------------------------
    for req in spec_doc.acceptance_criteria() {
        let covered = task_coverage_doc
            .acceptance_criteria()
            .get(req.id())
            .is_some_and(|refs| !refs.is_empty());
        if !covered {
            findings.push(VerifyFinding::error(format!(
                "coverage violation: acceptance_criteria requirement \"{}\" (id: {}) has no task_refs in task-coverage.json",
                req.text(),
                req.id()
            )));
        }
    }

    // -----------------------------------------------------------------------
    // 5c. Referential integrity: spec element ids
    //
    // Each task-coverage section is validated against its matching spec section.
    // `goal` is intentionally excluded because task-coverage.json has no `goal`
    // section — goal requirements are not tracked at the task level.
    // Cross-section mappings (e.g., a constraints ID in the in_scope map) are
    // flagged as referential integrity errors.
    // -----------------------------------------------------------------------
    let in_scope_ids: HashSet<String> =
        spec_doc.scope().in_scope().iter().map(|r| r.id().as_ref().to_owned()).collect();
    let out_of_scope_ids: HashSet<String> =
        spec_doc.scope().out_of_scope().iter().map(|r| r.id().as_ref().to_owned()).collect();
    let constraints_ids: HashSet<String> =
        spec_doc.constraints().iter().map(|r| r.id().as_ref().to_owned()).collect();
    let acceptance_criteria_ids: HashSet<String> =
        spec_doc.acceptance_criteria().iter().map(|r| r.id().as_ref().to_owned()).collect();

    // Validate referential integrity for each section against its matching spec section.
    check_section_integrity("in_scope", task_coverage_doc.in_scope(), &in_scope_ids, findings);
    check_section_integrity(
        "acceptance_criteria",
        task_coverage_doc.acceptance_criteria(),
        &acceptance_criteria_ids,
        findings,
    );
    check_section_integrity(
        "out_of_scope",
        task_coverage_doc.out_of_scope(),
        &out_of_scope_ids,
        findings,
    );
    check_section_integrity(
        "constraints",
        task_coverage_doc.constraints(),
        &constraints_ids,
        findings,
    );

    // -----------------------------------------------------------------------
    // 5d. Referential integrity: task ids against impl-plan.json
    //
    // `task-coverage.json` is present at this point (caller pre-filtered).
    // If `impl-plan.json` is missing, fail closed: the task_ref entries in
    // task-coverage.json have no authoritative task-id source to validate
    // against, so accepting the track would let dangling task references
    // pass silently (e.g. after an accidental delete or partial commit).
    // If present but unreadable/malformed, fail closed.
    // -----------------------------------------------------------------------
    let impl_plan_path = track_dir.join("impl-plan.json");
    match symlink_guard::reject_symlinks_below(&impl_plan_path, trusted_root) {
        Ok(false) => {
            findings.push(VerifyFinding::error(format!(
                "task-coverage.json is present but impl-plan.json is missing at {}; \
                 cannot validate task_ref integrity fail-closed",
                impl_plan_path.display()
            )));
        }
        Ok(true) => match load_impl_plan_task_ids_from_path(&impl_plan_path) {
            Ok(valid_task_ids) => {
                check_task_id_integrity(&task_coverage_doc, &valid_task_ids, findings);
            }
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "Cannot load impl-plan.json for referential-integrity check: {e}"
                )));
            }
        },
        Err(e) => {
            findings.push(VerifyFinding::error(format!("impl-plan.json symlink guard: {e}")));
        }
    }
}

/// Validate that every SpecElementId key in a single task-coverage section matches
/// an element in the corresponding spec section.
///
/// Emits error findings for any key not found in `valid_ids`.
fn check_section_integrity(
    section_name: &str,
    section_map: &std::collections::BTreeMap<domain::SpecElementId, Vec<domain::TaskId>>,
    valid_ids: &HashSet<String>,
    findings: &mut Vec<VerifyFinding>,
) {
    for req_id in section_map.keys() {
        let id_str = req_id.as_ref();
        if !valid_ids.contains(id_str) {
            findings.push(VerifyFinding::error(format!(
                "coverage violation: task-coverage.json \
                 {section_name}[\"{id_str}\"] references an element id that does not exist \
                 in spec.json {section_name} section"
            )));
        }
    }
}

/// Validate that every TaskId in all four task-coverage sections exists in `valid_task_ids`.
///
/// Emits error findings for any task_ref not found in the impl-plan task set.
fn check_task_id_integrity(
    task_coverage_doc: &domain::TaskCoverageDocument,
    valid_task_ids: &HashSet<domain::TaskId>,
    findings: &mut Vec<VerifyFinding>,
) {
    let sections: [(&str, &std::collections::BTreeMap<domain::SpecElementId, Vec<domain::TaskId>>);
        4] = [
        ("in_scope", task_coverage_doc.in_scope()),
        ("acceptance_criteria", task_coverage_doc.acceptance_criteria()),
        ("out_of_scope", task_coverage_doc.out_of_scope()),
        ("constraints", task_coverage_doc.constraints()),
    ];
    for (section_name, section_map) in &sections {
        for (req_id, task_refs) in *section_map {
            let id_str = req_id.as_ref();
            for task_ref in task_refs {
                if !valid_task_ids.contains(task_ref) {
                    findings.push(VerifyFinding::error(format!(
                        "coverage violation: task_ref \"{task_ref}\" in \
                         {section_name}[\"{id_str}\"] does not exist in impl-plan.json"
                    )));
                }
            }
        }
    }
}

/// Load task IDs from `impl-plan.json` at the given path.
///
/// Returns `Ok(ids)` when decoded successfully (may be empty — an empty plan
/// means every task_ref is invalid).
/// Returns `Err(message)` when the file is unreadable or malformed.
fn load_impl_plan_task_ids_from_path(path: &Path) -> Result<HashSet<domain::TaskId>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let doc = crate::impl_plan_codec::decode(&content)
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
    Ok(doc.tasks().iter().map(|t| t.id().clone()).collect())
}
