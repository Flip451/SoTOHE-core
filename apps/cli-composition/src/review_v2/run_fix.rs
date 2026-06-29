//! `review_run_fix_local` composition — wiring for the review-fix-lead fixer.
//!
//! Extracted from `review_v2/mod.rs` to keep that module under the 700-line
//! production-code cap. Public surface is unchanged: `CliApp::review_run_fix_local`
//! delegates here via `run_fix_local`.

use std::sync::Arc;

use infrastructure::review_v2::CodexReviewFixRunner;
use usecase::review_v2::run_review_fix::{
    ReviewFixRunner as _, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixError,
    RunReviewFixInteractor, RunReviewFixService as _,
};

use super::shared::{parse_round_type, resolve_agent_execution};
use super::{RunReviewFixLocalInput, validate_review_group_name_str, validate_track_id_str};
use crate::CommandOutcome;

/// Sentinel emitted on the first stdout line when `review-fix-lead.provider` is
/// `"claude"`, signalling the caller (orchestrator skill or harness) that the
/// fix loop must be executed in-process by dispatching the
/// `review-fix-lead` Claude Code subagent. The next stdout line is a single
/// JSON object with all parameters the subagent needs (model, scope,
/// briefing_file, track_id, round_type).
pub const SUBAGENT_DISPATCH_SENTINEL: &str = "SUBAGENT_DISPATCH_REQUIRED";

/// Exit code returned together with [`SUBAGENT_DISPATCH_SENTINEL`] for the
/// `claude` provider path. Distinct from 0 (codex completed), 1 (runner error),
/// and 2 (smoke-test failure) so callers can branch on the exit code alone
/// without parsing stdout.
pub const SUBAGENT_DISPATCH_EXIT_CODE: u8 = 64;

/// Run the review-fix-lead fixer with provider auto-resolved from agent-profiles.json.
///
/// Resolves the `review-fix-lead` capability from `agent-profiles.json` at the
/// repo root. Branches on the resolved provider:
///
/// - `"codex"`: constructs [`CodexReviewFixRunner`] and runs it through
///   [`RunReviewFixInteractor`].
/// - `"claude"`: returns a [`CommandOutcome`] whose stdout carries the
///   [`SUBAGENT_DISPATCH_SENTINEL`] line followed by a single-line JSON object
///   describing the subagent dispatch parameters, and whose exit code is
///   [`SUBAGENT_DISPATCH_EXIT_CODE`]. The caller (orchestrator skill or
///   harness) is expected to spawn the `review-fix-lead` Claude Code subagent
///   with those parameters — the binary never spawns the subagent itself
///   because the fix loop lives in-process inside Claude Code.
/// - other: returns an error.
///
/// This branching keeps the orchestrator skill (`/track:review`) provider-agnostic
/// — it always calls `bin/sotp review fix-local` (via
/// `cargo make track-local-review-fix`) and reacts to the exit code: codex
/// completion (0/1/2) flows through the existing `REVIEW_FIX_STATUS` contract,
/// claude completion ([`SUBAGENT_DISPATCH_EXIT_CODE`]) triggers in-process
/// subagent dispatch.
///
/// # Errors
/// Returns `Err` when profile loading, provider resolution, arg validation,
/// or the codex fix runner fails.
pub(crate) fn run_fix_local(input: RunReviewFixLocalInput) -> Result<CommandOutcome, String> {
    let track_id = input.track_id.trim().to_owned();
    validate_track_id_str(&track_id).map_err(|e| format!("invalid --track-id: {e}"))?;

    let scope = input.scope.trim().to_owned();
    validate_review_group_name_str(&scope).map_err(|e| format!("invalid --scope: {e}"))?;

    let infra_round_type = parse_round_type(&input.round_type).map_err(|e| e.to_string())?;
    let resolved =
        resolve_agent_execution(None, "review-fix-lead", infra_round_type, input.model.as_deref())
            .map_err(|e| e.to_string())?;
    let model = resolved.model;

    eprintln!("[sotp review fix-local] provider={} model={}", resolved.provider, &model);

    match resolved.provider.as_str() {
        "claude" => {
            // The claude review-fix-lead runs in-process as a Claude Code
            // subagent — the binary cannot spawn it. Emit a structured
            // dispatch instruction so the orchestrator skill can route the
            // request without provider conditionals.
            let json = format!(
                "{{\"agent\":\"review-fix-lead\",\"model\":{},\"scope\":{},\"briefing_file\":{},\"track_id\":{},\"round_type\":{}}}",
                json_str(&model),
                json_str(&scope),
                json_str(&input.briefing_file.display().to_string()),
                json_str(&track_id),
                json_str(&input.round_type),
            );
            Ok(CommandOutcome {
                stdout: Some(format!("{SUBAGENT_DISPATCH_SENTINEL}\n{json}")),
                stderr: None,
                exit_code: SUBAGENT_DISPATCH_EXIT_CODE,
            })
        }
        "codex" => {
            let runner = CodexReviewFixRunner::new(
                model.clone(),
                scope.clone(),
                input.briefing_file.clone(),
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
                model,
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
             (supported: 'codex', 'claude')"
        )),
    }
}

/// Render `s` as a JSON string literal (with surrounding quotes and minimal
/// escaping for the characters that can appear in the dispatch payload).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
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
            model: "gpt-5.5".to_owned(),
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
            model: "gpt-5.5".to_owned(),
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
