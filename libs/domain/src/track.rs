use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::{
    CommitHash, DomainError, NonEmptyString, PlanView, TaskId, TrackBranch, TrackId,
    TransitionError, ValidationError,
};

/// Derived status of a track, computed from its task states and optional override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackStatus {
    Planned,
    InProgress,
    Done,
    Blocked,
    Cancelled,
    Archived,
}

impl fmt::Display for TrackStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Planned => "planned",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
            Self::Archived => "archived",
        };
        f.write_str(value)
    }
}

/// Discriminant-only view of `TaskStatus` for display and error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatusKind {
    Todo,
    InProgress,
    Done,
    Skipped,
}

impl fmt::Display for TaskStatusKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Skipped => "skipped",
        };
        f.write_str(value)
    }
}

/// Status of a task.
///
/// `DonePending` means the task is complete but the commit hash is not yet known.
/// `DoneTraced` means the task is complete with a traced commit hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    DonePending,
    DoneTraced { commit_hash: CommitHash },
    Skipped,
}

impl TaskStatus {
    /// Returns the discriminant kind of this status.
    #[must_use]
    pub fn kind(&self) -> TaskStatusKind {
        match self {
            Self::Todo => TaskStatusKind::Todo,
            Self::InProgress => TaskStatusKind::InProgress,
            Self::DonePending | Self::DoneTraced { .. } => TaskStatusKind::Done,
            Self::Skipped => TaskStatusKind::Skipped,
        }
    }

    /// Returns `true` if the task is in a terminal state (Done or Skipped).
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::DonePending | Self::DoneTraced { .. } | Self::Skipped)
    }
}

/// Command enum representing valid task state transition requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskTransition {
    Start,
    Complete { commit_hash: Option<CommitHash> },
    BackfillHash { commit_hash: CommitHash },
    ResetToTodo,
    Skip,
    Reopen,
}

impl TaskTransition {
    /// Returns the target status kind this transition aims for.
    #[must_use]
    pub fn target_kind(&self) -> TaskStatusKind {
        match self {
            Self::Start => TaskStatusKind::InProgress,
            Self::Complete { .. } | Self::BackfillHash { .. } => TaskStatusKind::Done,
            Self::ResetToTodo => TaskStatusKind::Todo,
            Self::Skip => TaskStatusKind::Skipped,
            Self::Reopen => TaskStatusKind::InProgress,
        }
    }
}

/// The kind of status override (discriminant only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusOverrideKind {
    Blocked,
    Cancelled,
}

impl std::fmt::Display for StatusOverrideKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocked => f.write_str("blocked"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

/// Manual override for track status (Blocked or Cancelled with reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusOverride {
    kind: StatusOverrideKind,
    reason: NonEmptyString,
}

impl StatusOverride {
    /// Creates a Blocked override.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyString` if `reason` is empty.
    pub fn blocked(reason: impl Into<String>) -> Result<Self, ValidationError> {
        Ok(Self { kind: StatusOverrideKind::Blocked, reason: NonEmptyString::try_new(reason)? })
    }

    /// Creates a Cancelled override.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyString` if `reason` is empty.
    pub fn cancelled(reason: impl Into<String>) -> Result<Self, ValidationError> {
        Ok(Self { kind: StatusOverrideKind::Cancelled, reason: NonEmptyString::try_new(reason)? })
    }

    /// Creates a StatusOverride from a kind and validated reason (codec path).
    #[must_use]
    pub fn from_parts(kind: StatusOverrideKind, reason: NonEmptyString) -> Self {
        Self { kind, reason }
    }

    /// Returns the override kind.
    #[must_use]
    pub fn kind(&self) -> StatusOverrideKind {
        self.kind
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        self.reason.as_ref()
    }

    #[must_use]
    pub fn track_status(&self) -> TrackStatus {
        match self.kind {
            StatusOverrideKind::Blocked => TrackStatus::Blocked,
            StatusOverrideKind::Cancelled => TrackStatus::Cancelled,
        }
    }
}

/// A single task within a track, with its own state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackTask {
    id: TaskId,
    description: NonEmptyString,
    status: TaskStatus,
}

impl TrackTask {
    /// Creates a new task in `Todo` status.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyTaskDescription` if description is empty.
    pub fn new(id: TaskId, description: impl Into<String>) -> Result<Self, ValidationError> {
        Self::with_status(id, description, TaskStatus::Todo)
    }

    pub fn with_status(
        id: TaskId,
        description: impl Into<String>,
        status: TaskStatus,
    ) -> Result<Self, ValidationError> {
        let description = NonEmptyString::try_new(description)
            .map_err(|_| ValidationError::EmptyTaskDescription)?;
        Ok(Self { id, description, status })
    }

    #[must_use]
    pub fn id(&self) -> &TaskId {
        &self.id
    }

    #[must_use]
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }

    #[must_use]
    pub fn status(&self) -> &TaskStatus {
        &self.status
    }

    /// Applies a state transition to this task.
    ///
    /// # Errors
    /// Returns `TransitionError::InvalidTaskTransition` if the transition is not allowed.
    pub fn transition(&mut self, transition: TaskTransition) -> Result<(), TransitionError> {
        let from = self.status.kind();
        let next_status = match (&self.status, transition) {
            (TaskStatus::Todo, TaskTransition::Start) => TaskStatus::InProgress,
            (TaskStatus::Todo, TaskTransition::Skip) => TaskStatus::Skipped,
            (TaskStatus::InProgress, TaskTransition::Complete { commit_hash: None }) => {
                TaskStatus::DonePending
            }
            (TaskStatus::InProgress, TaskTransition::Complete { commit_hash: Some(hash) }) => {
                TaskStatus::DoneTraced { commit_hash: hash }
            }
            (TaskStatus::InProgress, TaskTransition::ResetToTodo) => TaskStatus::Todo,
            (TaskStatus::InProgress, TaskTransition::Skip) => TaskStatus::Skipped,
            (TaskStatus::DonePending, TaskTransition::BackfillHash { commit_hash }) => {
                TaskStatus::DoneTraced { commit_hash }
            }
            (TaskStatus::DonePending, TaskTransition::Reopen)
            | (TaskStatus::DoneTraced { .. }, TaskTransition::Reopen) => TaskStatus::InProgress,
            (TaskStatus::Skipped, TaskTransition::ResetToTodo) => TaskStatus::Todo,
            (_, transition) => {
                return Err(TransitionError::InvalidTaskTransition {
                    task_id: self.id.to_string(),
                    from,
                    to: transition.target_kind(),
                });
            }
        };

        self.status = next_status;
        Ok(())
    }
}

/// Root aggregate for a track: tasks, plan, and optional status override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMetadata {
    id: TrackId,
    branch: Option<TrackBranch>,
    title: NonEmptyString,
    tasks: Vec<TrackTask>,
    plan: PlanView,
    status_override: Option<StatusOverride>,
}

impl TrackMetadata {
    /// Creates a new `TrackMetadata`, validating plan-task referential integrity.
    ///
    /// # Errors
    /// Returns `DomainError` on empty title, duplicate tasks/sections, or plan-task mismatch.
    pub fn new(
        id: TrackId,
        title: impl Into<String>,
        tasks: Vec<TrackTask>,
        plan: PlanView,
        status_override: Option<StatusOverride>,
    ) -> Result<Self, DomainError> {
        Self::with_branch(id, None, title, tasks, plan, status_override)
    }

    /// Creates a new `TrackMetadata` with an optional branch field.
    ///
    /// # Errors
    /// Returns `DomainError` on empty title, duplicate tasks/sections, or plan-task mismatch.
    pub fn with_branch(
        id: TrackId,
        branch: Option<TrackBranch>,
        title: impl Into<String>,
        tasks: Vec<TrackTask>,
        plan: PlanView,
        status_override: Option<StatusOverride>,
    ) -> Result<Self, DomainError> {
        let title = NonEmptyString::try_new(title).map_err(|_| ValidationError::EmptyTrackTitle)?;

        validate_plan_invariants(&tasks, &plan)?;

        let track = Self { id, branch, title, tasks, plan, status_override };
        track.ensure_override_is_compatible()?;

        Ok(track)
    }

    #[must_use]
    pub fn id(&self) -> &TrackId {
        &self.id
    }

    #[must_use]
    pub fn branch(&self) -> Option<&TrackBranch> {
        self.branch.as_ref()
    }

    pub fn set_branch(&mut self, branch: Option<TrackBranch>) {
        self.branch = branch;
    }

    #[must_use]
    pub fn title(&self) -> &str {
        self.title.as_ref()
    }

    #[must_use]
    pub fn tasks(&self) -> &[TrackTask] {
        &self.tasks
    }

    #[must_use]
    pub fn plan(&self) -> &PlanView {
        &self.plan
    }

    #[must_use]
    pub fn status_override(&self) -> Option<&StatusOverride> {
        self.status_override.as_ref()
    }

    #[must_use]
    pub fn status(&self) -> TrackStatus {
        if let Some(status_override) = &self.status_override {
            return status_override.track_status();
        }

        if self.tasks.is_empty()
            || self.tasks.iter().all(|task| task.status.kind() == TaskStatusKind::Todo)
        {
            return TrackStatus::Planned;
        }

        if self.tasks_are_resolved() {
            return TrackStatus::Done;
        }

        TrackStatus::InProgress
    }

    /// Sets or clears the status override.
    ///
    /// # Errors
    /// Returns `DomainError` if all tasks are resolved and override is incompatible.
    pub fn set_status_override(
        &mut self,
        status_override: Option<StatusOverride>,
    ) -> Result<(), DomainError> {
        if let Some(status_override) = &status_override {
            if self.tasks_are_resolved() {
                return Err(ValidationError::OverrideIncompatibleWithResolvedTasks(
                    status_override.track_status(),
                )
                .into());
            }
        }

        self.status_override = status_override;
        Ok(())
    }

    /// Transitions a task and auto-clears override if all tasks become resolved.
    ///
    /// # Errors
    /// Returns `DomainError` if the task is not found or the transition is invalid.
    pub fn transition_task(
        &mut self,
        task_id: &TaskId,
        transition: TaskTransition,
    ) -> Result<(), DomainError> {
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.id == *task_id)
            .ok_or_else(|| TransitionError::TaskNotFound { task_id: task_id.to_string() })?;

        task.transition(transition)?;
        self.clear_override_if_resolved();
        Ok(())
    }

    #[must_use]
    pub fn next_open_task(&self) -> Option<&TrackTask> {
        let task_map: HashMap<&TaskId, &TrackTask> =
            self.tasks.iter().map(|task| (task.id(), task)).collect();

        for task_id in self.ordered_task_ids() {
            if let Some(task) = task_map.get(task_id) {
                if task.status.kind() == TaskStatusKind::InProgress {
                    return Some(*task);
                }
            }
        }

        for task_id in self.ordered_task_ids() {
            if let Some(task) = task_map.get(task_id) {
                if task.status.kind() == TaskStatusKind::Todo {
                    return Some(*task);
                }
            }
        }

        None
    }

    /// Returns `true` if the task list is non-empty and every task has status `Done` or `Skipped`.
    ///
    /// An empty task list returns `false` — a track with no tasks should not bypass the guard.
    /// Used by the PR push guard to ensure all tasks are resolved before pushing.
    #[must_use]
    pub fn all_tasks_resolved(&self) -> bool {
        !self.tasks.is_empty()
            && self.tasks.iter().all(|t| {
                matches!(
                    t.status(),
                    TaskStatus::DonePending | TaskStatus::DoneTraced { .. } | TaskStatus::Skipped
                )
            })
    }

    fn tasks_are_resolved(&self) -> bool {
        !self.tasks.is_empty() && self.tasks.iter().all(|task| task.status.is_resolved())
    }

    fn clear_override_if_resolved(&mut self) {
        if self.tasks_are_resolved() {
            self.status_override = None;
        }
    }

    fn ensure_override_is_compatible(&self) -> Result<(), ValidationError> {
        if let Some(status_override) = &self.status_override {
            if self.tasks_are_resolved() {
                return Err(ValidationError::OverrideIncompatibleWithResolvedTasks(
                    status_override.track_status(),
                ));
            }
        }

        Ok(())
    }

    /// Generates the next sequential `TaskId` based on existing tasks.
    ///
    /// Scans all tasks for the highest numeric suffix and returns `T<max+1>` zero-padded to 3 digits.
    ///
    /// # Errors
    /// Returns `ValidationError::InvalidTaskId` if the generated ID is invalid (should not happen).
    pub fn next_task_id(&self) -> Result<TaskId, ValidationError> {
        // TaskId validates that the numeric suffix fits in u64, so parse is infallible here.
        let max_num: u64 = self
            .tasks
            .iter()
            .filter_map(|t| {
                t.id().as_ref().strip_prefix('T').and_then(|d: &str| d.parse::<u64>().ok())
            })
            .max()
            .unwrap_or(0);
        let next = max_num.checked_add(1).ok_or_else(|| {
            ValidationError::InvalidTaskId(
                "task ID overflow: max T-number exceeded u64".to_string(),
            )
        })?;
        TaskId::try_new(format!("T{next:03}"))
    }

    /// Adds a new task to this track.
    ///
    /// The task is created in `Todo` status with the next sequential ID.
    /// It is appended to the tasks list and inserted into the specified section
    /// (or the first section if `section_id` is `None`).
    ///
    /// # Errors
    /// - `ValidationError::EmptyTaskDescription` if description is empty.
    /// - `ValidationError::SectionNotFound` if the specified section does not exist.
    /// - `ValidationError::NoSectionsAvailable` if no sections exist.
    pub fn add_task(
        &mut self,
        description: impl Into<String>,
        section_id: Option<&str>,
        after_task_id: Option<&TaskId>,
    ) -> Result<TaskId, DomainError> {
        let task_id = self.next_task_id()?;
        let task = TrackTask::new(task_id.clone(), description)?;

        self.plan.insert_task_into_section(task_id.clone(), section_id, after_task_id)?;
        self.tasks.push(task);

        Ok(task_id)
    }

    /// Validates that existing task descriptions have not been modified.
    ///
    /// Tasks that exist in both `self` and `previous` (matched by ID) must
    /// have identical descriptions. New tasks (IDs not in `previous`) are allowed.
    ///
    /// # Errors
    /// Returns `ValidationError::TaskDescriptionMutated` if a description changed.
    pub fn validate_descriptions_unchanged(
        &self,
        previous: &TrackMetadata,
    ) -> Result<(), ValidationError> {
        let prev_map: HashMap<&TaskId, &str> =
            previous.tasks.iter().map(|t| (t.id(), t.description())).collect();

        for task in &self.tasks {
            if let Some(prev_desc) = prev_map.get(task.id()) {
                if task.description() != *prev_desc {
                    return Err(ValidationError::TaskDescriptionMutated {
                        task_id: task.id().to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Validates that no previously existing tasks have been removed.
    ///
    /// Every task ID in `previous` must still exist in `self`.
    /// New tasks (IDs not in `previous`) are allowed.
    ///
    /// # Errors
    /// Returns `ValidationError::TaskRemoved` if a previously existing task is absent.
    pub fn validate_no_tasks_removed(
        &self,
        previous: &TrackMetadata,
    ) -> Result<(), ValidationError> {
        let new_ids: HashSet<&TaskId> = self.tasks.iter().map(|t| t.id()).collect();

        for prev_task in &previous.tasks {
            if !new_ids.contains(prev_task.id()) {
                return Err(ValidationError::TaskRemoved { task_id: prev_task.id().to_string() });
            }
        }
        Ok(())
    }

    fn ordered_task_ids(&self) -> Vec<&TaskId> {
        let mut ordered = Vec::new();
        let mut seen = HashSet::new();

        for section in self.plan.sections() {
            for task_id in section.task_ids() {
                if seen.insert(task_id) {
                    ordered.push(task_id);
                }
            }
        }

        ordered
    }
}

pub(crate) fn validate_plan_invariants(
    tasks: &[TrackTask],
    plan: &PlanView,
) -> Result<(), ValidationError> {
    let mut task_ids = HashSet::new();
    for task in tasks {
        if !task_ids.insert(task.id().clone()) {
            return Err(ValidationError::DuplicateTaskId(task.id().to_string()));
        }
    }

    let mut section_ids = HashSet::new();
    let mut task_ref_counts: HashMap<TaskId, usize> = HashMap::new();
    for section in plan.sections() {
        if !section_ids.insert(section.id().to_owned()) {
            return Err(ValidationError::DuplicatePlanSectionId(section.id().to_owned()));
        }

        for task_id in section.task_ids() {
            if !task_ids.contains(task_id) {
                return Err(ValidationError::UnknownTaskReference(task_id.to_string()));
            }
            *task_ref_counts.entry(task_id.clone()).or_insert(0) += 1;
        }
    }

    for task in tasks {
        match task_ref_counts.get(task.id()) {
            None => return Err(ValidationError::UnreferencedTask(task.id().to_string())),
            Some(count) if *count > 1 => {
                return Err(ValidationError::DuplicateTaskReference(task.id().to_string()));
            }
            Some(_) => {}
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::PlanSection;
    use rstest::rstest;

    fn make_track(task_ids: &[&str], section_task_ids: &[&str]) -> TrackMetadata {
        let tasks: Vec<TrackTask> = task_ids
            .iter()
            .map(|id| TrackTask::new(TaskId::try_new(*id).unwrap(), format!("Task {id}")).unwrap())
            .collect();
        let section = PlanSection::new(
            "S1",
            "Section 1",
            vec![],
            section_task_ids.iter().map(|id| TaskId::try_new(*id).unwrap()).collect(),
        )
        .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        TrackMetadata::new(TrackId::try_new("test-track").unwrap(), "Test Track", tasks, plan, None)
            .unwrap()
    }

    #[test]
    fn test_next_task_id_with_no_tasks_returns_t001() {
        let plan = PlanView::new(vec![], vec![]);
        let track = TrackMetadata {
            id: TrackId::try_new("test-track").unwrap(),
            branch: None,
            title: NonEmptyString::try_new("Test").unwrap(),
            tasks: vec![],
            plan,
            status_override: None,
        };
        assert_eq!(track.next_task_id().unwrap().as_ref(), "T001");
    }

    #[test]
    fn test_next_task_id_with_existing_tasks_returns_next() {
        let track = make_track(&["T001", "T002", "T003"], &["T001", "T002", "T003"]);
        assert_eq!(track.next_task_id().unwrap().as_ref(), "T004");
    }

    #[test]
    fn test_next_task_id_with_gaps_uses_max() {
        let track = make_track(&["T001", "T010"], &["T001", "T010"]);
        assert_eq!(track.next_task_id().unwrap().as_ref(), "T011");
    }

    #[test]
    fn test_add_task_appends_to_first_section_by_default() {
        let mut track = make_track(&["T001"], &["T001"]);
        let new_id = track.add_task("New task", None, None).unwrap();
        assert_eq!(new_id.as_ref(), "T002");
        assert_eq!(track.tasks().len(), 2);
        assert_eq!(track.tasks()[1].description(), "New task");
        assert_eq!(track.plan().sections()[0].task_ids().len(), 2);
        assert_eq!(track.plan().sections()[0].task_ids()[1], new_id);
    }

    #[test]
    fn test_add_task_inserts_after_specified_task() {
        let mut track = make_track(&["T001", "T002"], &["T001", "T002"]);
        let after = TaskId::try_new("T001").unwrap();
        let new_id = track.add_task("Inserted task", None, Some(&after)).unwrap();
        assert_eq!(new_id.as_ref(), "T003");
        let section_ids: Vec<&str> =
            track.plan().sections()[0].task_ids().iter().map(|id| id.as_ref()).collect();
        assert_eq!(section_ids, vec!["T001", "T003", "T002"]);
    }

    #[test]
    fn test_add_task_with_unknown_after_appends_to_end() {
        let mut track = make_track(&["T001"], &["T001"]);
        let unknown = TaskId::try_new("T999").unwrap();
        let new_id = track.add_task("Appended task", None, Some(&unknown)).unwrap();
        let section_ids: Vec<&str> =
            track.plan().sections()[0].task_ids().iter().map(|id| id.as_ref()).collect();
        assert_eq!(section_ids, vec!["T001", new_id.as_ref()]);
    }

    #[test]
    fn test_add_task_to_named_section() {
        let tasks = vec![TrackTask::new(TaskId::try_new("T001").unwrap(), "Task 1").unwrap()];
        let s1 =
            PlanSection::new("S1", "Section 1", vec![], vec![TaskId::try_new("T001").unwrap()])
                .unwrap();
        let s2 = PlanSection::new("S2", "Section 2", vec![], vec![]).unwrap();
        let plan = PlanView::new(vec![], vec![s1, s2]);
        let mut track = TrackMetadata {
            id: TrackId::try_new("test-track").unwrap(),
            branch: None,
            title: NonEmptyString::try_new("Test").unwrap(),
            tasks,
            plan,
            status_override: None,
        };
        let new_id = track.add_task("Task in S2", Some("S2"), None).unwrap();
        assert_eq!(track.plan().sections()[1].task_ids().len(), 1);
        assert_eq!(track.plan().sections()[1].task_ids()[0], new_id);
    }

    #[test]
    fn test_add_task_with_unknown_section_returns_error() {
        let mut track = make_track(&["T001"], &["T001"]);
        let result = track.add_task("Bad section", Some("S999"), None);
        assert!(matches!(
            result,
            Err(DomainError::Validation(ValidationError::SectionNotFound(s))) if s == "S999"
        ));
    }

    #[test]
    fn test_add_task_with_empty_description_returns_error() {
        let mut track = make_track(&["T001"], &["T001"]);
        let result = track.add_task("", None, None);
        assert!(matches!(
            result,
            Err(DomainError::Validation(ValidationError::EmptyTaskDescription))
        ));
    }

    #[rstest]
    #[case(&["T001", "T002", "T003"], "T004")]
    #[case(&["T099"], "T100")]
    #[case(&[], "T001")]
    fn test_next_task_id_parametrized(#[case] existing: &[&str], #[case] expected: &str) {
        let tasks: Vec<TrackTask> = existing
            .iter()
            .map(|id| TrackTask::new(TaskId::try_new(*id).unwrap(), format!("Task {id}")).unwrap())
            .collect();
        let section_ids: Vec<TaskId> =
            existing.iter().map(|id| TaskId::try_new(*id).unwrap()).collect();
        let sections = if existing.is_empty() {
            vec![]
        } else {
            vec![PlanSection::new("S1", "Section 1", vec![], section_ids).unwrap()]
        };
        let plan = PlanView::new(vec![], sections);
        let track = TrackMetadata {
            id: TrackId::try_new("test-track").unwrap(),
            branch: None,
            title: NonEmptyString::try_new("Test").unwrap(),
            tasks,
            plan,
            status_override: None,
        };
        assert_eq!(track.next_task_id().unwrap().as_ref(), expected);
    }

    #[test]
    fn test_add_task_with_no_sections_returns_error() {
        let plan = PlanView::new(vec![], vec![]);
        let mut track = TrackMetadata {
            id: TrackId::try_new("test-track").unwrap(),
            branch: None,
            title: NonEmptyString::try_new("Test").unwrap(),
            tasks: vec![],
            plan,
            status_override: None,
        };
        let result = track.add_task("Some task", None, None);
        assert!(matches!(
            result,
            Err(DomainError::Validation(ValidationError::NoSectionsAvailable))
        ));
    }

    #[test]
    fn test_validate_descriptions_unchanged_rejects_mutation() {
        let original = make_track(&["T001", "T002"], &["T001", "T002"]);

        let t1 = TrackTask::new(TaskId::try_new("T001").unwrap(), "CHANGED description").unwrap();
        let t2 = TrackTask::new(TaskId::try_new("T002").unwrap(), "Task T002").unwrap();
        let section = PlanSection::new(
            "S1",
            "Section 1",
            vec![],
            vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
        )
        .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        let modified = TrackMetadata::new(
            TrackId::try_new("test-track").unwrap(),
            "Test Track",
            vec![t1, t2],
            plan,
            None,
        )
        .unwrap();

        let result = modified.validate_descriptions_unchanged(&original);
        assert!(
            matches!(result, Err(ValidationError::TaskDescriptionMutated { ref task_id }) if task_id == "T001"),
            "expected TaskDescriptionMutated for T001, got: {result:?}"
        );
    }

    #[test]
    fn test_validate_descriptions_unchanged_accepts_unchanged() {
        let track = make_track(&["T001", "T002"], &["T001", "T002"]);
        let result = track.validate_descriptions_unchanged(&track);
        assert!(result.is_ok(), "expected Ok for unchanged descriptions, got: {result:?}");
    }

    #[test]
    fn test_validate_descriptions_unchanged_accepts_new_task() {
        let original = make_track(&["T001"], &["T001"]);

        let t1 = TrackTask::new(TaskId::try_new("T001").unwrap(), "Task T001").unwrap();
        let t2 = TrackTask::new(TaskId::try_new("T002").unwrap(), "Brand new task").unwrap();
        let section = PlanSection::new(
            "S1",
            "Section 1",
            vec![],
            vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
        )
        .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        let updated = TrackMetadata::new(
            TrackId::try_new("test-track").unwrap(),
            "Test Track",
            vec![t1, t2],
            plan,
            None,
        )
        .unwrap();

        let result = updated.validate_descriptions_unchanged(&original);
        assert!(result.is_ok(), "expected Ok when adding a new task, got: {result:?}");
    }

    #[test]
    fn test_validate_no_tasks_removed_rejects_task_removal() {
        let original = make_track(&["T001", "T002"], &["T001", "T002"]);

        // New state has only T001 — T002 was removed.
        let updated = make_track(&["T001"], &["T001"]);

        let result = updated.validate_no_tasks_removed(&original);
        assert!(
            matches!(result, Err(ValidationError::TaskRemoved { ref task_id }) if task_id == "T002"),
            "expected TaskRemoved for T002, got: {result:?}"
        );
    }

    // --- all_tasks_resolved tests ---

    #[test]
    fn test_all_tasks_resolved_returns_false_with_todo_tasks() {
        let track = make_track(&["T001", "T002"], &["T001", "T002"]);
        assert!(!track.all_tasks_resolved());
    }

    #[test]
    fn test_all_tasks_resolved_returns_false_with_in_progress_tasks() {
        let mut track = make_track(&["T001"], &["T001"]);
        let task_id = TaskId::try_new("T001").unwrap();
        track.transition_task(&task_id, TaskTransition::Start).unwrap();
        assert!(!track.all_tasks_resolved());
    }

    #[test]
    fn test_all_tasks_resolved_returns_true_with_all_done() {
        let mut track = make_track(&["T001", "T002"], &["T001", "T002"]);
        for id_str in &["T001", "T002"] {
            let task_id = TaskId::try_new(*id_str).unwrap();
            track.transition_task(&task_id, TaskTransition::Start).unwrap();
            track
                .transition_task(&task_id, TaskTransition::Complete { commit_hash: None })
                .unwrap();
        }
        assert!(track.all_tasks_resolved());
    }

    #[test]
    fn test_all_tasks_resolved_returns_true_with_done_and_skipped() {
        let mut track = make_track(&["T001", "T002"], &["T001", "T002"]);
        let t1 = TaskId::try_new("T001").unwrap();
        let t2 = TaskId::try_new("T002").unwrap();
        track.transition_task(&t1, TaskTransition::Start).unwrap();
        track.transition_task(&t1, TaskTransition::Complete { commit_hash: None }).unwrap();
        track.transition_task(&t2, TaskTransition::Skip).unwrap();
        assert!(track.all_tasks_resolved());
    }

    #[test]
    fn test_all_tasks_resolved_returns_false_with_empty_tasks() {
        let plan = PlanView::new(vec![], vec![]);
        let track = TrackMetadata {
            id: TrackId::try_new("test-track").unwrap(),
            branch: None,
            title: NonEmptyString::try_new("Test").unwrap(),
            tasks: vec![],
            plan,
            status_override: None,
        };
        assert!(!track.all_tasks_resolved());
    }

    // --- DonePending / DoneTraced / BackfillHash tests ---

    #[test]
    fn test_complete_without_hash_yields_done_pending() {
        let mut track = make_track(&["T001"], &["T001"]);
        let t1 = TaskId::try_new("T001").unwrap();
        track.transition_task(&t1, TaskTransition::Start).unwrap();
        track.transition_task(&t1, TaskTransition::Complete { commit_hash: None }).unwrap();
        assert!(matches!(track.tasks()[0].status(), TaskStatus::DonePending));
    }

    #[test]
    fn test_complete_with_hash_yields_done_traced() {
        let mut track = make_track(&["T001"], &["T001"]);
        let t1 = TaskId::try_new("T001").unwrap();
        let hash = CommitHash::try_new("abc1234").unwrap();
        track.transition_task(&t1, TaskTransition::Start).unwrap();
        track.transition_task(&t1, TaskTransition::Complete { commit_hash: Some(hash) }).unwrap();
        assert!(matches!(track.tasks()[0].status(), TaskStatus::DoneTraced { .. }));
    }

    #[test]
    fn test_backfill_hash_on_done_pending_yields_done_traced() {
        let mut track = make_track(&["T001"], &["T001"]);
        let t1 = TaskId::try_new("T001").unwrap();
        let hash = CommitHash::try_new("abc1234").unwrap();
        track.transition_task(&t1, TaskTransition::Start).unwrap();
        track.transition_task(&t1, TaskTransition::Complete { commit_hash: None }).unwrap();
        track
            .transition_task(&t1, TaskTransition::BackfillHash { commit_hash: hash.clone() })
            .unwrap();
        assert_eq!(track.tasks()[0].status(), &TaskStatus::DoneTraced { commit_hash: hash });
    }

    #[test]
    fn test_backfill_hash_on_done_traced_is_rejected() {
        let mut track = make_track(&["T001"], &["T001"]);
        let t1 = TaskId::try_new("T001").unwrap();
        let hash = CommitHash::try_new("abc1234").unwrap();
        track.transition_task(&t1, TaskTransition::Start).unwrap();
        track
            .transition_task(&t1, TaskTransition::Complete { commit_hash: Some(hash.clone()) })
            .unwrap();
        let result = track.transition_task(&t1, TaskTransition::BackfillHash { commit_hash: hash });
        assert!(matches!(
            result,
            Err(DomainError::Transition(TransitionError::InvalidTaskTransition { .. }))
        ));
    }

    #[test]
    fn test_done_pending_reopen_yields_in_progress() {
        let mut track = make_track(&["T001"], &["T001"]);
        let t1 = TaskId::try_new("T001").unwrap();
        track.transition_task(&t1, TaskTransition::Start).unwrap();
        track.transition_task(&t1, TaskTransition::Complete { commit_hash: None }).unwrap();
        track.transition_task(&t1, TaskTransition::Reopen).unwrap();
        assert!(matches!(track.tasks()[0].status(), TaskStatus::InProgress));
    }

    #[test]
    fn test_done_traced_reopen_yields_in_progress() {
        let mut track = make_track(&["T001"], &["T001"]);
        let t1 = TaskId::try_new("T001").unwrap();
        let hash = CommitHash::try_new("abc1234").unwrap();
        track.transition_task(&t1, TaskTransition::Start).unwrap();
        track.transition_task(&t1, TaskTransition::Complete { commit_hash: Some(hash) }).unwrap();
        track.transition_task(&t1, TaskTransition::Reopen).unwrap();
        assert!(matches!(track.tasks()[0].status(), TaskStatus::InProgress));
    }

    #[test]
    fn test_done_pending_is_resolved() {
        let task = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "test",
            TaskStatus::DonePending,
        )
        .unwrap();
        assert!(task.status().is_resolved());
    }

    #[test]
    fn test_done_traced_is_resolved() {
        let hash = CommitHash::try_new("abc1234").unwrap();
        let task = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "test",
            TaskStatus::DoneTraced { commit_hash: hash },
        )
        .unwrap();
        assert!(task.status().is_resolved());
    }

    #[test]
    fn test_done_pending_kind_is_done() {
        assert_eq!(TaskStatus::DonePending.kind(), TaskStatusKind::Done);
    }

    #[test]
    fn test_done_traced_kind_is_done() {
        let hash = CommitHash::try_new("abc1234").unwrap();
        assert_eq!(TaskStatus::DoneTraced { commit_hash: hash }.kind(), TaskStatusKind::Done);
    }

    #[test]
    fn test_backfill_hash_target_kind_is_done() {
        let hash = CommitHash::try_new("abc1234").unwrap();
        let t = TaskTransition::BackfillHash { commit_hash: hash };
        assert_eq!(t.target_kind(), TaskStatusKind::Done);
    }

    #[test]
    fn test_next_task_id_overflow_returns_error() {
        // Create a task with the max u64 value
        let max_id = format!("T{}", u64::MAX);
        let task = TrackTask::new(TaskId::try_new(&max_id).unwrap(), "Max task").unwrap();
        let section =
            PlanSection::new("S1", "Section 1", vec![], vec![TaskId::try_new(&max_id).unwrap()])
                .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        let track = TrackMetadata {
            id: TrackId::try_new("test-track").unwrap(),
            branch: None,
            title: NonEmptyString::try_new("Test").unwrap(),
            tasks: vec![task],
            plan,
            status_override: None,
        };
        let result = track.next_task_id();
        assert!(matches!(result, Err(ValidationError::InvalidTaskId(_))));
    }

    // --- T006: TrackMetadata::status() derivation tests ---

    fn make_track_with_statuses(statuses: &[(&str, TaskStatus)]) -> TrackMetadata {
        let tasks: Vec<TrackTask> = statuses
            .iter()
            .map(|(id, status)| {
                TrackTask::with_status(
                    TaskId::try_new(*id).unwrap(),
                    format!("Task {id}"),
                    status.clone(),
                )
                .unwrap()
            })
            .collect();
        let section_task_ids: Vec<TaskId> =
            statuses.iter().map(|(id, _)| TaskId::try_new(*id).unwrap()).collect();
        let plan = if section_task_ids.is_empty() {
            PlanView::new(vec![], vec![])
        } else {
            let section = PlanSection::new("S1", "Section 1", vec![], section_task_ids).unwrap();
            PlanView::new(vec![], vec![section])
        };
        TrackMetadata::new(
            TrackId::try_new("status-test").unwrap(),
            "Status Test",
            tasks,
            plan,
            None,
        )
        .unwrap()
    }

    fn done_traced(hash: &str) -> TaskStatus {
        TaskStatus::DoneTraced { commit_hash: CommitHash::try_new(hash).unwrap() }
    }

    #[test]
    fn test_status_empty_tasks_is_planned() {
        let track = make_track_with_statuses(&[]);
        assert_eq!(track.status(), TrackStatus::Planned);
    }

    #[test]
    fn test_status_all_todo_is_planned() {
        let track =
            make_track_with_statuses(&[("T001", TaskStatus::Todo), ("T002", TaskStatus::Todo)]);
        assert_eq!(track.status(), TrackStatus::Planned);
    }

    #[test]
    fn test_status_any_in_progress_is_in_progress() {
        let track = make_track_with_statuses(&[
            ("T001", TaskStatus::Todo),
            ("T002", TaskStatus::InProgress),
        ]);
        assert_eq!(track.status(), TrackStatus::InProgress);
    }

    #[test]
    fn test_status_mixed_done_and_todo_is_in_progress() {
        let track = make_track_with_statuses(&[
            ("T001", done_traced("abc1234")),
            ("T002", TaskStatus::Todo),
        ]);
        assert_eq!(track.status(), TrackStatus::InProgress);
    }

    #[test]
    fn test_status_all_done_is_done() {
        let track = make_track_with_statuses(&[
            ("T001", done_traced("abc1234")),
            ("T002", done_traced("def5678")),
        ]);
        assert_eq!(track.status(), TrackStatus::Done);
    }

    #[test]
    fn test_status_all_skipped_is_done() {
        let track = make_track_with_statuses(&[
            ("T001", TaskStatus::Skipped),
            ("T002", TaskStatus::Skipped),
        ]);
        assert_eq!(track.status(), TrackStatus::Done);
    }

    #[test]
    fn test_status_mixed_done_and_skipped_is_done() {
        let track = make_track_with_statuses(&[
            ("T001", done_traced("abc1234")),
            ("T002", TaskStatus::Skipped),
        ]);
        assert_eq!(track.status(), TrackStatus::Done);
    }

    #[test]
    fn test_status_mixed_skipped_and_todo_is_in_progress() {
        // Not all "todo" (so not Planned) and not all resolved (so not Done);
        // the derivation falls through to InProgress.
        let track =
            make_track_with_statuses(&[("T001", TaskStatus::Skipped), ("T002", TaskStatus::Todo)]);
        assert_eq!(track.status(), TrackStatus::InProgress);
    }

    #[test]
    fn test_status_override_blocked_wins_over_derived() {
        let mut track = make_track_with_statuses(&[
            ("T001", TaskStatus::InProgress),
            ("T002", TaskStatus::Todo),
        ]);
        track
            .set_status_override(Some(StatusOverride::blocked("waiting on upstream").unwrap()))
            .unwrap();
        assert_eq!(track.status(), TrackStatus::Blocked);
    }

    #[test]
    fn test_status_override_cancelled_wins_over_derived() {
        let mut track =
            make_track_with_statuses(&[("T001", TaskStatus::Todo), ("T002", TaskStatus::Todo)]);
        track
            .set_status_override(Some(StatusOverride::cancelled("de-prioritized").unwrap()))
            .unwrap();
        assert_eq!(track.status(), TrackStatus::Cancelled);
    }
}
