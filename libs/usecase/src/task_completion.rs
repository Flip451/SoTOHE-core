//! Task-completion gate orchestration (hexagonal usecase layer).
//!
//! This module implements the pre-merge task-completion check that ensures
//! all declared tasks in `metadata.json` are `done` or `skipped` before the
//! merge proceeds. It consolidates the previous CLI-layer `check_tasks_resolved`
//! logic from `apps/cli/src/commands/pr.rs` into a pure usecase workflow that
//! goes through the same [`TrackBlobReader`] port used by the strict merge gate.
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D9, §D9.1.

use domain::verify::{Finding, VerifyOutcome};
use domain::{TaskStatus, TrackId, validate_branch_ref};

use crate::merge_gate::{BlobFetchResult, TrackBlobReader};

/// Checks that all tasks in the track's `metadata.json` are resolved
/// (either `done`, `done-traced`, `done-pending`, or `skipped`) before the
/// merge proceeds.
///
/// # Behavior
///
/// 1. `plan/` branches → PASS (plan-only branches carry no implementation tasks)
/// 2. `validate_branch_ref` → fail-closed on dangerous characters
/// 3. `track/` prefix stripped and validated via `TrackId::try_new`
/// 4. `reader.read_track_metadata(branch, track_id)`:
///    - `Found(track)` → check `all_tasks_resolved()`; report unresolved task IDs
///    - `NotFound` → BLOCKED (metadata.json is required for every track)
///    - `FetchError` → BLOCKED
///
/// This function is a thin orchestration that delegates all I/O to the
/// [`TrackBlobReader`] port. Tests use `MockTrackBlobReader` to exercise
/// every branch without a real git repository.
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
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "invalid branch ref: {err}"
        ))]);
    }

    // 3. Resolve and validate track_id
    let track_id_str = branch.strip_prefix("track/").unwrap_or(branch);
    if branch.starts_with("track/") {
        if let Err(err) = TrackId::try_new(track_id_str) {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "invalid track id derived from branch '{branch}': {err}"
            ))]);
        }
    }

    // 4. Fetch and inspect metadata
    let track = match reader.read_track_metadata(branch, track_id_str) {
        BlobFetchResult::Found(t) => t,
        BlobFetchResult::NotFound => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "metadata.json not found on origin/{branch} — every track must have a metadata.json"
            ))]);
        }
        BlobFetchResult::FetchError(msg) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "failed to read metadata.json on origin/{branch}: {msg}"
            ))]);
        }
    };

    if !track.all_tasks_resolved() {
        let unresolved: Vec<String> = track
            .tasks()
            .iter()
            .filter(|t| {
                !matches!(
                    t.status(),
                    TaskStatus::DonePending | TaskStatus::DoneTraced { .. } | TaskStatus::Skipped
                )
            })
            .map(|t| format!("{} ({})", t.id(), t.status().kind()))
            .collect();
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "track has unresolved tasks: {} — run track-transition to mark tasks as done before merging",
            unresolved.join(", ")
        ))]);
    }

    VerifyOutcome::pass()
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::cell::RefCell;

    use domain::spec::SpecDocument;
    use domain::{
        PlanSection, PlanView, TaskId, TaskStatus, TrackMetadata, TrackTask, TypeCatalogueDocument,
    };

    use super::*;

    /// Mock reader dedicated to task_completion tests.
    struct MockReader {
        metadata: RefCell<Option<BlobFetchResult<TrackMetadata>>>,
    }

    impl MockReader {
        fn new(metadata: BlobFetchResult<TrackMetadata>) -> Self {
            Self { metadata: RefCell::new(Some(metadata)) }
        }

        fn unreachable() -> Self {
            Self { metadata: RefCell::new(None) }
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
        ) -> BlobFetchResult<TypeCatalogueDocument> {
            panic!("read_type_catalogue must not be called by task_completion tests")
        }

        fn read_track_metadata(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<TrackMetadata> {
            self.metadata.borrow_mut().take().expect(
                "read_track_metadata called twice or not configured (use ::unreachable() for short-circuit tests)",
            )
        }
    }

    // --- Helpers to construct TrackMetadata aggregates ---

    fn task_with(id: &str, status: TaskStatus) -> TrackTask {
        let task_id = TaskId::try_new(id).unwrap();
        TrackTask::with_status(task_id, "test task", status).unwrap()
    }

    fn track_metadata_with_tasks(id: &str, tasks: Vec<TrackTask>) -> TrackMetadata {
        let section = PlanSection::new(
            "S1",
            "Section 1",
            Vec::new(),
            tasks.iter().map(|t| t.id().clone()).collect(),
        )
        .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        TrackMetadata::new(TrackId::try_new(id).unwrap(), "Test track", tasks, plan, None).unwrap()
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
    fn test_k2_all_tasks_done_passes() {
        // K2: metadata with all done tasks → PASS
        let tasks = vec![
            task_with("T001", TaskStatus::DonePending),
            task_with("T002", TaskStatus::DonePending),
        ];
        let metadata = track_metadata_with_tasks("foo", tasks);
        let reader = MockReader::new(BlobFetchResult::Found(metadata));
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(!outcome.has_errors(), "{outcome:?}");
    }

    #[test]
    fn test_k3_unresolved_task_blocks_with_list() {
        // K3: one task still todo → BLOCKED with unresolved list in finding
        let tasks = vec![
            task_with("T001", TaskStatus::DonePending),
            task_with("T002", TaskStatus::Todo),
            task_with("T003", TaskStatus::InProgress),
        ];
        let metadata = track_metadata_with_tasks("foo", tasks);
        let reader = MockReader::new(BlobFetchResult::Found(metadata));
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(outcome.has_errors());
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("T002"), "unresolved T002 in finding: {msg}");
        assert!(msg.contains("T003"), "unresolved T003 in finding: {msg}");
    }

    #[test]
    fn test_k4_metadata_not_found_blocks() {
        // K4: metadata.json NotFound → BLOCKED
        let reader = MockReader::new(BlobFetchResult::NotFound);
        let outcome = check_tasks_resolved_from_git_ref("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(
            outcome.findings()[0].message().contains("metadata.json"),
            "finding must mention metadata.json: {}",
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
}
