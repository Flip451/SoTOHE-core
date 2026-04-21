//! Domain aggregate root for `impl-plan.json` (Phase 3 SSoT).
//!
//! `ImplPlanDocument` holds the implementation tasks and plan view for a track.
//! It replaces the `tasks` / `plan` fields that were previously embedded in
//! `TrackMetadata`. Introduced by ADR 2026-04-19-1242 §D1.4.

use crate::track::validate_plan_invariants;
use crate::{
    DomainError, PlanSection, PlanView, TaskId, TaskStatus, TaskStatusKind, TaskTransition,
    TrackMetadata, TrackTask, TransitionError, ValidationError,
};

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

    /// Returns `true` if every task in the document is in a resolved state
    /// (`done` or `skipped`).
    ///
    /// Note: returns `true` (vacuously) for an empty task list. An empty
    /// `ImplPlanDocument` is considered to have no unresolved work. Callers that
    /// want to distinguish "all done" from "nothing to do" should additionally
    /// check `tasks().is_empty()`.
    #[must_use]
    pub fn all_tasks_resolved(&self) -> bool {
        self.tasks.iter().all(|t| t.status().is_resolved())
    }

    /// Returns the IDs of tasks that are not yet resolved (not `done` or `skipped`).
    #[must_use]
    pub fn unresolved_task_ids(&self) -> Vec<&TaskId> {
        self.tasks.iter().filter(|t| !t.status().is_resolved()).map(|t| t.id()).collect()
    }

    /// Returns the first task that is in `Todo` or `InProgress` status, if any.
    ///
    /// Tasks are considered in plan order (section order, then task order within each section).
    /// `InProgress` tasks are prioritized over `Todo` tasks: the first `InProgress` task in
    /// plan order is returned before any `Todo` task, even if the `Todo` task appears earlier
    /// in the plan.
    #[must_use]
    pub fn next_open_task(&self) -> Option<&TrackTask> {
        // Walk plan order once, collecting the first InProgress and first Todo encountered.
        let mut first_in_progress: Option<&TrackTask> = None;
        let mut first_todo: Option<&TrackTask> = None;

        'outer: for section in self.plan.sections() {
            for task_id in section.task_ids() {
                if let Some(t) = self.tasks.iter().find(|t| t.id() == task_id) {
                    match t.status().kind() {
                        TaskStatusKind::InProgress if first_in_progress.is_none() => {
                            first_in_progress = Some(t);
                            if first_todo.is_some() {
                                // Both found; no need to continue.
                                break 'outer;
                            }
                        }
                        TaskStatusKind::Todo if first_todo.is_none() => {
                            first_todo = Some(t);
                            if first_in_progress.is_some() {
                                // Both found; no need to continue.
                                break 'outer;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // InProgress takes priority; fall back to first Todo in plan order.
        first_in_progress.or(first_todo)
    }

    /// Applies a `TaskTransition` to the task identified by `task_id`, returning
    /// the updated document.
    ///
    /// # Errors
    ///
    /// Returns `TransitionError::TaskNotFound` if no task with the given ID exists.
    /// Returns `TransitionError::InvalidTaskTransition` if the transition is invalid
    /// for the current task status.
    pub fn apply_transition(
        &mut self,
        task_id: &TaskId,
        transition: TaskTransition,
    ) -> Result<(), DomainError> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id() == task_id)
            .ok_or_else(|| TransitionError::TaskNotFound { task_id: task_id.to_string() })?;
        task.transition(transition)?;
        Ok(())
    }

    /// Resolves a target status string to the correct transition and applies it.
    ///
    /// Target strings: `"todo"`, `"in_progress"`, `"done"`, `"skipped"`.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::UnsupportedTargetStatus` if the status string is
    /// unrecognised or incompatible with the current task state.
    /// Returns `TransitionError::TaskNotFound` if the task does not exist.
    /// Returns `TransitionError::InvalidTaskTransition` if the transition is invalid.
    pub fn apply_transition_by_status(
        &mut self,
        task_id: &TaskId,
        target_status: &str,
        commit_hash: Option<crate::CommitHash>,
    ) -> Result<(), DomainError> {
        let current_status = self
            .tasks
            .iter()
            .find(|t| t.id() == task_id)
            .ok_or_else(|| TransitionError::TaskNotFound { task_id: task_id.to_string() })?
            .status()
            .clone();

        let transition = resolve_transition(&current_status, target_status, commit_hash)?;
        self.apply_transition(task_id, transition)
    }

    /// Adds a new task (in `Todo` status) and appends it to the end of the
    /// specified section (or the first section if `section_id` is `None`).
    ///
    /// When `after_task_id` is `Some`, the task is inserted immediately after
    /// that task within the section; otherwise it is appended to the end.
    ///
    /// Returns the `TaskId` allocated for the new task.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::EmptyTaskDescription` if `description` is empty.
    /// Returns `ValidationError::SectionNotFound` if `section_id` points to a
    /// non-existent section.
    /// Returns `ValidationError::NoSectionsAvailable` if there are no sections
    /// and `section_id` is `None`.
    pub fn add_task(
        &mut self,
        description: impl Into<String>,
        section_id: Option<&str>,
        after_task_id: Option<&TaskId>,
    ) -> Result<TaskId, DomainError> {
        let description = description.into();
        if description.trim().is_empty() {
            return Err(ValidationError::EmptyTaskDescription.into());
        }

        // --- Validate all inputs before mutating state ---

        // Compute next task ID using checked arithmetic to prevent overflow.
        let max_suffix = self
            .tasks
            .iter()
            .filter_map(|t| t.id().as_ref().strip_prefix('T').and_then(|n| n.parse::<u64>().ok()))
            .max()
            .unwrap_or(0);
        let next_num = max_suffix.checked_add(1).ok_or_else(|| {
            ValidationError::InvalidTaskId(
                "task id counter overflow: cannot allocate a new task id".to_owned(),
            )
        })?;
        let new_id = TaskId::try_new(format!("T{next_num:03}"))?;

        // Validate target section existence before mutating self.
        let summary = self.plan.summary().to_vec();
        let mut all_sections = self.plan.sections().to_vec();

        let target_idx = match section_id {
            Some(sid) => all_sections
                .iter()
                .position(|s| s.id() == sid)
                .ok_or_else(|| ValidationError::SectionNotFound(sid.to_owned()))?,
            None => {
                if all_sections.is_empty() {
                    return Err(ValidationError::NoSectionsAvailable.into());
                }
                0
            }
        };

        // --- All validation passed; now mutate state ---

        let new_task = TrackTask::new(new_id.clone(), description)?;
        self.tasks.push(new_task);

        // Rebuild the target section with the new task inserted.
        let target_section =
            all_sections.get(target_idx).ok_or(ValidationError::NoSectionsAvailable)?.clone();
        let mut new_task_ids: Vec<TaskId> = target_section.task_ids().to_vec();
        match after_task_id {
            Some(after) => {
                if let Some(pos) = new_task_ids.iter().position(|id| id == after) {
                    new_task_ids.insert(pos + 1, new_id.clone());
                } else {
                    new_task_ids.push(new_id.clone());
                }
            }
            None => new_task_ids.push(new_id.clone()),
        }

        let rebuilt = PlanSection::new(
            target_section.id(),
            target_section.title(),
            target_section.description().to_vec(),
            new_task_ids,
        )?;
        if let Some(slot) = all_sections.get_mut(target_idx) {
            *slot = rebuilt;
        }

        self.plan = PlanView::new(summary, all_sections);
        Ok(new_id)
    }
}

/// Resolves a target status string to the correct `TaskTransition` for the
/// given current status.
///
/// Takes the full `TaskStatus` to distinguish `DonePending` from `DoneTraced`:
/// `BackfillHash` is only valid for `DonePending` (the task completed without a
/// commit hash recorded). A `DoneTraced` task already has a hash and cannot be
/// backfilled again.
fn resolve_transition(
    current: &TaskStatus,
    target: &str,
    commit_hash: Option<crate::CommitHash>,
) -> Result<TaskTransition, DomainError> {
    match (current, target) {
        (TaskStatus::Todo, "in_progress") => Ok(TaskTransition::Start),
        (TaskStatus::Todo, "skipped") => Ok(TaskTransition::Skip),
        (TaskStatus::InProgress, "done") => Ok(TaskTransition::Complete { commit_hash }),
        (TaskStatus::InProgress, "todo") => Ok(TaskTransition::ResetToTodo),
        (TaskStatus::InProgress, "skipped") => Ok(TaskTransition::Skip),
        (TaskStatus::DonePending, "in_progress")
        | (TaskStatus::DoneTraced { .. }, "in_progress") => Ok(TaskTransition::Reopen),
        (TaskStatus::DonePending, "done") => {
            // Backfill: only valid when a commit hash is provided.
            match commit_hash {
                Some(hash) => Ok(TaskTransition::BackfillHash { commit_hash: hash }),
                None => Err(ValidationError::UnsupportedTargetStatus(
                    "task is done pending; provide --commit-hash to backfill".to_string(),
                )
                .into()),
            }
        }
        (TaskStatus::DoneTraced { .. }, "done") => {
            // Task already has a commit hash; backfill is not possible.
            Err(ValidationError::UnsupportedTargetStatus(
                "task already has a recorded commit hash; use 'in_progress' to reopen".to_string(),
            )
            .into())
        }
        (TaskStatus::Skipped, "todo") => Ok(TaskTransition::ResetToTodo),
        (current, target) => Err(ValidationError::UnsupportedTargetStatus(format!(
            "cannot transition from '{}' to '{target}'",
            current.kind()
        ))
        .into()),
    }
}

// ---------------------------------------------------------------------------
// Impl-plan presence invariant
// ---------------------------------------------------------------------------

/// Error emitted by [`check_impl_plan_presence`] when an activated track is
/// missing `impl-plan.json`. Encodes the invariant `is_activated() ↔
/// impl-plan.json present`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImplPlanPresenceError {
    #[error("activated track '{track_id}' is missing impl-plan.json")]
    MissingForActivatedTrack { track_id: String },
}

/// Enforces the invariant `is_activated() ↔ impl-plan.json present`:
///
/// - Planning-only track (`!is_activated()`): `impl_plan` may be `None`
///   (planning-only tracks legitimately have no tasks yet) → `Ok(())`.
/// - Planning-only track with `impl_plan = Some(...)` → `Ok(())` (harmless
///   over-presence; planning track may have pre-allocated tasks).
/// - Activated track (`is_activated()`) with `impl_plan = Some(...)` →
///   `Ok(())`.
/// - Activated track with `impl_plan = None` → `Err(MissingForActivatedTrack)`.
///
/// Every CLI / usecase call site that reads `impl-plan.json` alongside
/// `metadata.json` must route through this validator. Do not derive
/// activation from `status_override` or the computed status — those are
/// independent of branch materialization.
///
/// # Errors
///
/// Returns [`ImplPlanPresenceError::MissingForActivatedTrack`] when the
/// track is activated but no impl-plan.json was loaded.
pub fn check_impl_plan_presence(
    track: &TrackMetadata,
    impl_plan: Option<&ImplPlanDocument>,
) -> Result<(), ImplPlanPresenceError> {
    if track.is_activated() && impl_plan.is_none() {
        return Err(ImplPlanPresenceError::MissingForActivatedTrack {
            track_id: track.id().to_string(),
        });
    }
    Ok(())
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

    // --- all_tasks_resolved ---

    #[test]
    fn test_all_tasks_resolved_with_no_tasks_returns_true() {
        // An empty task list vacuously satisfies "all tasks resolved".
        // Callers that want to distinguish "all done" from "nothing to do"
        // should additionally check `tasks().is_empty()`.
        let doc = ImplPlanDocument::new(vec![], PlanView::new(vec![], vec![])).unwrap();
        assert!(doc.all_tasks_resolved());
    }

    #[test]
    fn test_all_tasks_resolved_with_todo_task_returns_false() {
        use crate::TaskStatus;
        let t = TrackTask::with_status(TaskId::try_new("T001").unwrap(), "task", TaskStatus::Todo)
            .unwrap();
        let doc = ImplPlanDocument::new(vec![t], plan(&["T001"])).unwrap();
        assert!(!doc.all_tasks_resolved());
    }

    #[test]
    fn test_all_tasks_resolved_when_all_done_returns_true() {
        use crate::{CommitHash, TaskStatus};
        let t = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "task",
            TaskStatus::DoneTraced { commit_hash: CommitHash::try_new("abc1234").unwrap() },
        )
        .unwrap();
        let doc = ImplPlanDocument::new(vec![t], plan(&["T001"])).unwrap();
        assert!(doc.all_tasks_resolved());
    }

    // --- unresolved_task_ids ---

    #[test]
    fn test_unresolved_task_ids_returns_unresolved() {
        use crate::TaskStatus;
        let t1 = TrackTask::with_status(TaskId::try_new("T001").unwrap(), "a", TaskStatus::Todo)
            .unwrap();
        let t2 =
            TrackTask::with_status(TaskId::try_new("T002").unwrap(), "b", TaskStatus::DonePending)
                .unwrap();
        let doc = ImplPlanDocument::new(
            vec![t1, t2],
            PlanView::new(
                vec![],
                vec![
                    PlanSection::new(
                        "S1",
                        "Impl",
                        vec![],
                        vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
                    )
                    .unwrap(),
                ],
            ),
        )
        .unwrap();
        let ids = doc.unresolved_task_ids();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].as_ref(), "T001");
    }

    // --- next_open_task ---

    #[test]
    fn test_next_open_task_returns_first_todo() {
        use crate::TaskStatus;
        let t1 = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "done task",
            TaskStatus::DonePending,
        )
        .unwrap();
        let t2 = TrackTask::with_status(TaskId::try_new("T002").unwrap(), "open", TaskStatus::Todo)
            .unwrap();
        let doc = ImplPlanDocument::new(
            vec![t1, t2],
            PlanView::new(
                vec![],
                vec![
                    PlanSection::new(
                        "S1",
                        "Impl",
                        vec![],
                        vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
                    )
                    .unwrap(),
                ],
            ),
        )
        .unwrap();
        let next = doc.next_open_task().unwrap();
        assert_eq!(next.id().as_ref(), "T002");
    }

    #[test]
    fn test_next_open_task_prefers_in_progress_over_todo_in_earlier_section() {
        // T001 (Todo) is in S1, T002 (InProgress) is in S2.
        // InProgress must be returned even though T001 appears first in plan order.
        use crate::TaskStatus;
        let t1 = TrackTask::with_status(TaskId::try_new("T001").unwrap(), "todo", TaskStatus::Todo)
            .unwrap();
        let t2 = TrackTask::with_status(
            TaskId::try_new("T002").unwrap(),
            "in progress",
            TaskStatus::InProgress,
        )
        .unwrap();
        let s1 = PlanSection::new("S1", "Phase 1", vec![], vec![TaskId::try_new("T001").unwrap()])
            .unwrap();
        let s2 = PlanSection::new("S2", "Phase 2", vec![], vec![TaskId::try_new("T002").unwrap()])
            .unwrap();
        let doc = ImplPlanDocument::new(vec![t1, t2], PlanView::new(vec![], vec![s1, s2])).unwrap();
        let next = doc.next_open_task().unwrap();
        assert_eq!(
            next.id().as_ref(),
            "T002",
            "InProgress task must be preferred over Todo task in earlier section"
        );
    }

    #[test]
    fn test_next_open_task_respects_plan_order_for_todo_tasks() {
        // T002 is stored first in tasks Vec, but T001 comes first in plan order.
        // next_open_task must return T001 (plan order).
        use crate::TaskStatus;
        let t2 =
            TrackTask::with_status(TaskId::try_new("T002").unwrap(), "second", TaskStatus::Todo)
                .unwrap();
        let t1 =
            TrackTask::with_status(TaskId::try_new("T001").unwrap(), "first", TaskStatus::Todo)
                .unwrap();
        // Plan order: T001 then T002.
        let s = PlanSection::new(
            "S1",
            "Impl",
            vec![],
            vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
        )
        .unwrap();
        // tasks Vec intentionally stores T002 before T001 to test plan-order traversal.
        let doc = ImplPlanDocument::new(vec![t2, t1], PlanView::new(vec![], vec![s])).unwrap();
        let next = doc.next_open_task().unwrap();
        assert_eq!(next.id().as_ref(), "T001", "plan order must be respected for Todo tasks");
    }

    // --- apply_transition ---

    #[test]
    fn test_apply_transition_todo_to_in_progress_succeeds() {
        use crate::{TaskStatus, TaskTransition};
        let doc = &mut ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        let id = TaskId::try_new("T001").unwrap();
        doc.apply_transition(&id, TaskTransition::Start).unwrap();
        assert!(matches!(doc.tasks()[0].status(), TaskStatus::InProgress));
    }

    #[test]
    fn test_apply_transition_with_unknown_task_returns_error() {
        use crate::{TaskTransition, TransitionError};
        let mut doc = ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        let id = TaskId::try_new("T999").unwrap();
        let err = doc.apply_transition(&id, TaskTransition::Start).unwrap_err();
        assert!(
            matches!(err, DomainError::Transition(TransitionError::TaskNotFound { ref task_id }) if task_id == "T999"),
            "unexpected: {err:?}"
        );
    }

    // --- apply_transition_by_status ---

    #[test]
    fn test_apply_transition_by_status_todo_to_in_progress_succeeds() {
        use crate::TaskStatus;
        let mut doc = ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        let id = TaskId::try_new("T001").unwrap();
        doc.apply_transition_by_status(&id, "in_progress", None).unwrap();
        assert!(matches!(doc.tasks()[0].status(), TaskStatus::InProgress));
    }

    #[test]
    fn test_apply_transition_by_status_in_progress_to_done_with_hash_succeeds() {
        use crate::{CommitHash, TaskStatus};
        let t = crate::TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "task",
            TaskStatus::InProgress,
        )
        .unwrap();
        let mut doc = ImplPlanDocument::new(vec![t], plan(&["T001"])).unwrap();
        let id = TaskId::try_new("T001").unwrap();
        let hash = CommitHash::try_new("abc1234").unwrap();
        doc.apply_transition_by_status(&id, "done", Some(hash)).unwrap();
        assert!(matches!(doc.tasks()[0].status(), TaskStatus::DoneTraced { .. }));
    }

    #[test]
    fn test_apply_transition_by_status_unsupported_target_returns_error() {
        let mut doc = ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        let id = TaskId::try_new("T001").unwrap();
        let err = doc.apply_transition_by_status(&id, "invalid_status", None).unwrap_err();
        assert!(
            matches!(err, DomainError::Validation(ValidationError::UnsupportedTargetStatus(_))),
            "unexpected: {err:?}"
        );
    }

    #[test]
    fn test_apply_transition_by_status_done_traced_to_done_returns_unsupported() {
        // DoneTraced tasks cannot be backfilled again; the API must reject "done" → "done"
        // when the task is already DoneTraced (not DonePending).
        use crate::{CommitHash, TaskStatus};
        let hash = CommitHash::try_new("abc1234").unwrap();
        let t = crate::TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "task",
            TaskStatus::DoneTraced { commit_hash: hash.clone() },
        )
        .unwrap();
        let mut doc = ImplPlanDocument::new(vec![t], plan(&["T001"])).unwrap();
        let id = TaskId::try_new("T001").unwrap();
        let err = doc.apply_transition_by_status(&id, "done", Some(hash)).unwrap_err();
        assert!(
            matches!(err, DomainError::Validation(ValidationError::UnsupportedTargetStatus(_))),
            "DoneTraced->done must return UnsupportedTargetStatus, got: {err:?}"
        );
    }

    #[test]
    fn test_apply_transition_by_status_done_pending_to_done_with_hash_succeeds() {
        // DonePending tasks can be backfilled with a commit hash.
        use crate::{CommitHash, TaskStatus};
        let t = crate::TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "task",
            TaskStatus::DonePending,
        )
        .unwrap();
        let mut doc = ImplPlanDocument::new(vec![t], plan(&["T001"])).unwrap();
        let id = TaskId::try_new("T001").unwrap();
        let hash = CommitHash::try_new("def5678").unwrap();
        doc.apply_transition_by_status(&id, "done", Some(hash)).unwrap();
        assert!(
            matches!(doc.tasks()[0].status(), TaskStatus::DoneTraced { .. }),
            "DonePending->done with hash must produce DoneTraced"
        );
    }

    // --- add_task ---

    #[test]
    fn test_add_task_appends_to_first_section_when_no_section_specified() {
        let mut doc = ImplPlanDocument::new(vec![task("T001", "first")], plan(&["T001"])).unwrap();
        let new_id = doc.add_task("second task", None, None).unwrap();
        assert_eq!(new_id.as_ref(), "T002");
        assert_eq!(doc.tasks().len(), 2);
        let section = &doc.plan().sections()[0];
        assert_eq!(section.task_ids().len(), 2);
        assert_eq!(section.task_ids()[1].as_ref(), "T002");
    }

    #[test]
    fn test_add_task_with_empty_description_returns_error() {
        let mut doc = ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        let err = doc.add_task("", None, None).unwrap_err();
        assert!(
            matches!(err, DomainError::Validation(ValidationError::EmptyTaskDescription)),
            "unexpected: {err:?}"
        );
    }

    #[test]
    fn test_add_task_with_invalid_section_returns_error() {
        let mut doc = ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        let err = doc.add_task("new", Some("S999"), None).unwrap_err();
        assert!(
            matches!(err, DomainError::Validation(ValidationError::SectionNotFound(_))),
            "unexpected: {err:?}"
        );
    }

    #[test]
    fn test_add_task_invalid_section_does_not_corrupt_document() {
        // Verify that a failed add_task leaves the document unchanged (no partial mutation).
        let mut doc = ImplPlanDocument::new(vec![task("T001", "task")], plan(&["T001"])).unwrap();
        let before_task_count = doc.tasks().len();
        let before_section_count = doc.plan().sections().len();
        let before_section_task_count = doc.plan().sections()[0].task_ids().len();

        let _ = doc.add_task("new task", Some("S_nonexistent"), None);

        assert_eq!(doc.tasks().len(), before_task_count, "task count must not change on error");
        assert_eq!(
            doc.plan().sections().len(),
            before_section_count,
            "section count must not change on error"
        );
        assert_eq!(
            doc.plan().sections()[0].task_ids().len(),
            before_section_task_count,
            "section task ids must not change on error"
        );
    }

    #[test]
    fn test_add_task_inserts_after_specified_task() {
        let tasks = vec![task("T001", "A"), task("T002", "B")];
        let s = PlanSection::new(
            "S1",
            "Impl",
            vec![],
            vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
        )
        .unwrap();
        let mut doc = ImplPlanDocument::new(tasks, PlanView::new(vec![], vec![s])).unwrap();
        let after = TaskId::try_new("T001").unwrap();
        let new_id = doc.add_task("between A and B", None, Some(&after)).unwrap();
        let task_ids: Vec<&str> =
            doc.plan().sections()[0].task_ids().iter().map(|id| id.as_ref()).collect();
        assert_eq!(task_ids, vec!["T001", new_id.as_ref(), "T002"]);
    }

    // -----------------------------------------------------------------------
    // check_impl_plan_presence
    // -----------------------------------------------------------------------

    use super::{ImplPlanPresenceError, check_impl_plan_presence};
    use crate::{StatusOverride, TrackBranch, TrackId, TrackMetadata};

    fn branchless_planning_track(id: &str) -> TrackMetadata {
        TrackMetadata::new(TrackId::try_new(id).unwrap(), "Planning-only", None).unwrap()
    }

    fn activated_track(id: &str) -> TrackMetadata {
        TrackMetadata::with_branch(
            TrackId::try_new(id).unwrap(),
            Some(TrackBranch::try_new(format!("track/{id}")).unwrap()),
            "Activated",
            None,
        )
        .unwrap()
    }

    fn empty_impl_plan() -> ImplPlanDocument {
        ImplPlanDocument::new(vec![], PlanView::new(vec![], vec![])).unwrap()
    }

    #[test]
    fn planning_only_without_impl_plan_is_ok() {
        let track = branchless_planning_track("example");
        assert!(check_impl_plan_presence(&track, None).is_ok());
    }

    #[test]
    fn planning_only_with_override_without_impl_plan_is_ok() {
        let mut track = branchless_planning_track("example");
        track.set_status_override(Some(StatusOverride::blocked("waiting").unwrap()));
        assert!(
            check_impl_plan_presence(&track, None).is_ok(),
            "override does not imply activation"
        );
    }

    #[test]
    fn planning_only_with_impl_plan_is_ok() {
        let track = branchless_planning_track("example");
        let plan = empty_impl_plan();
        assert!(check_impl_plan_presence(&track, Some(&plan)).is_ok());
    }

    #[test]
    fn activated_with_impl_plan_is_ok() {
        let track = activated_track("example");
        let plan = empty_impl_plan();
        assert!(check_impl_plan_presence(&track, Some(&plan)).is_ok());
    }

    #[test]
    fn activated_without_impl_plan_is_corruption() {
        let track = activated_track("example");
        let err = check_impl_plan_presence(&track, None).unwrap_err();
        assert_eq!(
            err,
            ImplPlanPresenceError::MissingForActivatedTrack { track_id: "example".to_owned() }
        );
    }
}
