//! Domain model of the declared batch plan.
//!
//! Holds the per-task line estimates the planner declares for each review scope,
//! the ordered batches those tasks are assigned to, the rejections that are
//! decidable from the plan file alone, the Phase 3 terminal gate that compares
//! the declared plan against the scope configuration and the planned tasks, and
//! the admission guard a `todo -> in_progress` transition passes through.
//!
//! See ADR 2026-07-28-1521-scope-diff-ceiling-admission-enforcement.

use crate::review_v2::{MainScopeName, ReviewScopeConfig, ScopeName};
use crate::{TaskId, TrackTask};

mod admission;
mod batch;
mod document;
mod error;
mod estimate;
mod gate;
mod line_count;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use admission::{
    AdmissionDecision, AdmissionEvaluationError, AdmissionRejection, NonZeroLineCount,
};
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
/// 1. every task the plan names exists in `planned_tasks`, whatever state that
///    task is in;
/// 2. every unsettled planned task belongs to a batch — a settled one need not be
///    declared, though declaring it is permitted and then judged in full;
/// 3. every declared dependency edge points at the dependent task's own batch
///    or an earlier one — task pairs that declare no dependency are not
///    examined;
/// 4. every batch's per-scope total stays within the resolved ceiling, unless
///    the only contributor to that scope states why it cannot be split;
/// 5. every main scope name an estimate declares exists in the configured scope
///    set — the reserved implicit scope `other` is always valid.
///
/// A scope with no configured ceiling is not compared, so its total cannot
/// change the verdict; a name the configuration does not hold reaches that state
/// by no third route, because check 5 reports it. This gate judges structure
/// only: whether an indivisibility reason is convincing, and whether a batch is
/// composed sensibly, stay with the reviewer.
pub fn check_batch_plan(
    plan: &BatchPlanDocument,
    scope_config: &ReviewScopeConfig,
    planned_tasks: &[TrackTask],
) -> BatchPlanGateOutcome {
    let mut violations = Vec::new();
    collect_task_set_violations(plan, planned_tasks, &mut violations);
    collect_dependency_order_violations(plan, planned_tasks, &mut violations);
    collect_ceiling_violations(plan, scope_config, &mut violations);
    collect_scope_name_violations(plan, scope_config, &mut violations);
    BatchPlanGateOutcome::from_violations(violations)
}

/// Reports the two directions of disagreement between the batch plan's task set
/// and the implementation plan's: a plan-declared id no task provides, and an
/// unsettled planned task no batch claims.
///
/// The two directions deliberately have different domains. Id existence is asked
/// of every declared id whatever state the task it names is in, because a plan
/// that names a task nobody planned is wrong either way. Membership is asked only
/// of unsettled tasks: what the plan must declare is the remaining work, so a
/// settled task the plan leaves out is not a finding. Settledness is read off the
/// status each task already carries, by the same rule the admission uses.
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
        if task.status().is_settled() {
            continue;
        }
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

/// Reports estimates that name a main scope the configured scope set does not
/// hold.
///
/// Fail-closed: an unrecognised name is a finding rather than a scope quietly
/// left uncompared, so the only ways a total goes unchecked stay the two
/// legitimate ones — a configured scope with no ceiling, and the reserved
/// implicit scope `other`.
fn collect_scope_name_violations(
    plan: &BatchPlanDocument,
    scope_config: &ReviewScopeConfig,
    violations: &mut Vec<BatchPlanGateViolation>,
) {
    for estimate in plan.task_estimates() {
        for scope_estimate in estimate.scope_estimates() {
            let Some(unknown) = unknown_main_scope_name(scope_config, scope_estimate.scope())
            else {
                continue;
            };
            violations.push(BatchPlanGateViolation::UnknownMainScopeName {
                task_id: estimate.task_id().clone(),
                scope: unknown.clone(),
            });
        }
    }
}

/// Returns the main scope name `scope` carries when the configuration does not
/// hold it, and `None` when it is configured or is the reserved implicit scope.
///
/// The one place the question is asked, so the Phase 3 gate and the admission
/// judgement cannot drift apart on which names are acceptable. `other` yields
/// `None` because the configuration treats it as valid.
fn unknown_main_scope_name<'a>(
    scope_config: &ReviewScopeConfig,
    scope: &'a ScopeName,
) -> Option<&'a MainScopeName> {
    match scope {
        ScopeName::Other => None,
        ScopeName::Main(name) => (!scope_config.contains_scope(scope)).then_some(name),
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

// ── evaluate_admission ────────────────────────────────────────────────────────

/// Judges a `todo -> in_progress` transition for `candidate`
/// (IN-09 / IN-10 / IN-11 / AC-10 / AC-11 / AC-12 / AC-13 / CN-03).
///
/// Pure, and settled by arithmetic and membership alone. Two things are asked
/// in turn:
///
/// 1. **Membership** — the candidate must belong to the batch currently being
///    consumed: the earliest declared batch that still has an uncommitted
///    member. A task from a later batch is refused until the current one is
///    committed.
/// 2. **Measured guard** — for each scope the candidate declares an estimate
///    for, the prior contribution (the measured per-scope diff plus the
///    declared estimates of the members already in progress) plus the
///    candidate's own estimate is compared with the resolved ceiling. A scope
///    whose prior contribution is zero admits the candidate whatever it
///    declares, so the first contribution to a scope always gets through and
///    the guard cannot deadlock. Scopes the candidate does not add to are not
///    examined, so an existing excess elsewhere does not block the transition.
///
/// # Errors
///
/// Returns [`AdmissionEvaluationError::MissingTaskEstimate`] when the batch plan
/// holds no estimate for `candidate`: without one there is nothing to compare,
/// and the transition is an error rather than a silent pass. Returns
/// [`AdmissionEvaluationError::UnknownMainScopeName`], of the same rank, when the
/// candidate's estimate names a main scope the configuration does not hold: such
/// a name must not resolve to an unconstrained ceiling and admit the transition
/// on that basis.
pub fn evaluate_admission(
    plan: &BatchPlanDocument,
    scope_config: &ReviewScopeConfig,
    candidate: &TaskId,
    committed_task_ids: &std::collections::BTreeSet<TaskId>,
    in_progress_task_ids: &std::collections::BTreeSet<TaskId>,
    measured: &[MeasuredScopeDiff],
) -> Result<AdmissionDecision, AdmissionEvaluationError> {
    let Some(candidate_estimate) = plan.estimate_for(candidate) else {
        return Err(AdmissionEvaluationError::MissingTaskEstimate { task_id: candidate.clone() });
    };
    // Asked of every declared scope before any comparison is made, so an
    // unrecognised name fails the evaluation whatever the arithmetic would have
    // said about it.
    for scope_estimate in candidate_estimate.scope_estimates() {
        if let Some(unknown) = unknown_main_scope_name(scope_config, scope_estimate.scope()) {
            return Err(AdmissionEvaluationError::UnknownMainScopeName {
                task_id: candidate.clone(),
                scope: unknown.clone(),
            });
        }
    }

    if let Some(rejection) = membership_rejection(plan, candidate, committed_task_ids) {
        return Ok(AdmissionDecision::Rejected(rejection));
    }

    for scope_estimate in candidate_estimate.scope_estimates() {
        let scope = scope_estimate.scope();
        let Some(prior_contribution) = NonZeroLineCount::try_new(prior_contribution(
            plan,
            candidate,
            in_progress_task_ids,
            measured,
            scope,
        )) else {
            // First contribution to this scope: always admitted (CN-03).
            continue;
        };

        let ceiling = ScopeCeiling::resolve(scope_config.diff_ceiling_for_scope(scope));
        let would_reach = prior_contribution.get().saturating_add(&scope_estimate.total());
        if ceiling.admits(&would_reach) {
            continue;
        }
        let Some(limit) = ceiling.limit() else { continue };

        return Ok(AdmissionDecision::Rejected(AdmissionRejection::ScopeCeilingWouldBeExceeded {
            scope: scope.clone(),
            prior_contribution,
            candidate_estimate: scope_estimate.total(),
            ceiling: limit,
        }));
    }

    Ok(AdmissionDecision::Admitted)
}

/// Refuses a candidate that belongs to a batch other than the one being
/// consumed. With every batch committed there is no batch left to consume, and
/// membership cannot be violated.
fn membership_rejection(
    plan: &BatchPlanDocument,
    candidate: &TaskId,
    committed_task_ids: &std::collections::BTreeSet<TaskId>,
) -> Option<AdmissionRejection> {
    let task_batch = plan.batch_of(candidate)?;
    let current_batch = plan.current_batch(committed_task_ids)?;
    if task_batch.id() == current_batch.id() {
        return None;
    }
    Some(AdmissionRejection::NotCurrentBatchMember {
        task_id: candidate.clone(),
        task_batch: task_batch.id().clone(),
        current_batch: current_batch.id().clone(),
    })
}

/// Sums what `scope` already carries: the measured diff plus the declared
/// estimates of the members already in progress.
///
/// The candidate itself is excluded, and counting a member's measured work
/// alongside its declared estimate is deliberate — the resulting overcount errs
/// towards a smaller batch (CN-04).
fn prior_contribution(
    plan: &BatchPlanDocument,
    candidate: &TaskId,
    in_progress_task_ids: &std::collections::BTreeSet<TaskId>,
    measured: &[MeasuredScopeDiff],
    scope: &ScopeName,
) -> LineCount {
    let measured_lines = measured
        .iter()
        .find(|diff| diff.scope() == scope)
        .map_or_else(|| LineCount::new(0), MeasuredScopeDiff::lines);

    in_progress_task_ids
        .iter()
        .filter(|task_id| *task_id != candidate)
        .filter_map(|task_id| plan.estimate_for(task_id))
        .filter_map(|estimate| estimate.estimate_for(scope))
        .fold(measured_lines, |total, estimate| total.saturating_add(&estimate.total()))
}
