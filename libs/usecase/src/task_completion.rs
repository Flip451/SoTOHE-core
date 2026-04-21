//! Task-completion gate orchestration (hexagonal usecase layer).
//!
//! This module implements the pre-merge task-completion check that ensures
//! all declared tasks in `impl-plan.json` are `done` or `skipped` before the
//! merge proceeds. It reads from the same [`TrackBlobReader`] port used by the
//! strict merge gate.
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D9, §D9.1.

use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{TrackId, validate_branch_ref};

use crate::merge_gate::{BlobFetchResult, TrackBlobReader};

/// Checks that all tasks in the track's `impl-plan.json` are resolved
/// (either `done`, `done-traced`, `done-pending`, or `skipped`) before the
/// merge proceeds.
///
/// # Behavior
///
/// 1. `plan/` branches → PASS (plan-only branches carry no implementation tasks)
/// 2. `validate_branch_ref` → fail-closed on dangerous characters
/// 3. `track/` prefix stripped and validated via `TrackId::try_new`
/// 4. `reader.read_impl_plan(branch, track_id)`:
///    - `Found(doc)` → check `all_tasks_resolved()`; report unresolved task IDs
///    - `NotFound` → BLOCKED (activated `track/*` branches must carry impl-plan.json;
///      `plan/*` branches already short-circuited at step 1, so a missing impl-plan
///      here is an activated track with no task list — a merge bypass path)
///    - `FetchError` → BLOCKED
///
/// This function is a thin orchestration that delegates all I/O to the
/// [`TrackBlobReader`] port. Tests use `MockReader` to exercise every branch
/// without a real git repository.
///
/// Reference: ADR §D9.
#[must_use]
pub fn check_tasks_resolved_from_git_ref(
    branch: &str,
    reader: &impl TrackBlobReader,
) -> VerifyOutcome {
    // 1. plan/ branches skip the gate entirely
    if branch.starts_with("plan/") {
        return VerifyOutcome::pass();
    }

    // 2. Branch-ref validation
    if let Err(err) = validate_branch_ref(branch) {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "invalid branch ref: {err}"
        ))]);
    }

    // 3. Resolve and validate track_id
    let track_id_str = branch.strip_prefix("track/").unwrap_or(branch);
    if branch.starts_with("track/") {
        if let Err(err) = TrackId::try_new(track_id_str) {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "invalid track id derived from branch '{branch}': {err}"
            ))]);
        }
    }

    // 4. Fetch and inspect impl-plan.json.
    //
    // `plan/*` branches short-circuit at step 1 (planning-only, no tasks yet).
    // `track/*` branches reach this point only after activation, at which
    // point impl-plan.json is required — it is the SSoT for the task list
    // that this gate must enforce. Treating a missing impl-plan.json as
    // `pass()` would silently bypass the "all tasks resolved" check, letting
    // a merge proceed on an activated track that never produced its task
    // plan (or whose plan was deleted). Fail closed instead.
    let impl_plan = match reader.read_impl_plan(branch, track_id_str) {
        BlobFetchResult::Found(doc) => doc,
        BlobFetchResult::NotFound => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "track '{track_id_str}' missing impl-plan.json on origin/{branch}: \
                 activated tracks must commit impl-plan.json before merge"
            ))]);
        }
        BlobFetchResult::FetchError(msg) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to read impl-plan.json on origin/{branch}: {msg}"
            ))]);
        }
    };

    // 5. All tasks must be resolved.
    if impl_plan.all_tasks_resolved() {
        return VerifyOutcome::pass();
    }

    let unresolved: Vec<String> =
        impl_plan.unresolved_task_ids().iter().map(|id| id.to_string()).collect();
    VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
        "track '{track_id_str}' has unresolved tasks on origin/{branch}: {}",
        unresolved.join(", ")
    ))])
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::cell::RefCell;

    use domain::spec::SpecDocument;
    use domain::{
        ImplPlanDocument, PlanSection, PlanView, TaskId, TaskStatus, TrackTask,
        TypeCatalogueDocument,
    };

    use super::*;

    /// Mock reader dedicated to task_completion tests.
    struct MockReader {
        impl_plan: RefCell<Option<BlobFetchResult<ImplPlanDocument>>>,
    }

    impl MockReader {
        fn new(impl_plan: BlobFetchResult<ImplPlanDocument>) -> Self {
            Self { impl_plan: RefCell::new(Some(impl_plan)) }
        }

        fn unreachable() -> Self {
            Self { impl_plan: RefCell::new(None) }
        }
    }

    impl TrackBlobReader for MockReader {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            panic!("read_spec_document must not be called by task_completion tests")
        }

        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(TypeCatalogueDocument, String)> {
            panic!("read_type_catalogue must not be called by task_completion tests")
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<ImplPlanDocument> {
            self.impl_plan.borrow_mut().take().expect(
                "read_impl_plan called twice or not configured (use ::unreachable() for short-circuit tests)",
            )
        }
    }

    // --- Helpers ---

    fn task_with_status(id: &str, desc: &str, status: TaskStatus) -> TrackTask {
        TrackTask::with_status(TaskId::try_new(id).unwrap(), desc, status).unwrap()
    }

    fn section_with_ids(id: &str, task_ids: &[&str]) -> PlanSection {
        PlanSection::new(
            id,
            "Section",
            vec![],
            task_ids.iter().map(|t| TaskId::try_new(*t).unwrap()).collect(),
        )
        .unwrap()
    }

    fn impl_plan_all_resolved() -> ImplPlanDocument {
        let t = task_with_status("T001", "done task", TaskStatus::DonePending);
        let s = section_with_ids("S1", &["T001"]);
        ImplPlanDocument::new(vec![t], PlanView::new(vec![], vec![s])).unwrap()
    }

    fn impl_plan_with_unresolved() -> ImplPlanDocument {
        let t1 = task_with_status("T001", "done task", TaskStatus::DonePending);
        let t2 = task_with_status("T002", "open task", TaskStatus::Todo);
        let s = section_with_ids("S1", &["T001", "T002"]);
        ImplPlanDocument::new(vec![t1, t2], PlanView::new(vec![], vec![s])).unwrap()
    }

    fn impl_plan_empty() -> ImplPlanDocument {
        ImplPlanDocument::new(vec![], PlanView::new(vec![], vec![])).unwrap()
    }

    // --- K1–K7 test matrix ---

    #[test]
    fn test_k1_plan_branch_short_circuits() {
        // K1: plan/ branch → PASS (reader never called)
        let reader = MockReader::unreachable();
        let outcome = check_tasks_resolved_from_git_ref("plan/dummy", &reader);
        assert!(!outcome.has_errors());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_k2_impl_plan_all_resolved_passes() {
        // K2: Found(all resolved) → PASS
        let reader = MockReader::new(BlobFetchResult::Found(impl_plan_all_resolved()));
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(!outcome.has_errors(), "{outcome:?}");
    }

    #[test]
    fn test_k3_impl_plan_with_unresolved_tasks_blocks() {
        // K3: Found(has unresolved tasks) → BLOCKED, mentions task IDs
        let reader = MockReader::new(BlobFetchResult::Found(impl_plan_with_unresolved()));
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(
            outcome.findings()[0].message().contains("T002"),
            "finding must mention unresolved task id: {}",
            outcome.findings()[0].message()
        );
    }

    #[test]
    fn test_k4_impl_plan_not_found_blocks_on_track_branch() {
        // K4: impl-plan.json NotFound on an activated `track/*` branch → BLOCKED.
        // planning-only `plan/*` branches short-circuit at step 1 before ever
        // reaching the reader; by the time `read_impl_plan` runs we are on
        // an activated track, so a missing impl-plan.json is a merge bypass.
        let reader = MockReader::new(BlobFetchResult::NotFound);
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(outcome.has_errors(), "{outcome:?}");
        assert!(
            outcome.findings()[0].message().contains("missing impl-plan.json"),
            "finding must mention missing impl-plan.json: {}",
            outcome.findings()[0].message()
        );
    }

    #[test]
    fn test_k5_fetch_error_blocks() {
        // K5: FetchError → BLOCKED
        let reader = MockReader::new(BlobFetchResult::FetchError("git show failed".to_owned()));
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings()[0].message().contains("git show failed"));
    }

    #[test]
    fn test_k6_dangerous_branch_chars_block() {
        // K6: branch contains .. → BLOCKED via validate_branch_ref, reader not called
        let reader = MockReader::unreachable();
        let outcome = check_tasks_resolved_from_git_ref("track/foo..bar", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings()[0].message().contains("invalid branch ref"));
    }

    #[test]
    fn test_k7_empty_branch_blocks() {
        // K7: empty branch name → BLOCKED, reader not called
        let reader = MockReader::unreachable();
        let outcome = check_tasks_resolved_from_git_ref("", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_track_id_validation_rejects_empty_suffix() {
        // Extra: track/ with empty suffix → BLOCKED via TrackId::try_new
        let reader = MockReader::unreachable();
        let outcome = check_tasks_resolved_from_git_ref("track/", &reader);
        assert!(outcome.has_errors());
        assert!(
            outcome.findings()[0].message().contains("invalid track id")
                || outcome.findings()[0].message().contains("invalid branch ref")
        );
    }

    #[test]
    fn test_empty_impl_plan_passes() {
        // Extra: impl-plan.json with zero tasks → all_tasks_resolved() = true → PASS
        let reader = MockReader::new(BlobFetchResult::Found(impl_plan_empty()));
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(!outcome.has_errors(), "{outcome:?}");
    }

    #[test]
    fn test_unresolved_tasks_error_mentions_track_id() {
        // Extra: error message must include track_id for diagnostics
        let reader = MockReader::new(BlobFetchResult::Found(impl_plan_with_unresolved()));
        let outcome = check_tasks_resolved_from_git_ref("track/my-feature", &reader);
        assert!(outcome.has_errors());
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("my-feature"), "error must mention track id: {msg}");
    }
}
