//! Domain model of the declared batch plan.
//!
//! Holds the per-task line estimates the planner declares for each review scope,
//! the ordered batches those tasks are assigned to, the rejections that are
//! decidable from the plan file alone, and the Phase 3 terminal gate that
//! compares the declared plan against the scope configuration and the planned
//! tasks. The transition admission guard consumes these values but is owned
//! elsewhere.
//!
//! See ADR 2026-07-28-1521-scope-diff-ceiling-admission-enforcement.

use crate::TrackTask;
use crate::review_v2::{ReviewScopeConfig, ScopeName};

mod batch;
mod document;
mod error;
mod estimate;
mod gate;
mod line_count;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use batch::{BatchDeclaration, BatchId};
pub use document::BatchPlanDocument;
pub use error::BatchPlanValidationError;
pub use estimate::{
    IndivisibilityJustification, ScopeLineEstimate, TaskDecomposition, TaskEstimate,
};
pub use gate::{BatchPlanGateOutcome, BatchPlanGateViolation, NonEmptyGateViolations};
pub use line_count::{LineCount, MeasuredScopeDiff, ScopeCeiling};

// ── check_batch_plan ──────────────────────────────────────────────────────────

/// Runs the Phase 3 terminal gate over `plan` (IN-06 / IN-07 / AC-06 / AC-07 /
/// AC-08).
///
/// Pure: it takes the declared plan, the scope configuration and the planned
/// tasks, and returns the verdict as data. `planned_tasks` carries whole tasks
/// rather than ids because the dependency-edge check reads what each task
/// declares, so the batch plan needs no dependency field of its own.
///
/// Checks, in report order:
/// 1. every task the plan names exists in `planned_tasks`;
/// 2. every planned task belongs to a batch;
/// 3. every declared dependency edge points at the dependent task's own batch
///    or an earlier one — task pairs that declare no dependency are not
///    examined;
/// 4. every batch's per-scope total stays within the resolved ceiling, unless
///    the only contributor to that scope states why it cannot be split.
///
/// A scope with no configured ceiling is not compared, so its total cannot
/// change the verdict. This gate judges structure only: whether an
/// indivisibility reason is convincing, and whether a batch is composed
/// sensibly, stay with the reviewer.
pub fn check_batch_plan(
    plan: &BatchPlanDocument,
    scope_config: &ReviewScopeConfig,
    planned_tasks: &[TrackTask],
) -> BatchPlanGateOutcome {
    let mut violations = Vec::new();
    collect_task_set_violations(plan, planned_tasks, &mut violations);
    collect_dependency_order_violations(plan, planned_tasks, &mut violations);
    collect_ceiling_violations(plan, scope_config, &mut violations);
    BatchPlanGateOutcome::from_violations(violations)
}

/// Reports the two directions of disagreement between the batch plan's task set
/// and the implementation plan's: a plan-declared id no task provides, and a
/// planned task no batch claims.
fn collect_task_set_violations(
    plan: &BatchPlanDocument,
    planned_tasks: &[TrackTask],
    violations: &mut Vec<BatchPlanGateViolation>,
) {
    for estimate in plan.task_estimates() {
        if !planned_tasks.iter().any(|task| task.id() == estimate.task_id()) {
            violations.push(BatchPlanGateViolation::UnknownTaskRef {
                task_id: estimate.task_id().clone(),
            });
        }
    }
    for task in planned_tasks {
        if plan.batch_of(task.id()).is_none() {
            violations.push(BatchPlanGateViolation::UnplannedTask { task_id: task.id().clone() });
        }
    }
}

/// Reports declared dependency edges whose target sits in a later batch. An
/// edge whose endpoints are not both placed is left to the task-set checks.
fn collect_dependency_order_violations(
    plan: &BatchPlanDocument,
    planned_tasks: &[TrackTask],
    violations: &mut Vec<BatchPlanGateViolation>,
) {
    for task in planned_tasks {
        let Some(task_batch) = plan.batch_of(task.id()) else { continue };
        let Some(task_position) = batch_position(plan, task_batch.id()) else { continue };
        for dependency in task.depends_on() {
            let Some(dependency_batch) = plan.batch_of(dependency) else { continue };
            let Some(dependency_position) = batch_position(plan, dependency_batch.id()) else {
                continue;
            };
            if dependency_position > task_position {
                violations.push(BatchPlanGateViolation::DependencyInLaterBatch {
                    task_id: task.id().clone(),
                    task_batch: task_batch.id().clone(),
                    dependency: dependency.clone(),
                    dependency_batch: dependency_batch.id().clone(),
                });
            }
        }
    }
}

/// Compares each batch's per-scope total against the resolved ceiling, and
/// applies the single-contributor exemption.
fn collect_ceiling_violations(
    plan: &BatchPlanDocument,
    scope_config: &ReviewScopeConfig,
    violations: &mut Vec<BatchPlanGateViolation>,
) {
    for batch in plan.batches() {
        for scope in declared_scopes(plan, batch) {
            let total = plan.scope_total(batch, scope);
            let ceiling = ScopeCeiling::resolve(scope_config.diff_ceiling_for_scope(scope));
            if ceiling.admits(&total) {
                continue;
            }
            let Some(limit) = ceiling.limit() else { continue };

            let contributors = contributors_to(plan, batch, scope);
            let indivisible =
                contributors.iter().position(|estimate| estimate.decomposition().is_indivisible());
            match indivisible {
                // The one contributor states why it cannot be split: exempt.
                Some(_) if contributors.len() == 1 => {}
                Some(index) => {
                    let Some(indivisible_task) = contributors.get(index) else { continue };
                    violations.push(BatchPlanGateViolation::OversizeScopeHasMultipleContributors {
                        batch_id: batch.id().clone(),
                        scope: scope.clone(),
                        indivisible_task: indivisible_task.task_id().clone(),
                        other_contributors: contributors
                            .iter()
                            .enumerate()
                            .filter(|(position, _)| *position != index)
                            .map(|(_, estimate)| estimate.task_id().clone())
                            .collect(),
                    });
                }
                None => violations.push(BatchPlanGateViolation::CeilingExceeded {
                    batch_id: batch.id().clone(),
                    scope: scope.clone(),
                    total,
                    ceiling: limit,
                }),
            }
        }
    }
}

/// Returns `batch_id`'s place in the declared batch order.
fn batch_position(plan: &BatchPlanDocument, batch_id: &BatchId) -> Option<usize> {
    plan.batches().iter().position(|batch| batch.id() == batch_id)
}

/// Returns the scopes `batch`'s members declare estimates for, in declaration
/// order and without repetition.
fn declared_scopes<'a>(
    plan: &'a BatchPlanDocument,
    batch: &BatchDeclaration,
) -> Vec<&'a ScopeName> {
    let mut scopes: Vec<&ScopeName> = Vec::new();
    for task_id in batch.task_ids() {
        let Some(estimate) = plan.estimate_for(task_id) else { continue };
        for scope_estimate in estimate.scope_estimates() {
            if !scopes.contains(&scope_estimate.scope()) {
                scopes.push(scope_estimate.scope());
            }
        }
    }
    scopes
}

/// Returns the members of `batch` that declare an estimate for `scope`, in
/// batch order. Declaring an estimate is what makes a task a contributor: a
/// task that does not touch the scope declares nothing for it.
fn contributors_to<'a>(
    plan: &'a BatchPlanDocument,
    batch: &BatchDeclaration,
    scope: &ScopeName,
) -> Vec<&'a TaskEstimate> {
    batch
        .task_ids()
        .iter()
        .filter_map(|task_id| plan.estimate_for(task_id))
        .filter(|estimate| estimate.estimate_for(scope).is_some())
        .collect()
}
