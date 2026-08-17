//! Outcome-to-telemetry mapping shared by reviewer composition entry points.

use std::time::Instant;

use super::CodexReviewOutcome;

pub(super) struct ReviewTelemetry {
    pub(super) verdict_parse_failed: bool,
    pub(super) emit_subprocess: bool,
    pub(super) subprocess_started_at: Option<Instant>,
}

pub(super) fn review_telemetry_for_outcome<E>(
    run_result: &Result<CodexReviewOutcome, E>,
) -> Option<ReviewTelemetry> {
    match run_result {
        Ok(CodexReviewOutcome::WithDiagnostics { outcome, .. }) => telemetry_for_outcome(outcome),
        Ok(outcome) => telemetry_for_outcome(outcome),
        Err(_) => None,
    }
}

fn telemetry_for_outcome(outcome: &CodexReviewOutcome) -> Option<ReviewTelemetry> {
    match outcome {
        CodexReviewOutcome::WithDiagnostics { outcome, .. } => telemetry_for_outcome(outcome),
        CodexReviewOutcome::FinalCompleted { subprocess_started_at, .. }
        | CodexReviewOutcome::FastCompleted { subprocess_started_at, .. } => {
            Some(ReviewTelemetry {
                verdict_parse_failed: false,
                emit_subprocess: true,
                subprocess_started_at: Some(*subprocess_started_at),
            })
        }
        CodexReviewOutcome::Skipped { .. } => Some(ReviewTelemetry {
            verdict_parse_failed: false,
            emit_subprocess: false,
            subprocess_started_at: None,
        }),
        CodexReviewOutcome::SubprocessFailed {
            verdict_parse_failed,
            subprocess_started_at,
            ..
        } => Some(ReviewTelemetry {
            verdict_parse_failed: *verdict_parse_failed,
            emit_subprocess: true,
            subprocess_started_at: Some(*subprocess_started_at),
        }),
    }
}
