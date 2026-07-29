//! The verdict of the Phase 3 terminal gate.
//!
//! The gate run itself lives in the parent module, beside the value objects it
//! judges.

use crate::TaskId;
use crate::review_v2::ScopeName;

use super::batch::BatchId;
use super::line_count::LineCount;

// ── BatchPlanGateViolation ────────────────────────────────────────────────────

/// One structural finding of the Phase 3 gate (IN-06 / IN-07 / AC-06 / AC-07).
///
/// Findings are data inside the verdict rather than an error type: the gate
/// reports every violation it finds instead of failing at the first one. Each
/// payload names the offending parties rather than counting them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchPlanGateViolation {
    /// A batch's total for one scope is above the resolved ceiling, and no
    /// contributor states why it cannot be split.
    CeilingExceeded {
        /// The batch whose total is above the ceiling.
        batch_id: BatchId,
        /// The scope the total was compared for.
        scope: ScopeName,
        /// The batch's derived total for the scope.
        total: LineCount,
        /// The resolved ceiling the total exceeded.
        ceiling: LineCount,
    },
    /// A batch is above the ceiling for a scope an indivisible task contributes
    /// to, but that task is not the only contributor — so the exemption for a
    /// justified oversize task does not apply.
    OversizeScopeHasMultipleContributors {
        /// The batch the contributors belong to.
        batch_id: BatchId,
        /// The scope whose total is above the ceiling.
        scope: ScopeName,
        /// The contributor that states why it cannot be split.
        indivisible_task: TaskId,
        /// The other tasks contributing to the same scope, in batch order.
        other_contributors: Vec<TaskId>,
    },
    /// The batch plan names a task the implementation plan does not contain.
    UnknownTaskRef {
        /// The task id the batch plan declares.
        task_id: TaskId,
    },
    /// A planned task belongs to no batch.
    UnplannedTask {
        /// The planned task the batch plan does not place.
        task_id: TaskId,
    },
    /// A declared dependency edge points into a batch that runs after the
    /// dependent task's own batch.
    DependencyInLaterBatch {
        /// The task that declared the dependency.
        task_id: TaskId,
        /// The batch the dependent task belongs to.
        task_batch: BatchId,
        /// The declared dependency.
        dependency: TaskId,
        /// The batch the dependency belongs to.
        dependency_batch: BatchId,
    },
}

// ── NonEmptyGateViolations ────────────────────────────────────────────────────

/// A violation list proven non-empty (IN-06 / AC-06 / AC-07).
///
/// Carries the blocked verdict's non-emptiness in the type, so a blocked
/// outcome with nothing to report is unrepresentable rather than rejected at
/// run time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyGateViolations(Vec<BatchPlanGateViolation>);

impl NonEmptyGateViolations {
    /// Wraps `violations` when it holds at least one finding.
    pub fn try_new(violations: Vec<BatchPlanGateViolation>) -> Option<NonEmptyGateViolations> {
        if violations.is_empty() { None } else { Some(NonEmptyGateViolations(violations)) }
    }

    /// Returns the findings, in the order the gate produced them.
    pub fn as_slice(&self) -> &[BatchPlanGateViolation] {
        &self.0
    }

    /// Consumes the wrapper and returns the findings.
    pub fn into_vec(self) -> Vec<BatchPlanGateViolation> {
        self.0
    }
}

// ── BatchPlanGateOutcome ──────────────────────────────────────────────────────

/// The Phase 3 gate verdict (IN-06 / AC-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchPlanGateOutcome {
    /// The plan conforms; Phase 3 may complete.
    Passed,
    /// The plan does not conform, for the reported findings.
    Blocked {
        /// The findings that block the plan.
        violations: NonEmptyGateViolations,
    },
}

impl BatchPlanGateOutcome {
    /// Builds the verdict from the findings a gate run produced. Total: an
    /// empty list is `Passed`, so no error channel is needed.
    pub fn from_violations(violations: Vec<BatchPlanGateViolation>) -> BatchPlanGateOutcome {
        match NonEmptyGateViolations::try_new(violations) {
            Some(violations) => BatchPlanGateOutcome::Blocked { violations },
            None => BatchPlanGateOutcome::Passed,
        }
    }

    /// Returns the findings of a blocked verdict, or `None` when the plan
    /// passed.
    pub fn violations(&self) -> Option<&NonEmptyGateViolations> {
        match self {
            BatchPlanGateOutcome::Passed => None,
            BatchPlanGateOutcome::Blocked { violations } => Some(violations),
        }
    }
}
