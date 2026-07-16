//! Outcome-to-telemetry mapping shared by reviewer composition entry points.

use std::time::Instant;

use super::CodexReviewOutcome;

pub(super) struct ReviewTelemetry<'a> {
    pub(super) findings_count: u32,
    pub(super) round_type: &'a str,
    pub(super) verdict_parse_failed: bool,
    pub(super) emit_subprocess: bool,
    pub(super) subprocess_started_at: Option<Instant>,
}

pub(super) fn review_telemetry_for_outcome<'a, E>(
    run_result: &'a Result<CodexReviewOutcome, E>,
    requested_round_type: &'a str,
) -> Option<ReviewTelemetry<'a>> {
    match run_result {
        Ok(CodexReviewOutcome::FinalCompleted {
            findings_count, subprocess_started_at, ..
        }) => Some(ReviewTelemetry {
            findings_count: *findings_count,
            round_type: "final",
            verdict_parse_failed: false,
            emit_subprocess: true,
            subprocess_started_at: Some(*subprocess_started_at),
        }),
        Ok(CodexReviewOutcome::FastCompleted { findings_count, subprocess_started_at, .. }) => {
            Some(ReviewTelemetry {
                findings_count: *findings_count,
                round_type: "fast",
                verdict_parse_failed: false,
                emit_subprocess: true,
                subprocess_started_at: Some(*subprocess_started_at),
            })
        }
        Ok(CodexReviewOutcome::Skipped { .. }) => Some(ReviewTelemetry {
            findings_count: 0,
            round_type: requested_round_type,
            verdict_parse_failed: false,
            emit_subprocess: false,
            subprocess_started_at: None,
        }),
        Ok(CodexReviewOutcome::SubprocessFailed {
            round_type,
            verdict_parse_failed,
            findings_count,
            subprocess_started_at,
            ..
        }) => Some(ReviewTelemetry {
            findings_count: *findings_count,
            round_type,
            verdict_parse_failed: *verdict_parse_failed,
            emit_subprocess: true,
            subprocess_started_at: Some(*subprocess_started_at),
        }),
        Err(_) => None,
    }
}
