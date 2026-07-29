//! The verdict of the transition admission guard.
//!
//! The judgement itself lives in the parent module, beside the value objects it
//! reads.

use std::fmt;

use thiserror::Error;

use crate::TaskId;
use crate::review_v2::ScopeName;

use super::batch::BatchId;
use super::line_count::LineCount;

// ── NonZeroLineCount ──────────────────────────────────────────────────────────

/// A [`LineCount`] proven non-zero (CN-03 / IN-10 / AC-11).
///
/// [`AdmissionRejection::ScopeCeilingWouldBeExceeded`] carries this type, so a
/// ceiling rejection cannot be constructed unless a non-zero prior contribution
/// was obtained first — which makes "the first contribution to a scope is always
/// admitted" a property of the types rather than of a documented rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonZeroLineCount(LineCount);

impl NonZeroLineCount {
    /// Wraps `value` when it is not zero.
    ///
    /// Returns `None` rather than an error because zero is the admit branch of
    /// the guard, not a failure.
    pub fn try_new(value: LineCount) -> Option<NonZeroLineCount> {
        if value.value() == 0 { None } else { Some(NonZeroLineCount(value)) }
    }

    /// Returns the quantity.
    pub fn get(&self) -> LineCount {
        self.0
    }
}

// ── AdmissionRejection ────────────────────────────────────────────────────────

/// Why a `todo -> in_progress` transition was refused
/// (IN-09 / IN-10 / AC-10 / AC-12 / CN-03).
///
/// Carries the arithmetic rather than a rendered sentence; `Display` renders it
/// at the CLI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRejection {
    /// The candidate belongs to a batch other than the one currently being
    /// consumed.
    NotCurrentBatchMember {
        /// The candidate task.
        task_id: TaskId,
        /// The batch the candidate belongs to.
        task_batch: BatchId,
        /// The batch currently being consumed.
        current_batch: BatchId,
    },
    /// Admitting the candidate would take a scope past its resolved ceiling.
    ScopeCeilingWouldBeExceeded {
        /// The scope the comparison was made for.
        scope: ScopeName,
        /// What the scope already carries: the measured diff plus the estimates
        /// of the members already in progress.
        prior_contribution: NonZeroLineCount,
        /// The candidate's declared estimate for the scope.
        candidate_estimate: LineCount,
        /// The resolved ceiling the sum would exceed.
        ceiling: LineCount,
    },
}

impl fmt::Display for AdmissionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdmissionRejection::NotCurrentBatchMember { task_id, task_batch, current_batch } => {
                write!(
                    f,
                    "task '{task_id}' belongs to batch '{}', but batch '{}' is the one being consumed",
                    task_batch.as_str(),
                    current_batch.as_str()
                )
            }
            AdmissionRejection::ScopeCeilingWouldBeExceeded {
                scope,
                prior_contribution,
                candidate_estimate,
                ceiling,
            } => write!(
                f,
                "scope '{scope}' already carries {} lines and the candidate declares {} more, \
                 which would exceed its ceiling of {}",
                prior_contribution.get().value(),
                candidate_estimate.value(),
                ceiling.value()
            ),
        }
    }
}

// ── AdmissionDecision ─────────────────────────────────────────────────────────

/// The admission verdict (IN-09 / IN-10 / AC-10 / AC-11).
///
/// An admitted transition carries no rejection and a refused one always carries
/// its reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionDecision {
    /// The transition may proceed.
    Admitted,
    /// The transition is refused, for the carried reason.
    Rejected(AdmissionRejection),
}

impl AdmissionDecision {
    /// Reports whether the transition may proceed.
    pub fn is_admitted(&self) -> bool {
        matches!(self, AdmissionDecision::Admitted)
    }

    /// Returns the reason a transition was refused, or `None` when it was
    /// admitted.
    pub fn rejection(&self) -> Option<&AdmissionRejection> {
        match self {
            AdmissionDecision::Admitted => None,
            AdmissionDecision::Rejected(rejection) => Some(rejection),
        }
    }
}

// ── AdmissionEvaluationError ──────────────────────────────────────────────────

/// A failure of the admission evaluation, as distinct from a rejection
/// (IN-11 / AC-13).
///
/// Only one case: the batch plan holds no estimate for the candidate, so no
/// judgement can be made and the transition is an error rather than a silent
/// pass. Every other malformed-plan shape is already refused by
/// `BatchPlanDocument::new`.
#[derive(Debug, Error)]
pub enum AdmissionEvaluationError {
    /// The candidate declares no estimate in the batch plan.
    #[error("task '{task_id}' has no estimate in the batch plan")]
    MissingTaskEstimate {
        /// The candidate task.
        task_id: TaskId,
    },
}
