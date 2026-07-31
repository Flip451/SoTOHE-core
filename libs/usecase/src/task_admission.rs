//! The admission judgement on every transition that enters `in_progress` — a
//! start from `todo` and a reopen from done alike (spec `IN-09`, `IN-10`,
//! `IN-11`, `AC-10`, `AC-11`, `AC-12`, `AC-13`, `AC-14`, `AC-26`, `AC-27`,
//! `CN-04`, `CN-09`, `CN-10`).
//!
//! Re-exported from [`crate::task_ops`], whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the workspace
//! module-size limit.
//!
//! The judgement itself belongs to the domain: this module reads the batch
//! plan, the measured diff and the scope configuration through the same ports
//! the Phase 3 gate uses, hands them to `evaluate_admission`, and maps the
//! refusal into a record the adapter can render. Which tasks count as settled
//! and which as in progress is asked of the implementation-plan aggregate, so
//! this module holds neither that classification nor any ceiling arithmetic, and
//! no step of it consults a language model (`AC-14`).
//!
//! Starting work needs the artifact the judgement is made against: on an active
//! track a missing `batch-plan.json` is an error, not a pass (`AC-05`). The
//! non-retroactivity of `CN-09` is about which tracks the gate applies to, not
//! about letting a transition through when the plan is absent, so no exemption
//! is opened here.

use std::num::NonZeroU32;
use std::sync::Arc;

use domain::batch_plan::{
    AdmissionDecision, AdmissionEvaluationError, AdmissionRejection, LineCount, evaluate_admission,
};
use domain::{ImplPlanDocument, TaskId, TaskStatusKind, TaskTransition, TrackId};

use crate::batch_plan::{
    BatchPlanReaderPort, PlanArtifactReadError, ScopeConfigReadError, ScopeConfigReaderPort,
    ScopeDiffMeasureError, ScopeDiffMeasurePort,
};
use crate::task_ops::{TaskOperationError, TaskOperationOutput};

// ── AdmissionRejectionOutput ──────────────────────────────────────────────────

/// Why a `todo -> in_progress` transition was refused, as the adapter receives
/// it (`IN-09`, `IN-10`, `AC-10`, `AC-12`).
///
/// Variant names and payload arity mirror the domain refusal one for one, so
/// the figures survive the crossing. The payloads are display strings and
/// counts because this is the presentation boundary and the values were
/// validated upstream; `prior_contribution` keeps its non-zero type, because a
/// ceiling refusal is only reachable once a scope already carries something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRejectionOutput {
    /// The candidate belongs to a batch other than the one being consumed.
    NotCurrentBatchMember {
        /// The candidate task.
        task_id: String,
        /// The batch the candidate belongs to.
        task_batch: String,
        /// The batch currently being consumed.
        current_batch: String,
    },
    /// Admitting the candidate would take a scope past its resolved ceiling.
    ScopeCeilingWouldBeExceeded {
        /// The scope the comparison was made for.
        scope: String,
        /// What the scope already carries: the measured diff plus the estimates
        /// of the members already in progress.
        prior_contribution: NonZeroU32,
        /// The candidate's declared estimate for the scope.
        candidate_estimate: u32,
        /// The resolved ceiling the sum would exceed.
        ceiling: u32,
    },
}

impl std::fmt::Display for AdmissionRejectionOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdmissionRejectionOutput::NotCurrentBatchMember {
                task_id,
                task_batch,
                current_batch,
            } => write!(
                f,
                "task '{task_id}' belongs to batch '{task_batch}', but batch '{current_batch}' \
                 is the one being consumed"
            ),
            AdmissionRejectionOutput::ScopeCeilingWouldBeExceeded {
                scope,
                prior_contribution,
                candidate_estimate,
                ceiling,
            } => write!(
                f,
                "scope '{scope}' already carries {prior_contribution} lines and the candidate \
                 declares {candidate_estimate} more, which would exceed its ceiling of {ceiling}"
            ),
        }
    }
}

// ── TaskTransitionOutcome ─────────────────────────────────────────────────────

/// What `transition_task` reports: the transition happened, or it was refused
/// with the typed reason (`IN-09`, `IN-10`, `AC-10`, `AC-12`).
///
/// A refusal is a legitimate result rather than an exceptional condition, so it
/// travels as data and keeps its scope, contribution, ceiling and batch
/// payloads intact instead of collapsing into a rendered string in the error
/// channel.
#[derive(Debug)]
pub enum TaskTransitionOutcome {
    /// The transition was made.
    Transitioned(TaskOperationOutput),
    /// The transition was refused, for the carried reason.
    Rejected(AdmissionRejectionOutput),
}

impl TaskTransitionOutcome {
    /// Returns the reason a transition was refused, or `None` when it was made.
    #[must_use]
    pub fn rejection(&self) -> Option<&AdmissionRejectionOutput> {
        match self {
            TaskTransitionOutcome::Transitioned(_) => None,
            TaskTransitionOutcome::Rejected(rejection) => Some(rejection),
        }
    }
}

// ── The judgement ─────────────────────────────────────────────────────────────

/// The collaborators the admission judgement reads its inputs through.
pub(crate) struct AdmissionPorts<'a> {
    pub(crate) batch_plan_reader: &'a Arc<dyn BatchPlanReaderPort>,
    pub(crate) scope_diff_measurer: &'a Arc<dyn ScopeDiffMeasurePort>,
    pub(crate) scope_config_reader: &'a Arc<dyn ScopeConfigReaderPort>,
}

/// Judges a `todo -> in_progress` transition, returning the refusal when there
/// is one and `Ok(None)` when the candidate is admitted.
///
/// # Errors
///
/// Returns [`TaskOperationError::TransitionFailed`] when the track declares no
/// batch plan, when the plan holds no estimate for the candidate, and when that
/// estimate names a scope the review scope configuration does not define: in
/// every case there is nothing to judge the start of work against, and starting
/// it anyway would be the silent pass the gate exists to prevent (`AC-05`,
/// `AC-13`, `AC-23`). Returns [`TaskOperationError::StoreFailed`] when one of the
/// three reads could not be made.
pub(crate) fn judge_admission(
    ports: &AdmissionPorts<'_>,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    candidate: &TaskId,
    plan: &ImplPlanDocument,
) -> Result<Option<AdmissionRejectionOutput>, TaskOperationError> {
    let batch_plan = match ports.batch_plan_reader.read(items_dir, track_id) {
        Ok(batch_plan) => batch_plan,
        // Without the plan there is nothing to judge the start against, so the
        // transition fails rather than passing unjudged.
        Err(PlanArtifactReadError::NotFound) => {
            return Err(TaskOperationError::TransitionFailed(
                "the track declares no batch plan, so a task cannot be started".to_owned(),
            ));
        }
        Err(PlanArtifactReadError::ReadFailed { message }) => {
            return Err(TaskOperationError::StoreFailed(format!(
                "batch plan could not be read: {message}"
            )));
        }
    };

    let scope_config = ports.scope_config_reader.read(items_dir, track_id).map_err(
        |ScopeConfigReadError::ReadFailed { message }| {
            TaskOperationError::StoreFailed(format!(
                "review scope configuration could not be read: {message}"
            ))
        },
    )?;
    let measured = ports.scope_diff_measurer.measure_scope_diff(items_dir, track_id).map_err(
        |ScopeDiffMeasureError::MeasureFailed { message }| {
            TaskOperationError::StoreFailed(format!("scope diff could not be measured: {message}"))
        },
    )?;

    let decision = evaluate_admission(
        &batch_plan,
        &scope_config,
        candidate,
        &plan.settled_task_ids(),
        &plan.in_progress_task_ids(),
        &measured,
    )
    .map_err(map_evaluation_error)?;

    Ok(match decision {
        AdmissionDecision::Admitted => None,
        AdmissionDecision::Rejected(rejection) => Some(map_rejection(rejection)),
    })
}

/// Reports whether `transition` enters `in_progress` (`IN-09`, `AC-26`,
/// `AC-27`).
///
/// The judged set is every transition whose target is `in_progress` — a start
/// from `todo` and a reopen from done alike. Which source statuses those are is
/// the domain's transition table to state, not this layer's: the caller resolves
/// the transition through the plan and this asks the resolved value where it
/// leads, so no status enumeration is restated here.
pub(crate) fn enters_in_progress(transition: &TaskTransition) -> bool {
    transition.target_kind() == TaskStatusKind::InProgress
}

/// Turns a failure to judge the transition into the refusal the caller reports.
///
/// Both failures carry the same posture: the judgement could not be made, so the
/// transition fails rather than proceeding unjudged (`AC-13`, `AC-23`). Each
/// message names the task, and the unknown-name one also names the scope, so the
/// operator can see which declaration to correct.
fn map_evaluation_error(error: AdmissionEvaluationError) -> TaskOperationError {
    match error {
        AdmissionEvaluationError::MissingTaskEstimate { task_id } => {
            TaskOperationError::TransitionFailed(format!(
                "task '{task_id}' has no estimate in the batch plan"
            ))
        }
        AdmissionEvaluationError::UnknownMainScopeName { task_id, scope } => {
            TaskOperationError::TransitionFailed(format!(
                "task '{task_id}' declares an estimate for scope '{scope}', which the review \
                 scope configuration does not define"
            ))
        }
    }
}

fn map_rejection(rejection: AdmissionRejection) -> AdmissionRejectionOutput {
    match rejection {
        AdmissionRejection::NotCurrentBatchMember { task_id, task_batch, current_batch } => {
            AdmissionRejectionOutput::NotCurrentBatchMember {
                task_id: task_id.to_string(),
                task_batch: task_batch.as_str().to_owned(),
                current_batch: current_batch.as_str().to_owned(),
            }
        }
        AdmissionRejection::ScopeCeilingWouldBeExceeded {
            scope,
            prior_contribution,
            candidate_estimate,
            ceiling,
        } => AdmissionRejectionOutput::ScopeCeilingWouldBeExceeded {
            scope: scope.to_string(),
            prior_contribution: non_zero(prior_contribution.get()),
            candidate_estimate: candidate_estimate.value(),
            ceiling: ceiling.value(),
        },
    }
}

/// Carries a count already proven non-zero across to the transport type.
///
/// The source is a `NonZeroLineCount` and the conversion to `u32` saturates
/// rather than wrapping, so the result is at least one and the fallback is
/// unreachable; it is written this way to keep the mapping total.
fn non_zero(value: LineCount) -> NonZeroU32 {
    NonZeroU32::new(value.value()).unwrap_or(NonZeroU32::MIN)
}
