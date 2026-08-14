//! Presentation-only renderer for the check-zero-findings command boundary.

use usecase::review_v2::{ReviewCheckZeroFindingsEvaluationError, ReviewCheckZeroFindingsOutcome};

use crate::render::CommandOutcome;

/// Converts the check-zero-findings evaluation into the command's fail-closed
/// exit-code contract.
pub(super) fn check_zero_findings_outcome_to_command_outcome(
    outcome: ReviewCheckZeroFindingsOutcome,
) -> CommandOutcome {
    match outcome {
        ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings => CommandOutcome::success(Some(
            "current final review verdict is zero_findings".to_owned(),
        )),
        ReviewCheckZeroFindingsOutcome::EmptyScope => CommandOutcome::success(Some(
            "review scope is empty; no final review verdict is required".to_owned(),
        )),
        ReviewCheckZeroFindingsOutcome::MissingFinalVerdict => CommandOutcome::failure(Some(
            "no final review verdict exists for this scope".to_owned(),
        )),
        ReviewCheckZeroFindingsOutcome::StaleFinalVerdict => {
            CommandOutcome::failure(Some("final review verdict is stale for this scope".to_owned()))
        }
        ReviewCheckZeroFindingsOutcome::FindingsRemain => {
            CommandOutcome::failure(Some("final review verdict has findings remaining".to_owned()))
        }
    }
}

/// Maps both a completed check and an evaluation error to the command boundary.
pub(super) fn check_zero_findings_result_to_command_outcome(
    result: Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError>,
) -> CommandOutcome {
    match result {
        Ok(outcome) => check_zero_findings_outcome_to_command_outcome(outcome),
        Err(error) => CommandOutcome::failure(Some(error.to_string())),
    }
}
