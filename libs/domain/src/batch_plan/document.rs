//! The declared batch plan as a whole.

use crate::review_v2::ScopeName;
use crate::{TaskId, TrackId};

use super::batch::{BatchDeclaration, BatchId};
use super::error::BatchPlanValidationError;
use super::estimate::TaskEstimate;
use super::line_count::LineCount;

/// Every batch claiming one task, in declaration order.
type Memberships<'a> = Vec<(&'a TaskId, Vec<&'a BatchId>)>;

// ── BatchPlanDocument ─────────────────────────────────────────────────────────

/// Domain model of the declared batch plan (IN-01 / IN-05 / AC-01 / AC-04 /
/// CN-01).
///
/// The constructor owns every invariant decidable from the plan alone, so a
/// codec reading the artifact re-implements none of them. A batch declares no
/// line figure: [`BatchPlanDocument::scope_total`] derives a batch's per-scope
/// quantity from its members' estimates, so no declared total can drift away
/// from the members it summarises. [`BatchPlanDocument::current_batch`] is a
/// pure derivation from the committed-task set.
///
/// Agreement with the task list in `impl-plan.json` is not checked here: that
/// judgement needs the planned tasks and belongs to the Phase 3 gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlanDocument {
    track_id: TrackId,
    task_estimates: Vec<TaskEstimate>,
    batches: Vec<BatchDeclaration>,
}

impl BatchPlanDocument {
    /// Assembles a batch plan from its declared estimates and batches.
    ///
    /// # Errors
    ///
    /// Rejects, in this order:
    /// - [`BatchPlanValidationError::DuplicateTaskEstimate`] — a task estimated
    ///   more than once.
    /// - [`BatchPlanValidationError::DuplicateBatchId`] — a batch identifier
    ///   declared more than once.
    /// - [`BatchPlanValidationError::DuplicateBatchMembership`] — a task
    ///   claimed by more than one batch membership.
    /// - [`BatchPlanValidationError::MissingTaskEstimate`] — a batch member
    ///   with no declared estimate.
    /// - [`BatchPlanValidationError::UnassignedTask`] — an estimated task that
    ///   no batch claims.
    pub fn new(
        track_id: TrackId,
        task_estimates: Vec<TaskEstimate>,
        batches: Vec<BatchDeclaration>,
    ) -> Result<BatchPlanDocument, BatchPlanValidationError> {
        reject_repeated_estimate(&task_estimates)?;
        reject_repeated_batch_id(&batches)?;
        let memberships = collect_memberships(&batches);
        reject_repeated_membership(&memberships)?;
        reject_unestimated_member(&batches, &task_estimates)?;
        reject_unassigned_task(&task_estimates, &memberships)?;
        Ok(BatchPlanDocument { track_id, task_estimates, batches })
    }

    /// Returns the track the plan was declared for.
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
    }

    /// Returns the declared task estimates, in declaration order.
    pub fn task_estimates(&self) -> &[TaskEstimate] {
        &self.task_estimates
    }

    /// Returns the declared batches, in declaration order.
    pub fn batches(&self) -> &[BatchDeclaration] {
        &self.batches
    }

    /// Returns the estimate declared for `task_id`.
    pub fn estimate_for(&self, task_id: &TaskId) -> Option<&TaskEstimate> {
        self.task_estimates.iter().find(|estimate| estimate.task_id() == task_id)
    }

    /// Returns the batch `task_id` belongs to — the single one, since the
    /// constructor rejects a task claimed more than once.
    pub fn batch_of(&self, task_id: &TaskId) -> Option<&BatchDeclaration> {
        self.batches.iter().find(|batch| batch.contains(task_id))
    }

    /// Derives `batch`'s total for `scope` as the sum of its members'
    /// estimates. Members that declare no estimate for the scope contribute
    /// nothing.
    pub fn scope_total(&self, batch: &BatchDeclaration, scope: &ScopeName) -> LineCount {
        batch
            .task_ids()
            .iter()
            .filter_map(|task_id| self.estimate_for(task_id))
            .filter_map(|estimate| estimate.estimate_for(scope))
            .fold(LineCount::new(0), |total, estimate| total.saturating_add(&estimate.total()))
    }

    /// Returns the batch currently being consumed: the earliest declared batch
    /// that still has a member outside `committed_task_ids`, or `None` once
    /// every batch is fully committed.
    pub fn current_batch(
        &self,
        committed_task_ids: &std::collections::BTreeSet<TaskId>,
    ) -> Option<&BatchDeclaration> {
        self.batches
            .iter()
            .find(|batch| batch.task_ids().iter().any(|id| !committed_task_ids.contains(id)))
    }
}

/// Rejects a task estimated more than once.
fn reject_repeated_estimate(
    task_estimates: &[TaskEstimate],
) -> Result<(), BatchPlanValidationError> {
    let mut seen: Vec<&TaskId> = Vec::new();
    for estimate in task_estimates {
        if seen.contains(&estimate.task_id()) {
            return Err(BatchPlanValidationError::DuplicateTaskEstimate {
                task_id: estimate.task_id().clone(),
            });
        }
        seen.push(estimate.task_id());
    }
    Ok(())
}

/// Rejects a batch identifier declared more than once.
fn reject_repeated_batch_id(batches: &[BatchDeclaration]) -> Result<(), BatchPlanValidationError> {
    let mut seen: Vec<&BatchId> = Vec::new();
    for batch in batches {
        if seen.contains(&batch.id()) {
            return Err(BatchPlanValidationError::DuplicateBatchId {
                batch_id: batch.id().clone(),
            });
        }
        seen.push(batch.id());
    }
    Ok(())
}

/// Collects, per task, every batch membership claiming it — in the order the
/// batches and their members are declared.
fn collect_memberships(batches: &[BatchDeclaration]) -> Memberships<'_> {
    let mut memberships: Memberships<'_> = Vec::new();
    for batch in batches {
        for task_id in batch.task_ids() {
            match memberships.iter_mut().find(|(claimed, _)| *claimed == task_id) {
                Some((_, claiming)) => claiming.push(batch.id()),
                None => memberships.push((task_id, vec![batch.id()])),
            }
        }
    }
    memberships
}

/// Rejects a task claimed by more than one batch membership.
fn reject_repeated_membership(
    memberships: &Memberships<'_>,
) -> Result<(), BatchPlanValidationError> {
    for (task_id, claiming) in memberships {
        if claiming.len() > 1 {
            return Err(BatchPlanValidationError::DuplicateBatchMembership {
                task_id: (*task_id).clone(),
                batch_ids: claiming.iter().map(|batch_id| (*batch_id).clone()).collect(),
            });
        }
    }
    Ok(())
}

/// Rejects a batch member that declares no estimate.
fn reject_unestimated_member(
    batches: &[BatchDeclaration],
    task_estimates: &[TaskEstimate],
) -> Result<(), BatchPlanValidationError> {
    for batch in batches {
        for task_id in batch.task_ids() {
            if !task_estimates.iter().any(|estimate| estimate.task_id() == task_id) {
                return Err(BatchPlanValidationError::MissingTaskEstimate {
                    task_id: task_id.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Rejects an estimated task that no batch claims.
fn reject_unassigned_task(
    task_estimates: &[TaskEstimate],
    memberships: &Memberships<'_>,
) -> Result<(), BatchPlanValidationError> {
    for estimate in task_estimates {
        if !memberships.iter().any(|(task_id, _)| *task_id == estimate.task_id()) {
            return Err(BatchPlanValidationError::UnassignedTask {
                task_id: estimate.task_id().clone(),
            });
        }
    }
    Ok(())
}
