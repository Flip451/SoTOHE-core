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

/// Identity-only aggregate for a track (metadata.json SSoT after ADR 2026-04-19-1242 §D1.4).
///
/// Retains only: `id`, `branch`, `title`, `status` (explicitly stored),
/// `created_at`, `updated_at` (handled at DTO layer in `DocumentMeta`).
///
/// `tasks` and `plan` have moved to `ImplPlanDocument` (`impl-plan.json`).
/// Status is an explicit stored value, no longer task-derived.
/// `status_override` is retained as an optional sub-field that feeds into
/// the stored status on save; codec layer (T005) stores it separately.
///
/// Branch validation: when a branch is present it must begin with `"track/"` and
/// the slug after the prefix must match the track id exactly.
/// Constructor validates: non-empty title, valid id (caller responsibility),
/// branch format and branch-id consistency if provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMetadata {
    id: TrackId,
    branch: Option<TrackBranch>,
    title: NonEmptyString,
    /// Explicit stored status. No longer derived from tasks.
    status: TrackStatus,
    /// Optional override sub-field preserved for callers that still use
    /// status_override semantics (e.g., `track_phase.rs` Blocked/Cancelled display).
    /// The codec stores this when present.
    status_override: Option<StatusOverride>,
}

impl TrackMetadata {
    /// Creates a new identity-only `TrackMetadata`.
    ///
    /// # Errors
    /// Returns `DomainError::Validation(ValidationError::EmptyTrackTitle)` if title is empty.
    pub fn new(
        id: TrackId,
        title: impl Into<String>,
        status: TrackStatus,
        status_override: Option<StatusOverride>,
    ) -> Result<Self, DomainError> {
        Self::with_branch(id, None, title, status, status_override)
    }

    /// Creates a new `TrackMetadata` with an optional branch field.
    ///
    /// When `branch` is present, its slug (the portion after `"track/"`) must
    /// match the track `id` exactly. This ensures that branch-based readers and
    /// activation logic cannot be misrouted.
    ///
    /// When `status_override` is `Some`, the `status` must equal the override's
    /// canonical status (e.g., `StatusOverride::blocked(...)` requires
    /// `TrackStatus::Blocked`).
    ///
    /// # Errors
    /// - `DomainError::Validation(ValidationError::EmptyTrackTitle)` if title is empty.
    /// - `DomainError::Validation(ValidationError::BranchIdMismatch)` if the branch
    ///   slug does not match the track id.
    /// - `DomainError::Validation(ValidationError::StatusOverrideMismatch)` if
    ///   `status_override` is present but `status` does not match.
    pub fn with_branch(
        id: TrackId,
        branch: Option<TrackBranch>,
        title: impl Into<String>,
        status: TrackStatus,
        status_override: Option<StatusOverride>,
    ) -> Result<Self, DomainError> {
        if let Some(ref b) = branch {
            let expected_prefix = format!("track/{}", id.as_ref());
            if b.as_ref() != expected_prefix {
                return Err(DomainError::Validation(ValidationError::BranchIdMismatch {
                    id: id.to_string(),
                    branch: b.to_string(),
                }));
            }
        }
        if let Some(ref o) = status_override {
            let required = o.track_status();
            if status != required {
                return Err(DomainError::Validation(ValidationError::StatusOverrideMismatch {
                    override_kind: o.kind().to_string(),
                    required: required.to_string(),
                    actual: status.to_string(),
                }));
            }
        }
        let title = NonEmptyString::try_new(title).map_err(|_| ValidationError::EmptyTrackTitle)?;
        Ok(Self { id, branch, title, status, status_override })
    }

    #[must_use]
    pub fn id(&self) -> &TrackId {
        &self.id
    }

    #[must_use]
    pub fn branch(&self) -> Option<&TrackBranch> {
        self.branch.as_ref()
    }

    /// Sets or clears the branch field, enforcing branch-id consistency.
    ///
    /// When `branch` is `Some`, its slug (the portion after `"track/"`) must
    /// match the track id exactly.
    ///
    /// # Errors
    /// Returns `DomainError::Validation(ValidationError::BranchIdMismatch)` if the
    /// branch slug does not match the track id.
    pub fn set_branch(&mut self, branch: Option<TrackBranch>) -> Result<(), DomainError> {
        if let Some(ref b) = branch {
            let expected_prefix = format!("track/{}", self.id.as_ref());
            if b.as_ref() != expected_prefix {
                return Err(DomainError::Validation(ValidationError::BranchIdMismatch {
                    id: self.id.to_string(),
                    branch: b.to_string(),
                }));
            }
        }
        self.branch = branch;
        Ok(())
    }

    #[must_use]
    pub fn title(&self) -> &str {
        self.title.as_ref()
    }

    /// Returns the explicitly stored status.
    #[must_use]
    pub fn status(&self) -> TrackStatus {
        self.status
    }

    /// Sets the explicit stored status.
    ///
    /// If a `status_override` is currently set and the new `status` does not
    /// match the override's canonical status, the override is automatically
    /// cleared to preserve the status/override coherence invariant.
    pub fn set_status(&mut self, status: TrackStatus) {
        if let Some(ref o) = self.status_override {
            if status != o.track_status() {
                self.status_override = None;
            }
        }
        self.status = status;
    }

    #[must_use]
    pub fn status_override(&self) -> Option<&StatusOverride> {
        self.status_override.as_ref()
    }

    /// Sets or clears the status override sub-field.
    ///
    /// When `status_override` is `Some`, the `status` is automatically synced
    /// to the override's canonical status (e.g., setting a `Blocked` override
    /// also sets `status = TrackStatus::Blocked`).
    ///
    /// When `status_override` is `None`, the `status` is left unchanged;
    /// the caller is responsible for setting an appropriate status after clearing.
    pub fn set_status_override(&mut self, status_override: Option<StatusOverride>) {
        if let Some(ref o) = status_override {
            self.status = o.track_status();
        }
        self.status_override = status_override;
    }
}

/// Re-exported for `impl_plan.rs` which still needs plan validation.
///
/// Remains public within the crate for `impl_plan::ImplPlanDocument::new`.
pub(crate) fn validate_plan_invariants(
    tasks: &[TrackTask],
    plan: &PlanView,
) -> Result<(), ValidationError> {
    use std::collections::{HashMap, HashSet};

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

    // --- Identity-only TrackMetadata construction ---

    #[test]
    fn test_track_metadata_new_with_valid_title_succeeds() {
        let track = TrackMetadata::new(
            TrackId::try_new("my-track").unwrap(),
            "My Track",
            TrackStatus::Planned,
            None,
        )
        .unwrap();
        assert_eq!(track.id().as_ref(), "my-track");
        assert_eq!(track.title(), "My Track");
        assert_eq!(track.status(), TrackStatus::Planned);
        assert!(track.branch().is_none());
        assert!(track.status_override().is_none());
    }

    #[test]
    fn test_track_metadata_new_with_empty_title_returns_error() {
        let result = TrackMetadata::new(
            TrackId::try_new("my-track").unwrap(),
            "",
            TrackStatus::Planned,
            None,
        );
        assert!(matches!(result, Err(DomainError::Validation(ValidationError::EmptyTrackTitle))));
    }

    #[test]
    fn test_track_metadata_with_branch_stores_branch() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track").unwrap(),
            Some(TrackBranch::try_new("track/my-track").unwrap()),
            "My Track",
            TrackStatus::InProgress,
            None,
        )
        .unwrap();
        assert_eq!(track.branch().unwrap().as_ref(), "track/my-track");
        assert_eq!(track.status(), TrackStatus::InProgress);
    }

    #[test]
    fn test_track_metadata_with_branch_rejects_id_mismatch() {
        let result = TrackMetadata::with_branch(
            TrackId::try_new("my-track").unwrap(),
            Some(TrackBranch::try_new("track/other-track").unwrap()),
            "My Track",
            TrackStatus::Planned,
            None,
        );
        assert!(
            matches!(
                result,
                Err(DomainError::Validation(ValidationError::BranchIdMismatch { .. }))
            ),
            "expected BranchIdMismatch but got: {result:?}"
        );
    }

    #[test]
    fn test_track_metadata_without_branch_returns_none() {
        let track = TrackMetadata::new(
            TrackId::try_new("my-track").unwrap(),
            "My Track",
            TrackStatus::Planned,
            None,
        )
        .unwrap();
        assert!(track.branch().is_none());
    }

    #[test]
    fn test_track_metadata_set_branch_updates_branch() {
        let mut track = TrackMetadata::new(
            TrackId::try_new("my-track").unwrap(),
            "My Track",
            TrackStatus::Planned,
            None,
        )
        .unwrap();
        assert!(track.branch().is_none());
        track.set_branch(Some(TrackBranch::try_new("track/my-track").unwrap())).unwrap();
        assert_eq!(track.branch().unwrap().as_ref(), "track/my-track");
    }

    #[test]
    fn test_track_metadata_set_status_updates_status() {
        let mut track = TrackMetadata::new(
            TrackId::try_new("status-track").unwrap(),
            "Status Track",
            TrackStatus::Planned,
            None,
        )
        .unwrap();
        track.set_status(TrackStatus::Done);
        assert_eq!(track.status(), TrackStatus::Done);
    }

    #[test]
    fn test_track_metadata_stores_blocked_status_directly() {
        let track = TrackMetadata::new(
            TrackId::try_new("blocked-track").unwrap(),
            "Blocked Track",
            TrackStatus::Blocked,
            Some(StatusOverride::blocked("waiting on review").unwrap()),
        )
        .unwrap();
        assert_eq!(track.status(), TrackStatus::Blocked);
        assert!(track.status_override().is_some());
    }

    #[test]
    fn test_track_metadata_stores_archived_status() {
        let track = TrackMetadata::new(
            TrackId::try_new("archived-track").unwrap(),
            "Archived Track",
            TrackStatus::Archived,
            None,
        )
        .unwrap();
        assert_eq!(track.status(), TrackStatus::Archived);
    }

    #[test]
    fn test_track_metadata_set_status_override_stores_override_and_syncs_status() {
        let mut track = TrackMetadata::new(
            TrackId::try_new("my-track").unwrap(),
            "My Track",
            TrackStatus::InProgress,
            None,
        )
        .unwrap();
        track.set_status_override(Some(StatusOverride::blocked("dep issue").unwrap()));
        assert!(track.status_override().is_some());
        // set_status_override auto-syncs status to the override's canonical status.
        assert_eq!(track.status(), TrackStatus::Blocked);
    }

    #[test]
    fn test_track_metadata_set_status_clears_incompatible_override() {
        let mut track = TrackMetadata::new(
            TrackId::try_new("my-track").unwrap(),
            "My Track",
            TrackStatus::Blocked,
            Some(StatusOverride::blocked("waiting").unwrap()),
        )
        .unwrap();
        assert!(track.status_override().is_some());
        // Setting status to an incompatible value auto-clears the override.
        track.set_status(TrackStatus::Planned);
        assert_eq!(track.status(), TrackStatus::Planned);
        assert!(track.status_override().is_none());
    }

    #[test]
    fn test_track_metadata_constructor_rejects_status_override_mismatch() {
        let result = TrackMetadata::new(
            TrackId::try_new("my-track").unwrap(),
            "My Track",
            TrackStatus::Planned, // inconsistent: override says Blocked
            Some(StatusOverride::blocked("dep issue").unwrap()),
        );
        assert!(
            matches!(
                result,
                Err(DomainError::Validation(ValidationError::StatusOverrideMismatch { .. }))
            ),
            "expected StatusOverrideMismatch but got: {result:?}"
        );
    }

    // --- TaskStatus and TaskTransition tests (standalone, no TrackMetadata dependency) ---

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

    // --- TrackStatus display ---

    #[test]
    fn test_track_status_planned_displays_correctly() {
        assert_eq!(TrackStatus::Planned.to_string(), "planned");
    }

    #[test]
    fn test_track_status_archived_displays_correctly() {
        assert_eq!(TrackStatus::Archived.to_string(), "archived");
    }

    #[test]
    fn test_track_status_all_variants_display() {
        assert_eq!(TrackStatus::InProgress.to_string(), "in_progress");
        assert_eq!(TrackStatus::Done.to_string(), "done");
        assert_eq!(TrackStatus::Blocked.to_string(), "blocked");
        assert_eq!(TrackStatus::Cancelled.to_string(), "cancelled");
    }
}
