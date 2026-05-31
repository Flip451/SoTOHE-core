//! `review_run_fix_local` composition — wiring for the review-fix-lead fixer.
//!
//! Extracted from `review_v2/mod.rs` to keep that module under the 700-line
//! production-code cap. Public surface is unchanged: `CliApp::review_run_fix_local`
//! delegates here via `run_fix_local`.

use std::sync::Arc;

use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::review_v2::CodexReviewFixRunner;
use usecase::review_v2::run_review_fix::{
    ReviewFixRunner as _, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixError,
    RunReviewFixInteractor, RunReviewFixService as _,
};

use super::{RunReviewFixLocalInput, validate_review_group_name_str, validate_track_id_str};
use crate::CommandOutcome;

/// Run the review-fix-lead fixer with provider auto-resolved from agent-profiles.json.
///
/// Resolves the `review-fix-lead` capability from `agent-profiles.json` at the
/// repo root. Supports only `"codex"` provider — constructs `CodexReviewFixRunner`
/// and runs it through `RunReviewFixInteractor`. Unknown or unsupported providers
/// return a clear error (mirrors `review_run_local` provider rejection).
///
/// # Errors
/// Returns `Err` when profile loading, provider resolution, arg validation,
/// or the fix runner fails.
pub(crate) fn run_fix_local(input: RunReviewFixLocalInput) -> Result<CommandOutcome, String> {
    let repo = SystemGitRepo::discover()
        .map_err(|e| format!("[ERROR] failed to discover git repository root: {e}"))?;
    let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
    let profiles = AgentProfiles::load(&profiles_path)
        .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))?;

    let track_id = input.track_id.trim().to_owned();
    validate_track_id_str(&track_id).map_err(|e| format!("invalid --track-id: {e}"))?;

    let scope = input.scope.trim().to_owned();
    validate_review_group_name_str(&scope).map_err(|e| format!("invalid --scope: {e}"))?;

    let infra_round_type = match input.round_type.as_str() {
        "fast" => RoundType::Fast,
        "final" => RoundType::Final,
        other => {
            return Err(format!(
                "[ERROR] unknown round type '{other}' (expected 'fast' or 'final')"
            ));
        }
    };
    let resolved =
        profiles.resolve_execution("review-fix-lead", infra_round_type).ok_or_else(|| {
            "[ERROR] review-fix-lead capability not defined in agent-profiles.json".to_owned()
        })?;

    let model = resolved.model.clone().unwrap_or_else(|| input.model.clone());

    eprintln!("[sotp review fix-local] provider={} model={}", resolved.provider, &model);

    match resolved.provider.as_str() {
        "codex" => {
            let runner = CodexReviewFixRunner::new(
                model,
                scope.clone(),
                input.briefing_file.clone(),
                input.scope_files.clone(),
            );
            let runner_arc = Arc::new(runner);
            let run_fn = Arc::new(
                move |cmd: RunReviewFixCommand| -> Result<
                    usecase::review_v2::run_review_fix::RunReviewFixOutput,
                    RunReviewFixError,
                > {
                    runner_arc.as_ref().run_fix(cmd).map_err(|e| match e {
                        ReviewFixRunnerError::SmokeTestFailed(message) => {
                            RunReviewFixError::SmokeTestFailed(message)
                        }
                        ReviewFixRunnerError::SpawnFailed(_) => RunReviewFixError::FixRunnerFailed(
                            "fix runner process failed".to_owned(),
                        ),
                        ReviewFixRunnerError::SentinelNotFound(_) => {
                            RunReviewFixError::FixRunnerFailed(
                                "fix runner did not report a completion status".to_owned(),
                            )
                        }
                        ReviewFixRunnerError::Unexpected(_) => RunReviewFixError::FixRunnerFailed(
                            "fix runner failed unexpectedly".to_owned(),
                        ),
                    })
                },
            );
            let interactor = RunReviewFixInteractor::new(run_fn);
            let command = RunReviewFixCommand {
                scope,
                briefing_file: input.briefing_file,
                track_id,
                round_type: input.round_type,
                reviewer_model: input.reviewer_model,
                model: input.model,
                scope_files: input.scope_files,
            };
            let output = interactor.run(command).map_err(|e| format!("[ERROR] {e}"))?;
            Ok(CommandOutcome {
                stdout: Some(format!("REVIEW_FIX_STATUS: {}", output.status)),
                stderr: None,
                exit_code: u8::try_from(output.exit_code).unwrap_or(1),
            })
        }
        other => Err(format!(
            "[ERROR] unsupported review-fix-lead provider '{other}' \
             (supported: 'codex')"
        )),
    }
}
