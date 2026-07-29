//! One declared batch: its identity and its member task ids.

use crate::{NonEmptyString, TaskId};

use super::error::BatchPlanValidationError;

// ── BatchId ───────────────────────────────────────────────────────────────────

/// Identifier of one declared batch (IN-03 / AC-03).
///
/// Batch membership is attributed per identifier, so the identifier has to be
/// present: an empty one is rejected at construction. Surrounding whitespace is
/// trimmed before the non-empty check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchId(NonEmptyString);

impl BatchId {
    /// Validates `value` as a batch identifier.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPlanValidationError::EmptyBatchId`] when `value` is empty
    /// or whitespace-only.
    pub fn try_new(value: &str) -> Result<BatchId, BatchPlanValidationError> {
        NonEmptyString::try_new(value)
            .map(BatchId)
            .map_err(|_| BatchPlanValidationError::EmptyBatchId)
    }

    /// Returns the identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

// ── BatchDeclaration ──────────────────────────────────────────────────────────

/// One declared batch: an identifier plus its ordered member task ids
/// (IN-03 / AC-03 / CN-01).
///
/// The declaration carries no line figure of its own — a batch's per-scope
/// total is derived from its members' estimates — so no declared total can
/// drift away from the members it summarises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchDeclaration {
    id: BatchId,
    task_ids: Vec<TaskId>,
}

impl BatchDeclaration {
    /// Declares a batch over `task_ids`, in the order they are declared.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPlanValidationError::EmptyBatch`] when `task_ids` is
    /// empty: a batch with no member consumes nothing and is rejected rather
    /// than skipped.
    pub fn new(
        id: BatchId,
        task_ids: Vec<TaskId>,
    ) -> Result<BatchDeclaration, BatchPlanValidationError> {
        if task_ids.is_empty() {
            return Err(BatchPlanValidationError::EmptyBatch { batch_id: id });
        }
        Ok(BatchDeclaration { id, task_ids })
    }

    /// Returns the batch identifier.
    pub fn id(&self) -> &BatchId {
        &self.id
    }

    /// Returns the member task ids, in declaration order.
    pub fn task_ids(&self) -> &[TaskId] {
        &self.task_ids
    }

    /// Reports whether `task_id` is one of the declared members.
    pub fn contains(&self, task_id: &TaskId) -> bool {
        self.task_ids.contains(task_id)
    }
}
