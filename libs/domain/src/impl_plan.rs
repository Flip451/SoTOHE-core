//! Domain aggregate root for `impl-plan.json` (Phase 3 SSoT).
//!
//! `ImplPlanDocument` holds the implementation tasks and plan view for a track.
//! It replaces the `tasks` / `plan` fields that were previously embedded in
//! `TrackMetadata`. Introduced by ADR 2026-04-19-1242 §D1.4.

use crate::track::validate_plan_invariants;
use crate::{DomainError, PlanView, TrackTask};

/// The current schema version for `impl-plan.json`.
pub const IMPL_PLAN_SCHEMA_VERSION: u32 = 1;

/// Aggregate root for `track/items/<id>/impl-plan.json`.
///
/// Holds the ordered task list and plan view for a track's Phase 3 implementation plan.
/// Reuses `TrackTask` and `PlanView` / `PlanSection` from the domain.
///
/// Invariants enforced on construction:
/// - Every `TaskId` in `tasks` appears exactly once in `plan` (no unreferenced tasks).
/// - No duplicate `TaskId` values in `tasks`.
/// - No task referenced in `plan` that does not exist in `tasks`.
/// - No duplicate section IDs in `plan`.
/// - No task referenced more than once across all sections.
///
/// These are the same plan-task consistency invariants enforced by `TrackMetadata::new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplPlanDocument {
    schema_version: u32,
    tasks: Vec<TrackTask>,
    plan: PlanView,
}

impl ImplPlanDocument {
    /// Creates a new `ImplPlanDocument`, validating plan-task referential integrity.
    ///
    /// # Errors
    ///
    /// Returns `DomainError` on duplicate task IDs, duplicate section IDs,
    /// unreferenced tasks, unknown task references, or duplicate task references in the plan.
    pub fn new(tasks: Vec<TrackTask>, plan: PlanView) -> Result<Self, DomainError> {
        validate_plan_invariants(&tasks, &plan)?;
        Ok(Self { schema_version: IMPL_PLAN_SCHEMA_VERSION, tasks, plan })
    }

    /// Returns the schema version.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the task list.
    #[must_use]
    pub fn tasks(&self) -> &[TrackTask] {
        &self.tasks
    }

    /// Returns the plan view.
    #[must_use]
    pub fn plan(&self) -> &PlanView {
        &self.plan
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use crate::{
        DomainError, PlanSection, PlanView, TaskId, TrackTask, ValidationError,
        impl_plan::ImplPlanDocument,
    };

    fn task(id: &str, desc: &str) -> TrackTask {
        TrackTask::new(TaskId::try_new(id).unwrap(), desc).unwrap()
    }

    fn section(id: &str, title: &str, task_ids: &[&str]) -> PlanSection {
        PlanSection::new(
            id,
            title,
            vec![],
            task_ids.iter().map(|t| TaskId::try_new(*t).unwrap()).collect(),
        )
        .unwrap()
    }

    fn plan(task_ids: &[&str]) -> PlanView {
        PlanView::new(vec![], vec![section("S1", "Impl", task_ids)])
    }

    // --- happy path ---

    #[test]
    fn test_new_with_valid_tasks_and_plan_succeeds() {
        let tasks = vec![task("T001", "First"), task("T002", "Second")];
        let p = plan(&["T001", "T002"]);
        let doc = ImplPlanDocument::new(tasks, p).unwrap();
        assert_eq!(doc.schema_version(), 1);
        assert_eq!(doc.tasks().len(), 2);
        assert_eq!(doc.plan().sections().len(), 1);
    }

    #[test]
    fn test_new_with_empty_tasks_and_empty_plan_succeeds() {
        let doc = ImplPlanDocument::new(vec![], PlanView::new(vec![], vec![])).unwrap();
        assert_eq!(doc.tasks().len(), 0);
        assert_eq!(doc.plan().sections().len(), 0);
    }

    // --- validation: duplicate task ID ---

    #[test]
    fn test_new_with_duplicate_task_id_returns_error() {
        let tasks = vec![task("T001", "First"), task("T001", "Duplicate")];
        let p = plan(&["T001"]);
        let err = ImplPlanDocument::new(tasks, p).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::DuplicateTaskId(ref id)) if id == "T001"
            ),
            "unexpected error: {err:?}"
        );
    }

    // --- validation: unreferenced task ---

    #[test]
    fn test_new_with_unreferenced_task_returns_error() {
        let tasks = vec![task("T001", "First"), task("T002", "Unreferenced")];
        let p = plan(&["T001"]); // T002 not in plan
        let err = ImplPlanDocument::new(tasks, p).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::UnreferencedTask(ref id)) if id == "T002"
            ),
            "unexpected error: {err:?}"
        );
    }

    // --- validation: unknown task reference ---

    #[test]
    fn test_new_with_unknown_task_reference_returns_error() {
        let tasks = vec![task("T001", "Only task")];
        // plan references T999 which does not exist
        let s = PlanSection::new(
            "S1",
            "Section",
            vec![],
            vec![
                TaskId::try_new("T001").unwrap(),
                TaskId::try_new("T999").unwrap(), // unknown
            ],
        )
        .unwrap();
        let p = PlanView::new(vec![], vec![s]);
        let err = ImplPlanDocument::new(tasks, p).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::UnknownTaskReference(ref id)) if id == "T999"
            ),
            "unexpected error: {err:?}"
        );
    }

    // --- validation: duplicate task reference in plan ---

    #[test]
    fn test_new_with_duplicate_task_reference_returns_error() {
        let tasks = vec![task("T001", "Task one")];
        // T001 referenced twice in the same section
        let s = PlanSection::new(
            "S1",
            "Section",
            vec![],
            vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T001").unwrap()],
        )
        .unwrap();
        let p = PlanView::new(vec![], vec![s]);
        let err = ImplPlanDocument::new(tasks, p).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::DuplicateTaskReference(ref id)) if id == "T001"
            ),
            "unexpected error: {err:?}"
        );
    }

    // --- validation: duplicate section ID ---

    #[test]
    fn test_new_with_duplicate_section_id_returns_error() {
        let tasks = vec![task("T001", "Task one"), task("T002", "Task two")];
        let s1 = section("S1", "Section One", &["T001"]);
        let s2 = section("S1", "Duplicate Section ID", &["T002"]);
        let p = PlanView::new(vec![], vec![s1, s2]);
        let err = ImplPlanDocument::new(tasks, p).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::DuplicatePlanSectionId(ref id)) if id == "S1"
            ),
            "unexpected error: {err:?}"
        );
    }

    // --- accessors ---

    #[test]
    fn test_schema_version_is_1() {
        let doc = ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        assert_eq!(doc.schema_version(), 1);
    }

    #[test]
    fn test_tasks_accessor_returns_correct_count() {
        let tasks = vec![task("T001", "A"), task("T002", "B"), task("T003", "C")];
        let p = plan(&["T001", "T002", "T003"]);
        let doc = ImplPlanDocument::new(tasks, p).unwrap();
        assert_eq!(doc.tasks().len(), 3);
        assert_eq!(doc.tasks()[0].id().as_ref(), "T001");
    }

    #[test]
    fn test_plan_accessor_returns_sections() {
        let tasks = vec![task("T001", "task")];
        let p = plan(&["T001"]);
        let doc = ImplPlanDocument::new(tasks, p).unwrap();
        assert_eq!(doc.plan().sections().len(), 1);
        assert_eq!(doc.plan().sections()[0].id(), "S1");
    }
}
