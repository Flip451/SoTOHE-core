//! Rejection of a batch plan by the invariants decidable from the plan file alone.

use thiserror::Error;

use crate::TaskId;
use crate::review_v2::ScopeName;

use super::batch::BatchId;

// ── BatchPlanValidationError ──────────────────────────────────────────────────

/// Rejection of a batch plan by the constructors that own its file-internal
/// invariants (IN-05 / AC-04).
///
/// Every variant is a refusal: the plan is rejected rather than partially
/// accepted, so no malformed declaration reaches a gate as a skipped entry.
/// Invariants that need the resolved per-scope ceiling are not decidable here —
/// their owner is the Phase 3 gate, which receives the scope configuration.
#[derive(Debug, Error)]
pub enum BatchPlanValidationError {
    /// An indivisibility justification was declared with no text.
    #[error("indivisibility justification must not be empty")]
    EmptyJustification,
    /// A batch was declared with an empty identifier.
    #[error("batch id must not be empty")]
    EmptyBatchId,
    /// A batch was declared with no member task.
    #[error("batch '{}' declares no member task", .batch_id.as_str())]
    EmptyBatch {
        /// The batch that declared no member.
        batch_id: BatchId,
    },
    /// The plan declares more than one estimate for the same task.
    #[error("task '{task_id}' declares more than one estimate")]
    DuplicateTaskEstimate {
        /// The task estimated more than once.
        task_id: TaskId,
    },
    /// A task declares two competing figures for one review scope.
    #[error("task '{task_id}' declares more than one estimate for scope '{scope}'")]
    DuplicateScopeEstimate {
        /// The task holding the repeated scope.
        task_id: TaskId,
        /// The scope estimated more than once.
        scope: ScopeName,
    },
    /// A task declares an estimate that names no scope at all.
    ///
    /// Distinct from [`BatchPlanValidationError::MissingTaskEstimate`], which is
    /// an absent entry rather than a present but empty one.
    #[error("task '{task_id}' declares an estimate naming no scope")]
    EmptyScopeEstimates {
        /// The task whose estimate names no scope.
        task_id: TaskId,
    },
    /// The plan declares the same batch identifier more than once.
    #[error("batch id '{}' is declared more than once", .batch_id.as_str())]
    DuplicateBatchId {
        /// The repeated batch identifier.
        batch_id: BatchId,
    },
    /// A batch member has no declared estimate.
    #[error("task '{task_id}' is a batch member without a declared estimate")]
    MissingTaskEstimate {
        /// The task whose estimate is missing.
        task_id: TaskId,
    },
    /// An estimated task belongs to no batch.
    #[error("task '{task_id}' belongs to no batch")]
    UnassignedTask {
        /// The task no batch claims.
        task_id: TaskId,
    },
    /// A task is claimed by more than one batch.
    #[error(
        "task '{task_id}' belongs to more than one batch: {}",
        render_batch_ids(.batch_ids)
    )]
    DuplicateBatchMembership {
        /// The task claimed more than once.
        task_id: TaskId,
        /// Every batch claiming the task, in declaration order.
        batch_ids: Vec<BatchId>,
    },
}

/// Renders the offending batch identifiers of a duplicate-membership rejection.
fn render_batch_ids(batch_ids: &[BatchId]) -> String {
    batch_ids.iter().map(BatchId::as_str).collect::<Vec<_>>().join(", ")
}
