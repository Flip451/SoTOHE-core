use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::{
    CommitHash, DomainError, PlanView, TaskId, TrackBranch, TrackId, TransitionError,
    ValidationError,
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

/// Status of a task. `Done` carries an optional commit hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done { commit_hash: Option<CommitHash> },
    Skipped,
}

impl TaskStatus {
    /// Returns the discriminant kind of this status.
    #[must_use]
    pub fn kind(&self) -> TaskStatusKind {
        match self {
            Self::Todo => TaskStatusKind::Todo,
            Self::InProgress => TaskStatusKind::InProgress,
            Self::Done { .. } => TaskStatusKind::Done,
            Self::Skipped => TaskStatusKind::Skipped,
        }
    }

    /// Returns `true` if the task is in a terminal state (Done or Skipped).
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Done { .. } | Self::Skipped)
    }
}

/// Command enum representing valid task state transition requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskTransition {
    Start,
    Complete { commit_hash: Option<CommitHash> },
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
            Self::Complete { .. } => TaskStatusKind::Done,
            Self::ResetToTodo => TaskStatusKind::Todo,
            Self::Skip => TaskStatusKind::Skipped,
            Self::Reopen => TaskStatusKind::InProgress,
        }
    }
}

/// Manual override for track status (Blocked or Cancelled with reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusOverride {
    Blocked { reason: String },
    Cancelled { reason: String },
}

impl StatusOverride {
    #[must_use]
    pub fn blocked(reason: impl Into<String>) -> Self {
        Self::Blocked { reason: reason.into() }
    }

    #[must_use]
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self::Cancelled { reason: reason.into() }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            Self::Blocked { reason } | Self::Cancelled { reason } => reason,
        }
    }

    #[must_use]
    pub fn track_status(&self) -> TrackStatus {
        match self {
            Self::Blocked { .. } => TrackStatus::Blocked,
            Self::Cancelled { .. } => TrackStatus::Cancelled,
        }
    }
}

/// A single task within a track, with its own state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackTask {
    id: TaskId,
    description: String,
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
        let description = description.into();
        if description.trim().is_empty() {
            return Err(ValidationError::EmptyTaskDescription);
        }

        Ok(Self { id, description, status })
    }

    #[must_use]
    pub fn id(&self) -> &TaskId {
        &self.id
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
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
            (TaskStatus::InProgress, TaskTransition::Complete { commit_hash }) => {
                TaskStatus::Done { commit_hash }
            }
            (TaskStatus::InProgress, TaskTransition::ResetToTodo) => TaskStatus::Todo,
            (TaskStatus::InProgress, TaskTransition::Skip) => TaskStatus::Skipped,
            (TaskStatus::Done { .. }, TaskTransition::Reopen) => TaskStatus::InProgress,
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
    title: String,
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
        let title = title.into();
        if title.trim().is_empty() {
            return Err(ValidationError::EmptyTrackTitle.into());
        }

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
        &self.title
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
            .filter_map(|t| t.id().as_str().strip_prefix('T').and_then(|d| d.parse::<u64>().ok()))
            .max()
            .unwrap_or(0);
        let next = max_num.checked_add(1).ok_or_else(|| {
            ValidationError::InvalidTaskId(
                "task ID overflow: max T-number exceeded u64".to_string(),
            )
        })?;
        TaskId::new(format!("T{next:03}"))
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
    /// Compares task descriptions between `self` (the new state) and `previous`
    /// (the last saved state). Tasks that exist in both (matched by ID) must
    /// have identical descriptions. New tasks (IDs not in `previous`) are allowed.
    ///
    /// # Errors
    /// Returns `ValidationError::TaskDescriptionMutated` if any existing task's
    /// description has been changed.
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

fn validate_plan_invariants(tasks: &[TrackTask], plan: &PlanView) -> Result<(), ValidationError> {
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
            .map(|id| TrackTask::new(TaskId::new(*id).unwrap(), format!("Task {id}")).unwrap())
            .collect();
        let section = PlanSection::new(
            "S1",
            "Section 1",
            vec![],
            section_task_ids.iter().map(|id| TaskId::new(*id).unwrap()).collect(),
        )
        .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        TrackMetadata::new(TrackId::new("test-track").unwrap(), "Test Track", tasks, plan, None)
            .unwrap()
    }

    #[test]
    fn test_next_task_id_with_no_tasks_returns_t001() {
        let plan = PlanView::new(vec![], vec![]);
        let track = TrackMetadata {
            id: TrackId::new("test-track").unwrap(),
            branch: None,
            title: "Test".to_string(),
            tasks: vec![],
            plan,
            status_override: None,
        };
        assert_eq!(track.next_task_id().unwrap().as_str(), "T001");
    }

    #[test]
    fn test_next_task_id_with_existing_tasks_returns_next() {
        let track = make_track(&["T001", "T002", "T003"], &["T001", "T002", "T003"]);
        assert_eq!(track.next_task_id().unwrap().as_str(), "T004");
    }

    #[test]
    fn test_next_task_id_with_gaps_uses_max() {
        let track = make_track(&["T001", "T010"], &["T001", "T010"]);
        assert_eq!(track.next_task_id().unwrap().as_str(), "T011");
    }

    #[test]
    fn test_add_task_appends_to_first_section_by_default() {
        let mut track = make_track(&["T001"], &["T001"]);
        let new_id = track.add_task("New task", None, None).unwrap();
        assert_eq!(new_id.as_str(), "T002");
        assert_eq!(track.tasks().len(), 2);
        assert_eq!(track.tasks()[1].description(), "New task");
        assert_eq!(track.plan().sections()[0].task_ids().len(), 2);
        assert_eq!(track.plan().sections()[0].task_ids()[1], new_id);
    }

    #[test]
    fn test_add_task_inserts_after_specified_task() {
        let mut track = make_track(&["T001", "T002"], &["T001", "T002"]);
        let after = TaskId::new("T001").unwrap();
        let new_id = track.add_task("Inserted task", None, Some(&after)).unwrap();
        assert_eq!(new_id.as_str(), "T003");
        let section_ids: Vec<&str> =
            track.plan().sections()[0].task_ids().iter().map(|id| id.as_str()).collect();
        assert_eq!(section_ids, vec!["T001", "T003", "T002"]);
    }

    #[test]
    fn test_add_task_with_unknown_after_appends_to_end() {
        let mut track = make_track(&["T001"], &["T001"]);
        let unknown = TaskId::new("T999").unwrap();
        let new_id = track.add_task("Appended task", None, Some(&unknown)).unwrap();
        let section_ids: Vec<&str> =
            track.plan().sections()[0].task_ids().iter().map(|id| id.as_str()).collect();
        assert_eq!(section_ids, vec!["T001", new_id.as_str()]);
    }

    #[test]
    fn test_add_task_to_named_section() {
        let tasks = vec![TrackTask::new(TaskId::new("T001").unwrap(), "Task 1").unwrap()];
        let s1 = PlanSection::new("S1", "Section 1", vec![], vec![TaskId::new("T001").unwrap()])
            .unwrap();
        let s2 = PlanSection::new("S2", "Section 2", vec![], vec![]).unwrap();
        let plan = PlanView::new(vec![], vec![s1, s2]);
        let mut track = TrackMetadata {
            id: TrackId::new("test-track").unwrap(),
            branch: None,
            title: "Test".to_string(),
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
            .map(|id| TrackTask::new(TaskId::new(*id).unwrap(), format!("Task {id}")).unwrap())
            .collect();
        let section_ids: Vec<TaskId> =
            existing.iter().map(|id| TaskId::new(*id).unwrap()).collect();
        let sections = if existing.is_empty() {
            vec![]
        } else {
            vec![PlanSection::new("S1", "Section 1", vec![], section_ids).unwrap()]
        };
        let plan = PlanView::new(vec![], sections);
        let track = TrackMetadata {
            id: TrackId::new("test-track").unwrap(),
            branch: None,
            title: "Test".to_string(),
            tasks,
            plan,
            status_override: None,
        };
        assert_eq!(track.next_task_id().unwrap().as_str(), expected);
    }

    #[test]
    fn test_add_task_with_no_sections_returns_error() {
        let plan = PlanView::new(vec![], vec![]);
        let mut track = TrackMetadata {
            id: TrackId::new("test-track").unwrap(),
            branch: None,
            title: "Test".to_string(),
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

        let t1 = TrackTask::new(TaskId::new("T001").unwrap(), "CHANGED description").unwrap();
        let t2 = TrackTask::new(TaskId::new("T002").unwrap(), "Task T002").unwrap();
        let section = PlanSection::new(
            "S1",
            "Section 1",
            vec![],
            vec![TaskId::new("T001").unwrap(), TaskId::new("T002").unwrap()],
        )
        .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        let modified = TrackMetadata::new(
            TrackId::new("test-track").unwrap(),
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

        let t1 = TrackTask::new(TaskId::new("T001").unwrap(), "Task T001").unwrap();
        let t2 = TrackTask::new(TaskId::new("T002").unwrap(), "Brand new task").unwrap();
        let section = PlanSection::new(
            "S1",
            "Section 1",
            vec![],
            vec![TaskId::new("T001").unwrap(), TaskId::new("T002").unwrap()],
        )
        .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        let updated = TrackMetadata::new(
            TrackId::new("test-track").unwrap(),
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
    fn test_next_task_id_overflow_returns_error() {
        // Create a task with the max u64 value
        let max_id = format!("T{}", u64::MAX);
        let task = TrackTask::new(TaskId::new(&max_id).unwrap(), "Max task").unwrap();
        let section =
            PlanSection::new("S1", "Section 1", vec![], vec![TaskId::new(&max_id).unwrap()])
                .unwrap();
        let plan = PlanView::new(vec![], vec![section]);
        let track = TrackMetadata {
            id: TrackId::new("test-track").unwrap(),
            branch: None,
            title: "Test".to_string(),
            tasks: vec![task],
            plan,
            status_override: None,
        };
        let result = track.next_task_id();
        assert!(matches!(result, Err(ValidationError::InvalidTaskId(_))));
    }
}
