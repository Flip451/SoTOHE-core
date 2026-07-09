use thiserror::Error;

use crate::tddd::test_obligation::ids::DiagnosticMessage;
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
    #[error("track id '{}' must be a lowercase slug", .0.as_str())]
    InvalidTrackId(DiagnosticMessage),
    #[error("task id '{}' must match the pattern T<digits>", .0.as_str())]
    InvalidTaskId(DiagnosticMessage),
    #[error("commit hash '{}' must be 7 to 40 lowercase hex characters", .0.as_str())]
    InvalidCommitHash(DiagnosticMessage),
    #[error("invalid timestamp: {}", .0.as_str())]
    InvalidTimestamp(DiagnosticMessage),
    #[error("track branch '{}' must match the pattern track/<slug>", .0.as_str())]
    InvalidTrackBranch(DiagnosticMessage),
    #[error("branch '{}' slug does not match track id '{}'", .branch.as_str(), .id.as_str())]
    BranchIdMismatch { id: DiagnosticMessage, branch: DiagnosticMessage },
    #[error(
        "status_override kind '{}' requires status '{}', but status is '{}'",
        .override_kind.as_str(), .required.as_str(), .actual.as_str()
    )]
    StatusOverrideMismatch {
        override_kind: DiagnosticMessage,
        required: DiagnosticMessage,
        actual: DiagnosticMessage,
    },
    #[error("track title must not be empty")]
    EmptyTrackTitle,
    #[error("task description must not be empty")]
    EmptyTaskDescription,
    #[error("plan section id must not be empty")]
    EmptyPlanSectionId,
    #[error("plan section title must not be empty")]
    EmptyPlanSectionTitle,
    #[error("duplicate task id '{}'", .0.as_str())]
    DuplicateTaskId(DiagnosticMessage),
    #[error("duplicate plan section id '{}'", .0.as_str())]
    DuplicatePlanSectionId(DiagnosticMessage),
    #[error("plan references unknown task '{}'", .0.as_str())]
    UnknownTaskReference(DiagnosticMessage),
    #[error("task '{}' is referenced more than once in the plan", .0.as_str())]
    DuplicateTaskReference(DiagnosticMessage),
    #[error("task '{}' is not referenced by any plan section", .0.as_str())]
    UnreferencedTask(DiagnosticMessage),
    #[error("status override '{0}' is incompatible with all tasks resolved")]
    OverrideIncompatibleWithResolvedTasks(TrackStatus),
    #[error("track '{}' is not planning-only; current status is '{}'", .track_id.as_str(), .status)]
    TrackActivationRequiresPlanningOnly { track_id: DiagnosticMessage, status: TrackStatus },
    #[error(
        "track '{}' requires schema_version 3 for activation; current schema_version is {}",
        .track_id.as_str(), .schema_version
    )]
    TrackActivationRequiresSchemaV3 { track_id: DiagnosticMessage, schema_version: u32 },
    #[error("track '{}' is already materialized on branch '{}'", .track_id.as_str(), .branch.as_str())]
    TrackAlreadyMaterialized { track_id: DiagnosticMessage, branch: DiagnosticMessage },
    #[error("unsupported target status: {}", .0.as_str())]
    UnsupportedTargetStatus(DiagnosticMessage),
    #[error("section '{}' not found", .0.as_str())]
    SectionNotFound(DiagnosticMessage),
    #[error("no sections available to add task to")]
    NoSectionsAvailable,
    #[error(
        "task '{}' description was mutated; task descriptions are immutable once created",
        .task_id.as_str()
    )]
    TaskDescriptionMutated { task_id: DiagnosticMessage },
    #[error("task '{}' was removed; existing tasks cannot be deleted via save", .task_id.as_str())]
    TaskRemoved { task_id: DiagnosticMessage },
    #[error("duplicate spec element id '{}' — ids must be unique across all sections", .0.as_str())]
    DuplicateElementId(DiagnosticMessage),
    #[error(
        "layer id '{}' must be a non-empty ASCII identifier starting with a letter \
         (allowed: letters, digits, `_`, `-`)",
        .0.as_str()
    )]
    InvalidLayerId(DiagnosticMessage),
    #[error(
        "spec element id '{}' must match the pattern <UPPER>{{2,}}-<digits>+ \
         (e.g. IN-01, AC-02, CO-03)",
        .0.as_str()
    )]
    InvalidSpecElementId(DiagnosticMessage),
    #[error("ADR anchor must not be empty")]
    EmptyAdrAnchor,
    #[error("convention anchor must not be empty")]
    EmptyConventionAnchor,
    #[error("content hash '{}' must be 64 lowercase hex characters (SHA-256)", .0.as_str())]
    InvalidContentHash(DiagnosticMessage),
    #[error("informal ground summary must not be empty")]
    EmptyInformalGroundSummary,
    #[error("informal ground summary must be a single line (no line breaks)")]
    MultiLineInformalGroundSummary,
    #[error("decision ground reference must not be empty or whitespace-only")]
    EmptyDecisionGroundRef,
    #[error("test-obligation minimum '{0}' must be at least 1")]
    InvalidObligationMinimum(usize),
    #[error("detection rate '{0}' must be a percentage in 0..=100")]
    InvalidDetectionRate(u8),
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
