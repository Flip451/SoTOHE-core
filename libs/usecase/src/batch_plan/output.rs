//! What the Phase 3 gate returns to its adapter (spec `IN-06`, `IN-07`,
//! `AC-06`, `AC-07`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the workspace
//! module-size limit.
//!
//! The domain verdict stops here: the interactor maps it into these records, so
//! a primary adapter renders this crate's own types and destructures no domain
//! value object.

use domain::batch_plan::{BatchPlanGateOutcome, BatchPlanGateViolation, NonEmptyGateViolations};

// ── BatchPlanViolationOutput ──────────────────────────────────────────────────

/// One gate finding, as the driver receives it (`IN-06`, `IN-07`, `AC-06`,
/// `AC-07`).
///
/// Variant names and payload arity mirror the domain finding one for one, so
/// nothing is lost in the crossing. The payloads are display strings and counts
/// because this is the presentation boundary: the values were validated
/// upstream and the adapter only renders them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchPlanViolationOutput {
    /// A batch's total for one scope is above the resolved ceiling, and no
    /// contributor states why it cannot be split.
    CeilingExceeded {
        /// The batch whose total is above the ceiling.
        batch_id: String,
        /// The scope the total was compared for.
        scope: String,
        /// The batch's derived total for the scope.
        total: u32,
        /// The resolved ceiling the total exceeded.
        ceiling: u32,
    },
    /// A batch is above the ceiling for a scope an indivisible task contributes
    /// to, but that task is not the only contributor.
    OversizeScopeHasMultipleContributors {
        /// The batch the contributors belong to.
        batch_id: String,
        /// The scope whose total is above the ceiling.
        scope: String,
        /// The contributor that states why it cannot be split.
        indivisible_task: String,
        /// The other tasks contributing to the same scope, in batch order.
        other_contributors: Vec<String>,
    },
    /// The batch plan names a task the implementation plan does not contain.
    UnknownTaskRef {
        /// The task id the batch plan declares.
        task_id: String,
    },
    /// A planned task belongs to no batch.
    UnplannedTask {
        /// The planned task the batch plan does not place.
        task_id: String,
    },
    /// A declared dependency edge points into a batch that runs after the
    /// dependent task's own batch.
    DependencyInLaterBatch {
        /// The task that declared the dependency.
        task_id: String,
        /// The batch the dependent task belongs to.
        task_batch: String,
        /// The declared dependency.
        dependency: String,
        /// The batch the dependency belongs to.
        dependency_batch: String,
    },
}

// ── NonEmptyViolationOutputs ──────────────────────────────────────────────────

/// A findings list proven non-empty at the boundary (`IN-06`, `AC-06`,
/// `AC-07`).
///
/// Mirrors the domain's non-empty collection in structure and not only in
/// variant names: without it a blocked output could carry an empty list, so the
/// guarantee the domain establishes would be dropped exactly where the adapter
/// consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyViolationOutputs(Vec<BatchPlanViolationOutput>);

impl NonEmptyViolationOutputs {
    /// Wraps `violations` when it holds at least one finding.
    ///
    /// Returns `Option` because emptiness is a shape question at the mapping
    /// site, not a failure the use case reports.
    #[must_use]
    pub fn try_new(violations: Vec<BatchPlanViolationOutput>) -> Option<NonEmptyViolationOutputs> {
        if violations.is_empty() { None } else { Some(NonEmptyViolationOutputs(violations)) }
    }

    /// Returns the findings, in the order the gate produced them.
    #[must_use]
    pub fn as_slice(&self) -> &[BatchPlanViolationOutput] {
        &self.0
    }

    /// Consumes the wrapper and returns the findings.
    #[must_use]
    pub fn into_vec(self) -> Vec<BatchPlanViolationOutput> {
        self.0
    }

    /// Builds the boundary collection from the domain's own non-empty one.
    ///
    /// Infallible by construction: the domain collection already carries the
    /// non-emptiness and mapping element-wise preserves it, so the crossing
    /// never revisits the verdict the gate reached.
    fn from_domain(violations: NonEmptyGateViolations) -> NonEmptyViolationOutputs {
        NonEmptyViolationOutputs(violations.into_vec().into_iter().map(map_violation).collect())
    }
}

// ── BatchPlanCheckOutput ──────────────────────────────────────────────────────

/// The gate verdict, as the driver receives it (`IN-06`, `AC-06`).
///
/// Mirrors the domain verdict: a plan that conforms carries nothing, and a
/// blocked one carries its findings. `Blocked` carries
/// [`NonEmptyViolationOutputs`], so a blocked verdict with nothing to report is
/// unrepresentable on this side of the boundary too, rather than resting on the
/// upstream invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchPlanCheckOutput {
    /// The plan conforms; Phase 3 may complete.
    Passed,
    /// The plan does not conform, for the reported findings.
    Blocked {
        /// The findings that block the plan, in the order the gate produced
        /// them.
        violations: NonEmptyViolationOutputs,
    },
}

// ── Mapping ───────────────────────────────────────────────────────────────────

/// Maps the domain verdict into the output record the adapter renders.
///
/// The verdict itself is carried across unchanged: what the gate blocked stays
/// blocked, because the non-empty collection is rebuilt from the domain's own
/// rather than re-derived from a list that could turn out empty.
pub(crate) fn map_outcome(outcome: BatchPlanGateOutcome) -> BatchPlanCheckOutput {
    match outcome {
        BatchPlanGateOutcome::Passed => BatchPlanCheckOutput::Passed,
        BatchPlanGateOutcome::Blocked { violations } => BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::from_domain(violations),
        },
    }
}

fn map_violation(violation: BatchPlanGateViolation) -> BatchPlanViolationOutput {
    match violation {
        BatchPlanGateViolation::CeilingExceeded { batch_id, scope, total, ceiling } => {
            BatchPlanViolationOutput::CeilingExceeded {
                batch_id: batch_id.as_str().to_owned(),
                scope: scope.to_string(),
                total: total.value(),
                ceiling: ceiling.value(),
            }
        }
        BatchPlanGateViolation::OversizeScopeHasMultipleContributors {
            batch_id,
            scope,
            indivisible_task,
            other_contributors,
        } => BatchPlanViolationOutput::OversizeScopeHasMultipleContributors {
            batch_id: batch_id.as_str().to_owned(),
            scope: scope.to_string(),
            indivisible_task: indivisible_task.to_string(),
            other_contributors: other_contributors.iter().map(ToString::to_string).collect(),
        },
        BatchPlanGateViolation::UnknownTaskRef { task_id } => {
            BatchPlanViolationOutput::UnknownTaskRef { task_id: task_id.to_string() }
        }
        BatchPlanGateViolation::UnplannedTask { task_id } => {
            BatchPlanViolationOutput::UnplannedTask { task_id: task_id.to_string() }
        }
        BatchPlanGateViolation::DependencyInLaterBatch {
            task_id,
            task_batch,
            dependency,
            dependency_batch,
        } => BatchPlanViolationOutput::DependencyInLaterBatch {
            task_id: task_id.to_string(),
            task_batch: task_batch.as_str().to_owned(),
            dependency: dependency.to_string(),
            dependency_batch: dependency_batch.as_str().to_owned(),
        },
    }
}
