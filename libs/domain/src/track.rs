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
