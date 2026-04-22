use thiserror::Error;

use crate::{TaskStatusKind, TrackStatus};

/// Top-level domain error encompassing validation and transition errors.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

/// Validation errors for domain invariants.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    #[error("string must not be empty")]
    EmptyString,
    #[error("track id '{0}' must be a lowercase slug")]
    InvalidTrackId(String),
    #[error("task id '{0}' must match the pattern T<digits>")]
    InvalidTaskId(String),
    #[error("commit hash '{0}' must be 7 to 40 lowercase hex characters")]
    InvalidCommitHash(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("track branch '{0}' must match the pattern track/<slug>")]
    InvalidTrackBranch(String),
    #[error("branch '{branch}' slug does not match track id '{id}'")]
    BranchIdMismatch { id: String, branch: String },
    #[error(
        "status_override kind '{override_kind}' requires status '{required}', \
         but status is '{actual}'"
    )]
    StatusOverrideMismatch { override_kind: String, required: String, actual: String },
    #[error("track title must not be empty")]
    EmptyTrackTitle,
    #[error("task description must not be empty")]
    EmptyTaskDescription,
    #[error("plan section id must not be empty")]
    EmptyPlanSectionId,
    #[error("plan section title must not be empty")]
    EmptyPlanSectionTitle,
    #[error("duplicate task id '{0}'")]
    DuplicateTaskId(String),
    #[error("duplicate plan section id '{0}'")]
    DuplicatePlanSectionId(String),
    #[error("plan references unknown task '{0}'")]
    UnknownTaskReference(String),
    #[error("task '{0}' is referenced more than once in the plan")]
    DuplicateTaskReference(String),
    #[error("task '{0}' is not referenced by any plan section")]
    UnreferencedTask(String),
    #[error("status override '{0}' is incompatible with all tasks resolved")]
    OverrideIncompatibleWithResolvedTasks(TrackStatus),
    #[error("track '{track_id}' is not planning-only; current status is '{status}'")]
    TrackActivationRequiresPlanningOnly { track_id: String, status: TrackStatus },
    #[error(
        "track '{track_id}' requires schema_version 3 for activation; current schema_version is {schema_version}"
    )]
    TrackActivationRequiresSchemaV3 { track_id: String, schema_version: u32 },
    #[error("track '{track_id}' is already materialized on branch '{branch}'")]
    TrackAlreadyMaterialized { track_id: String, branch: String },
    #[error("unsupported target status: {0}")]
    UnsupportedTargetStatus(String),
    #[error("section '{0}' not found")]
    SectionNotFound(String),
    #[error("no sections available to add task to")]
    NoSectionsAvailable,
    #[error(
        "task '{task_id}' description was mutated; task descriptions are immutable once created"
    )]
    TaskDescriptionMutated { task_id: String },
    #[error("task '{task_id}' was removed; existing tasks cannot be deleted via save")]
    TaskRemoved { task_id: String },
    #[error("duplicate spec element id '{0}' — ids must be unique across all sections")]
    DuplicateElementId(String),
    #[error(
        "layer id '{0}' must be a non-empty ASCII identifier starting with a letter \
         (allowed: letters, digits, `_`, `-`)"
    )]
    InvalidLayerId(String),
    #[error(
        "spec element id '{0}' must match the pattern <UPPER>{{2,}}-<digits>+ \
         (e.g. IN-01, AC-02, CO-03)"
    )]
    InvalidSpecElementId(String),
    #[error("ADR anchor must not be empty")]
    EmptyAdrAnchor,
    #[error("convention anchor must not be empty")]
    EmptyConventionAnchor,
    #[error("content hash '{0}' must be 64 lowercase hex characters (SHA-256)")]
    InvalidContentHash(String),
    #[error("informal ground summary must not be empty")]
    EmptyInformalGroundSummary,
    #[error("informal ground summary must be a single line (no line breaks)")]
    MultiLineInformalGroundSummary,
}

/// Errors from invalid task state transitions.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransitionError {
    #[error("task '{task_id}' not found")]
    TaskNotFound { task_id: String },
    #[error("invalid task transition for '{task_id}': {from} -> {to}")]
    InvalidTaskTransition { task_id: String, from: TaskStatusKind, to: TaskStatusKind },
}

/// Errors from repository operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepositoryError {
    #[error("track '{0}' was not found")]
    TrackNotFound(String),
    #[error("repository error: {0}")]
    Message(String),
}

/// Error type for `TrackReader` port operations.
#[derive(Debug, Error)]
pub enum TrackReadError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// Error type for `TrackWriter` port operations.
#[derive(Debug, Error)]
pub enum TrackWriteError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl From<TrackReadError> for TrackWriteError {
    fn from(e: TrackReadError) -> Self {
        match e {
            TrackReadError::Repository(repo_err) => TrackWriteError::Repository(repo_err),
        }
    }
}

/// Error type for `WorktreeReader` port operations.
#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("worktree status failed: {0}")]
    StatusFailed(String),
}
