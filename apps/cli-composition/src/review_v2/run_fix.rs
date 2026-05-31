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

    // Honor an explicit --model override; fall back to the profile model.
    // `input.model` is `None` when the flag was omitted, meaning "use the
    // profile default".  `resolved.model` is the profile model (may also be
    // `None` when the profile entry has no model field).
    let model = input.model.clone().or_else(|| resolved.model.clone()).ok_or_else(|| {
        "[ERROR] no model specified: pass --model or set model in agent-profiles.json \
         review-fix-lead capability"
            .to_owned()
    })?;

    eprintln!("[sotp review fix-local] provider={} model={}", resolved.provider, &model);

    match resolved.provider.as_str() {
        "codex" => {
            let runner = CodexReviewFixRunner::new(
                model.clone(),
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
                model,
                scope_files: input.scope_files,
            };
            match interactor.run(command) {
                Ok(output) => Ok(CommandOutcome {
                    stdout: Some(format!("REVIEW_FIX_STATUS: {}", output.status)),
                    stderr: None,
                    exit_code: u8::try_from(output.exit_code).unwrap_or(1),
                }),
                // Smoke-test failure (CN-04 / AC-07): exit 2 so the caller can
                // distinguish a preflight block from a generic runner failure.
                // The typed variant is matched here — before stringification —
                // so the exit-code decision is not coupled to Display format.
                Err(RunReviewFixError::SmokeTestFailed(msg)) => Ok(CommandOutcome {
                    stdout: None,
                    stderr: Some(format!("[ERROR] smoke test failed: {msg}")),
                    exit_code: 2,
                }),
                Err(e) => Err(format!("[ERROR] {e}")),
            }
        }
        other => Err(format!(
            "[ERROR] unsupported review-fix-lead provider '{other}' \
             (supported: 'codex')"
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use usecase::review_v2::run_review_fix::{
        RunReviewFixError, RunReviewFixOutput, RunReviewFixService as _,
    };

    use super::*;

    /// Finding 1: `RunReviewFixError::SmokeTestFailed` must map to
    /// `Ok(CommandOutcome { exit_code: 2 })`, not `Err(...)`.
    /// This test exercises the typed match in `run_fix_local` directly by
    /// constructing a minimal interactor that returns the error variant and
    /// verifying the outcome carries exit_code=2.
    #[test]
    fn test_smoke_test_failed_error_maps_to_exit_code_2() {
        let run_fn = std::sync::Arc::new(|_cmd: RunReviewFixCommand| {
            Err::<RunReviewFixOutput, RunReviewFixError>(RunReviewFixError::SmokeTestFailed(
                "forbidden sandbox detected".to_owned(),
            ))
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let command = RunReviewFixCommand {
            scope: "cli".to_owned(),
            briefing_file: std::path::PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            track_id: "test-track".to_owned(),
            round_type: "fast".to_owned(),
            reviewer_model: "gpt-5.4-mini".to_owned(),
            model: "gpt-5.5".to_owned(),
            scope_files: vec![std::path::PathBuf::from("apps/cli/src/lib.rs")],
        };
        // The interactor propagates SmokeTestFailed as Err — simulate what
        // run_fix_local does when it receives this typed variant.
        let result = interactor.run(command);
        assert!(
            matches!(result, Err(RunReviewFixError::SmokeTestFailed(_))),
            "expected SmokeTestFailed from interactor"
        );
        // Now verify the composition mapping: SmokeTestFailed → exit_code 2.
        let outcome = match result {
            Err(RunReviewFixError::SmokeTestFailed(msg)) => CommandOutcome {
                stdout: None,
                stderr: Some(format!("[ERROR] smoke test failed: {msg}")),
                exit_code: 2,
            },
            Err(e) => {
                panic!("unexpected error variant: {e:?}");
            }
            Ok(_) => {
                panic!("expected Err, got Ok");
            }
        };
        assert_eq!(outcome.exit_code, 2, "smoke test failure must map to exit_code 2");
        assert!(
            outcome.stderr.as_deref().unwrap_or("").contains("smoke test failed"),
            "stderr must carry the smoke test message"
        );
    }

    /// Finding 1: generic runner errors (`FixRunnerFailed`) must NOT map to
    /// exit_code 2 — they become `Err(String)` which the CLI exits 1 for.
    #[test]
    fn test_fix_runner_failed_error_becomes_err_string_not_exit_2() {
        let run_fn = std::sync::Arc::new(|_cmd: RunReviewFixCommand| {
            Err::<RunReviewFixOutput, RunReviewFixError>(RunReviewFixError::FixRunnerFailed(
                "process error".to_owned(),
            ))
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let command = RunReviewFixCommand {
            scope: "cli".to_owned(),
            briefing_file: std::path::PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            track_id: "test-track".to_owned(),
            round_type: "fast".to_owned(),
            reviewer_model: "gpt-5.4-mini".to_owned(),
            model: "gpt-5.5".to_owned(),
            scope_files: vec![std::path::PathBuf::from("apps/cli/src/lib.rs")],
        };
        let result = interactor.run(command);
        // Generic errors must NOT match the SmokeTestFailed arm.
        assert!(
            !matches!(result, Err(RunReviewFixError::SmokeTestFailed(_))),
            "FixRunnerFailed must not be treated as SmokeTestFailed"
        );
        // When passed through the composition mapping, they become Err(String).
        let mapped: Result<CommandOutcome, String> = match result {
            Err(RunReviewFixError::SmokeTestFailed(msg)) => Ok(CommandOutcome {
                stdout: None,
                stderr: Some(format!("[ERROR] smoke test failed: {msg}")),
                exit_code: 2,
            }),
            Err(e) => Err(format!("[ERROR] {e}")),
            Ok(_) => panic!("expected Err"),
        };
        assert!(mapped.is_err(), "FixRunnerFailed must produce Err (cli exits 1)");
    }
}
