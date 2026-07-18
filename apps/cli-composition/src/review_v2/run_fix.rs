//! Review-fix composition — wiring for the review-fix-lead driver lane.
//!
//! Extracted from `review_v2/mod.rs` to keep that module under the 700-line
//! production-code cap. Composition resolves the profile, builds the typed
//! command, and injects the interactor into `cli_driver`.

use std::sync::Arc;

use cli_driver::review::ReviewFixDriver;
use domain::{ReviewGroupName, TrackId};
use infrastructure::review_v2::CodexReviewFixRunner;
use usecase::DiagnosticMessage;
use usecase::capability_exec::ModelName;
use usecase::review_v2::ReviewRoundType;
use usecase::review_v2::run_review_fix::{
    ReviewFixRunner as _, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixError,
    RunReviewFixInteractor, RunReviewFixService, SubagentDispatchInstruction, SubagentName,
};

use super::RunReviewFixLocalInput;
use super::shared::resolve_agent_execution;

/// Builds a review-fix driver with provider auto-resolved from agent-profiles.json.
///
/// Resolves the `review-fix-lead` capability from `agent-profiles.json` at the
/// repo root. Branches on the resolved provider:
///
/// - `"codex"`: constructs [`CodexReviewFixRunner`] and injects its
///   [`RunReviewFixInteractor`] into [`ReviewFixDriver`].
/// - `"claude"`: injects an interactor that yields a typed
///   [`SubagentDispatchInstruction`]; the driver renders the sentinel and JSON.
/// - other: returns an error.
///
/// This branching keeps the orchestrator skill (`/track:review`) provider-agnostic
/// — it always calls `bin/sotp review fix-local` (via
/// `cargo make track-local-review-fix`) and reacts to the exit code: codex
/// completion (0/1/2) flows through the existing `REVIEW_FIX_STATUS` contract,
/// while a Claude resolution renders the in-process subagent dispatch protocol.
///
/// # Errors
/// Returns `Err` when profile loading, provider resolution, or typed input
/// construction fails. The driver owns invocation and outcome rendering.
pub(crate) fn review_fix_driver(
    input: RunReviewFixLocalInput,
) -> Result<ReviewFixDriver, RunReviewFixError> {
    let track_id = TrackId::try_new(input.track_id.trim()).map_err(|e| {
        RunReviewFixError::InvalidTrackId(diagnostic_message(format!("invalid --track-id: {e}")))
    })?;

    let scope = ReviewGroupName::try_new(input.scope.trim()).map_err(|e| {
        RunReviewFixError::InvalidScope(diagnostic_message(format!("invalid --scope: {e}")))
    })?;

    let round_type = ReviewRoundType::parse(&input.round_type)
        .map_err(|e| RunReviewFixError::InvalidRoundType(diagnostic_message(e.to_string())))?;
    let infra_round_type = match round_type {
        ReviewRoundType::Fast => infrastructure::agent_profiles::RoundType::Fast,
        ReviewRoundType::Final => infrastructure::agent_profiles::RoundType::Final,
    };
    let resolved =
        resolve_agent_execution(None, "review-fix-lead", infra_round_type, input.model.as_deref())
            .map_err(|e| RunReviewFixError::FixRunnerFailed(diagnostic_message(e.to_string())))?;
    let model = ModelName::try_new(resolved.model)
        .map_err(|e| RunReviewFixError::FixRunnerFailed(diagnostic_message(e.to_string())))?;
    let effort = resolved.effort;

    let command = RunReviewFixCommand {
        scope: scope.to_string(),
        briefing_file: input.briefing_file,
        track_id: track_id.to_string(),
        round_type: match round_type {
            ReviewRoundType::Fast => "fast".to_owned(),
            ReviewRoundType::Final => "final".to_owned(),
        },
        model: model.to_string(),
    };

    let service: Arc<dyn RunReviewFixService> = match resolved.provider.as_str() {
        "claude" => {
            let instruction = SubagentDispatchInstruction {
                agent: SubagentName::try_new("review-fix-lead").map_err(|e| {
                    RunReviewFixError::FixRunnerFailed(diagnostic_message(e.to_string()))
                })?,
                model,
                effort,
                scope,
                briefing_file: command.briefing_file.clone(),
                track_id,
                round_type,
            };
            let run_fn = Arc::new(move |_command: RunReviewFixCommand| {
                Err(RunReviewFixError::SubagentDispatchRequired(Box::new(instruction.clone())))
            });
            Arc::new(RunReviewFixInteractor::new(run_fn))
        }
        "codex" => {
            let runner = CodexReviewFixRunner::new(model.clone(), effort);
            let runner_arc = Arc::new(runner);
            let run_fn = Arc::new(
                move |cmd: RunReviewFixCommand| -> Result<
                    usecase::review_v2::run_review_fix::RunReviewFixOutput,
                    RunReviewFixError,
                > {
                    runner_arc.as_ref().run_fix(cmd).map_err(map_codex_fix_runner_error)
                },
            );
            Arc::new(RunReviewFixInteractor::new(run_fn))
        }
        other => {
            return Err(RunReviewFixError::FixRunnerFailed(diagnostic_message(format!(
                "unsupported review-fix-lead provider '{other}' (supported: 'codex', 'claude')"
            ))));
        }
    };

    Ok(ReviewFixDriver::new(service, command, resolved.provider))
}

fn map_codex_fix_runner_error(error: ReviewFixRunnerError) -> RunReviewFixError {
    match error {
        ReviewFixRunnerError::SmokeTestFailed(message) => {
            RunReviewFixError::SmokeTestFailed(diagnostic_message(message))
        }
        ReviewFixRunnerError::SpawnFailed(_) => {
            RunReviewFixError::FixRunnerFailed(diagnostic_message("fix runner process failed"))
        }
        ReviewFixRunnerError::SentinelNotFound(message) => {
            RunReviewFixError::FixRunnerFailed(diagnostic_message(message))
        }
        ReviewFixRunnerError::Unexpected(_) => {
            RunReviewFixError::FixRunnerFailed(diagnostic_message("fix runner failed unexpectedly"))
        }
    }
}

/// Builds a non-empty diagnostic payload for `RunReviewFixError`.
fn diagnostic_message(value: impl Into<String>) -> DiagnosticMessage {
    let mut value = value.into();
    if value.trim().is_empty() {
        value = "review-fix diagnostic detail unavailable".to_owned();
    }
    loop {
        match DiagnosticMessage::try_new(value) {
            Ok(message) => return message,
            Err(_) => value = "review-fix diagnostic detail unavailable".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::map_codex_fix_runner_error;
    use usecase::review_v2::run_review_fix::ReviewFixRunnerError;

    #[test]
    fn test_map_codex_fix_runner_error_preserves_sentinel_diagnostics() {
        let error = map_codex_fix_runner_error(ReviewFixRunnerError::SentinelNotFound(
            "codex fixer exit code 126; session log: tmp/reviewer-runtime/session.log".to_owned(),
        ));

        let rendered = error.to_string();
        assert!(rendered.contains("exit code 126"));
        assert!(rendered.contains("session log:"));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
