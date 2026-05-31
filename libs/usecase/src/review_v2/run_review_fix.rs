//! Run-review-fix application service (usecase layer).
//!
//! Wraps the `ReviewFixRunner` secondary port so the CLI never imports
//! infrastructure types directly (CN-01 / D1).
//! Mirrors the structure of `run_review.rs` and the `Reviewer` port in `ports.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── RunReviewFixCommand ───────────────────────────────────────────────────────

/// CQRS command for the run-review-fix use case (`sotp review fix-local`).
///
/// Carries stdlib-typed fields only (String, PathBuf — no domain / infrastructure
/// types) per AC-01 and CN-01. Maps to the 7 CLI flags:
/// `--scope` / `--briefing-file` / `--track-id` / `--round-type` /
/// `--reviewer-model` / `--model` / `--scope-files`.
/// `round_type` is a plain `String` (converted to `ReviewRoundType` internally
/// by the interactor). The 7-flag shape mirrors the current `Makefile.toml`
/// `track-local-review-fix-codex` bash arguments.
pub struct RunReviewFixCommand {
    pub scope: String,
    pub briefing_file: Option<PathBuf>,
    pub track_id: String,
    pub round_type: String,
    pub reviewer_model: String,
    pub model: String,
    pub scope_files: Vec<PathBuf>,
}

// ── RunReviewFixOutput ────────────────────────────────────────────────────────

/// DTO returned by [`RunReviewFixService`].
///
/// `status` carries the sentinel string from the codex output:
/// `'completed'` | `'blocked_cross_scope'` | `'failed'`.
/// Using `String` (not an enum) keeps the public usecase boundary free of domain
/// types per AC-01 — consistent with `RunReviewOutput.verdict_kind`.
/// `exit_code` maps the sentinel to a CLI exit code
/// (0=completed, 2=blocked_cross_scope, 1=failed).
/// The interactor parses and validates the sentinel before returning.
pub struct RunReviewFixOutput {
    pub status: String,
    pub exit_code: i32,
}

// ── ReviewFixRunnerError ──────────────────────────────────────────────────────

/// Error type for the [`ReviewFixRunner`] secondary port.
///
/// `SmokeTestFailed` covers forbidden sandbox flag or codex version range
/// failures (CN-04). `SpawnFailed` covers codex exec launch failure.
/// `SentinelNotFound` covers the case where no `REVIEW_FIX_STATUS` sentinel
/// was found in the output (AC-08). `Unexpected` wraps any other error.
#[derive(Debug, Error)]
pub enum ReviewFixRunnerError {
    #[error("smoke test failed: {0}")]
    SmokeTestFailed(String),
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("sentinel not found in output: {0}")]
    SentinelNotFound(String),
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

// ── RunReviewFixError ─────────────────────────────────────────────────────────

/// Error type for [`RunReviewFixService`].
///
/// `InvalidScope` / `InvalidTrackId` / `InvalidRoundType` / `EmptyScopeFiles`
/// cover argument validation failures. `SmokeTestFailed` covers forbidden
/// sandbox flag detection or codex version range failure per CN-04.
/// `FixRunnerFailed` wraps the [`ReviewFixRunnerError`] from the port without
/// leaking infrastructure types.
#[derive(Debug, Error)]
pub enum RunReviewFixError {
    #[error("invalid scope: {0}")]
    InvalidScope(String),
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("invalid round type: {0}")]
    InvalidRoundType(String),
    #[error("empty scope_files: {0}")]
    EmptyScopeFiles(String),
    #[error("smoke test failed: {0}")]
    SmokeTestFailed(String),
    #[error("fix runner failed: {0}")]
    FixRunnerFailed(String),
}

// ── ReviewFixRunner ───────────────────────────────────────────────────────────

/// Secondary port for the review-fix-lead fixer.
///
/// Implemented by infrastructure adapters (e.g. `CodexReviewFixRunner`).
/// Accepts [`RunReviewFixCommand`] and returns [`RunReviewFixOutput`] on success,
/// [`ReviewFixRunnerError`] on failure. The usecase interactor drives this port;
/// the infrastructure adapter implements it — mirroring the [`Reviewer`][crate::review_v2::Reviewer] port.
pub trait ReviewFixRunner: Send + Sync {
    /// Runs the review-fix-lead fixer for the given command.
    ///
    /// # Errors
    ///
    /// Returns [`ReviewFixRunnerError`] on smoke-test failure, spawn failure,
    /// missing sentinel, or other unexpected errors.
    fn run_fix(
        &self,
        command: RunReviewFixCommand,
    ) -> Result<RunReviewFixOutput, ReviewFixRunnerError>;
}

// ── RunReviewFixService ───────────────────────────────────────────────────────

/// Application service trait (primary port) for the run-review-fix use case.
///
/// Driven by `apps/cli` via `apps/cli-composition`. The CLI never imports
/// domain or infrastructure types directly — it calls this service through the
/// composition root. Mirrors [`RunReviewService`][crate::review_v2::RunReviewService] in `run_review.rs`.
pub trait RunReviewFixService: Send + Sync {
    /// Runs the review-fix-lead fixer for the given command.
    ///
    /// # Errors
    ///
    /// Returns [`RunReviewFixError`] on argument validation, smoke-test, or
    /// runner failures.
    fn run(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, RunReviewFixError>;
}

// ── RunReviewFixInteractor ────────────────────────────────────────────────────

/// Concrete interactor implementing [`RunReviewFixService`].
///
/// Validates the command fields (`scope` / `track_id` / `round_type` strings)
/// then delegates to the injected [`ReviewFixRunner`] port. Converts
/// [`ReviewFixRunnerError`] to [`RunReviewFixError`] without leaking infra types.
/// The `run_fn` field (function pointer supplied by `cli-composition`) performs
/// the domain+infra wiring — mirroring the `RunReviewInteractor` pattern.
pub struct RunReviewFixInteractor {
    run_fn: Arc<
        dyn Fn(RunReviewFixCommand) -> Result<RunReviewFixOutput, RunReviewFixError> + Send + Sync,
    >,
}

impl RunReviewFixInteractor {
    /// Creates a new interactor with the given run function.
    #[must_use]
    pub fn new(
        run_fn: Arc<
            dyn Fn(RunReviewFixCommand) -> Result<RunReviewFixOutput, RunReviewFixError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl RunReviewFixService for RunReviewFixInteractor {
    fn run(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, RunReviewFixError> {
        // Validate scope (must be non-empty)
        if command.scope.is_empty() {
            return Err(RunReviewFixError::InvalidScope("scope must not be empty".to_owned()));
        }
        // Validate track_id (must be non-empty)
        if command.track_id.is_empty() {
            return Err(RunReviewFixError::InvalidTrackId("track_id must not be empty".to_owned()));
        }
        // Validate round_type (must be "fast" or "final")
        match command.round_type.as_str() {
            "fast" | "final" => {}
            other => {
                return Err(RunReviewFixError::InvalidRoundType(format!(
                    "unknown round type: '{other}' (expected 'fast' or 'final')"
                )));
            }
        }
        // Validate scope_files (must not be empty — an empty boundary is fail-open:
        // the workspace-write fixer would retain full repo write access with only a
        // prompt-level instruction as the sole constraint, which is not a safety
        // boundary.  Reject early so all callers are protected regardless of
        // transport (CLI, test, or future API).
        if command.scope_files.is_empty() {
            return Err(RunReviewFixError::EmptyScopeFiles(
                "scope_files must not be empty: launching a workspace-write fixer without \
                 a concrete file boundary is fail-open (pass --scope-files)"
                    .to_owned(),
            ));
        }
        let out = (self.run_fn)(command)?;
        // Validate the returned DTO: status must be one of the three sentinels,
        // and exit_code must match the canonical mapping (completed=0,
        // blocked_cross_scope=2, failed=1). Mismatched output is surfaced as an
        // error so the boundary never leaks an inconsistent DTO to the caller.
        let expected_exit_code = match out.status.as_str() {
            "completed" => 0,
            "blocked_cross_scope" => 2,
            "failed" => 1,
            other => {
                return Err(RunReviewFixError::FixRunnerFailed(format!(
                    "invalid status sentinel: '{other}' (expected 'completed', \
                     'blocked_cross_scope', or 'failed')"
                )));
            }
        };
        if out.exit_code != expected_exit_code {
            return Err(RunReviewFixError::FixRunnerFailed(format!(
                "exit_code {} does not match status '{}' (expected {})",
                out.exit_code, out.status, expected_exit_code
            )));
        }
        Ok(out)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn make_valid_command() -> RunReviewFixCommand {
        RunReviewFixCommand {
            scope: "domain".to_owned(),
            briefing_file: None,
            track_id: "my-track-2026-05-31".to_owned(),
            round_type: "fast".to_owned(),
            reviewer_model: "o4-mini".to_owned(),
            model: "o4-mini".to_owned(),
            scope_files: vec![std::path::PathBuf::from("libs/domain/src/lib.rs")],
        }
    }

    // ── RunReviewFixError variants ────────────────────────────────────────────

    #[test]
    fn test_run_review_fix_error_invalid_scope_variant_exists() {
        let e = RunReviewFixError::InvalidScope("bad".to_owned());
        assert!(matches!(e, RunReviewFixError::InvalidScope(_)));
    }

    #[test]
    fn test_run_review_fix_error_invalid_track_id_variant_exists() {
        let e = RunReviewFixError::InvalidTrackId("bad".to_owned());
        assert!(matches!(e, RunReviewFixError::InvalidTrackId(_)));
    }

    #[test]
    fn test_run_review_fix_error_invalid_round_type_variant_exists() {
        let e = RunReviewFixError::InvalidRoundType("bad".to_owned());
        assert!(matches!(e, RunReviewFixError::InvalidRoundType(_)));
    }

    #[test]
    fn test_run_review_fix_error_smoke_test_failed_variant_exists() {
        let e = RunReviewFixError::SmokeTestFailed("reason".to_owned());
        assert!(matches!(e, RunReviewFixError::SmokeTestFailed(_)));
    }

    #[test]
    fn test_run_review_fix_error_fix_runner_failed_variant_exists() {
        let e = RunReviewFixError::FixRunnerFailed("reason".to_owned());
        assert!(matches!(e, RunReviewFixError::FixRunnerFailed(_)));
    }

    #[test]
    fn test_run_review_fix_error_empty_scope_files_variant_exists() {
        let e = RunReviewFixError::EmptyScopeFiles("no files".to_owned());
        assert!(matches!(e, RunReviewFixError::EmptyScopeFiles(_)));
    }

    // ── ReviewFixRunnerError variants ─────────────────────────────────────────

    #[test]
    fn test_review_fix_runner_error_smoke_test_failed_variant_exists() {
        let e = ReviewFixRunnerError::SmokeTestFailed("reason".to_owned());
        assert!(matches!(e, ReviewFixRunnerError::SmokeTestFailed(_)));
    }

    #[test]
    fn test_review_fix_runner_error_spawn_failed_variant_exists() {
        let e = ReviewFixRunnerError::SpawnFailed("reason".to_owned());
        assert!(matches!(e, ReviewFixRunnerError::SpawnFailed(_)));
    }

    #[test]
    fn test_review_fix_runner_error_sentinel_not_found_variant_exists() {
        let e = ReviewFixRunnerError::SentinelNotFound("no sentinel found".to_owned());
        assert!(matches!(e, ReviewFixRunnerError::SentinelNotFound(_)));
    }

    #[test]
    fn test_review_fix_runner_error_unexpected_variant_exists() {
        let e = ReviewFixRunnerError::Unexpected("reason".to_owned());
        assert!(matches!(e, ReviewFixRunnerError::Unexpected(_)));
    }

    // ── Interactor validation ─────────────────────────────────────────────────

    #[test]
    fn test_run_review_fix_interactor_empty_scope_returns_invalid_scope_error() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let mut cmd = make_valid_command();
        cmd.scope = String::new();
        match interactor.run(cmd) {
            Err(e) => assert!(matches!(e, RunReviewFixError::InvalidScope(_))),
            Ok(_) => panic!("expected Err(InvalidScope), got Ok"),
        }
    }

    #[test]
    fn test_run_review_fix_interactor_empty_track_id_returns_invalid_track_id_error() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let mut cmd = make_valid_command();
        cmd.track_id = String::new();
        match interactor.run(cmd) {
            Err(e) => assert!(matches!(e, RunReviewFixError::InvalidTrackId(_))),
            Ok(_) => panic!("expected Err(InvalidTrackId), got Ok"),
        }
    }

    #[test]
    fn test_run_review_fix_interactor_unknown_round_type_returns_invalid_round_type_error() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let mut cmd = make_valid_command();
        cmd.round_type = "bad".to_owned();
        match interactor.run(cmd) {
            Err(e) => assert!(matches!(e, RunReviewFixError::InvalidRoundType(_))),
            Ok(_) => panic!("expected Err(InvalidRoundType), got Ok"),
        }
    }

    #[test]
    fn test_run_review_fix_interactor_empty_scope_files_returns_empty_scope_files_error() {
        // Empty scope_files must be rejected: launching a workspace-write fixer
        // without a concrete file boundary is fail-open (CN-03 / Finding 3).
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let mut cmd = make_valid_command();
        cmd.scope_files = vec![];
        match interactor.run(cmd) {
            Err(e) => assert!(matches!(e, RunReviewFixError::EmptyScopeFiles(_))),
            Ok(_) => panic!("expected Err(EmptyScopeFiles), got Ok"),
        }
    }

    #[test]
    fn test_run_review_fix_interactor_nonempty_scope_files_is_accepted() {
        // A command with a non-empty scope_files must pass the boundary validation.
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let out = interactor.run(make_valid_command()).unwrap();
        assert_eq!(out.status, "completed");
    }

    // ── Interactor delegation: completed scenario ─────────────────────────────

    #[test]
    fn test_run_review_fix_interactor_delegates_completed_to_run_fn() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let out = interactor.run(make_valid_command()).unwrap();
        assert_eq!(out.status, "completed");
        assert_eq!(out.exit_code, 0);
    }

    // ── Interactor delegation: blocked_cross_scope scenario ───────────────────

    #[test]
    fn test_run_review_fix_interactor_delegates_blocked_cross_scope_to_run_fn() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "blocked_cross_scope".to_owned(), exit_code: 2 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let mut cmd = make_valid_command();
        cmd.round_type = "final".to_owned();
        let out = interactor.run(cmd).unwrap();
        assert_eq!(out.status, "blocked_cross_scope");
        assert_eq!(out.exit_code, 2);
    }

    // ── Interactor delegation: failed scenario ────────────────────────────────

    #[test]
    fn test_run_review_fix_interactor_delegates_failed_to_run_fn() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "failed".to_owned(), exit_code: 1 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        let out = interactor.run(make_valid_command()).unwrap();
        assert_eq!(out.status, "failed");
        assert_eq!(out.exit_code, 1);
    }

    // ── Interactor delegation: run_fn error propagation ──────────────────────

    #[test]
    fn test_run_review_fix_interactor_propagates_run_fn_error() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Err(RunReviewFixError::FixRunnerFailed("runner error".to_owned()))
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        match interactor.run(make_valid_command()) {
            Err(e) => assert!(matches!(e, RunReviewFixError::FixRunnerFailed(_))),
            Ok(_) => panic!("expected Err(FixRunnerFailed), got Ok"),
        }
    }

    // ── Interactor output validation: invalid sentinel ────────────────────────

    #[test]
    fn test_run_review_fix_interactor_invalid_status_sentinel_returns_fix_runner_failed() {
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "unknown_sentinel".to_owned(), exit_code: 99 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        match interactor.run(make_valid_command()) {
            Err(e) => assert!(matches!(e, RunReviewFixError::FixRunnerFailed(_))),
            Ok(_) => panic!("expected Err(FixRunnerFailed) for invalid sentinel, got Ok"),
        }
    }

    // ── Interactor output validation: mismatched exit_code ───────────────────

    #[test]
    fn test_run_review_fix_interactor_mismatched_exit_code_returns_fix_runner_failed() {
        // "completed" maps to exit_code=0; returning 2 must be rejected.
        let run_fn = Arc::new(|_cmd: RunReviewFixCommand| {
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 2 })
        });
        let interactor = RunReviewFixInteractor::new(run_fn);
        match interactor.run(make_valid_command()) {
            Err(e) => assert!(matches!(e, RunReviewFixError::FixRunnerFailed(_))),
            Ok(_) => panic!("expected Err(FixRunnerFailed) for mismatched exit_code, got Ok"),
        }
    }

    // ── ReviewFixRunner mock: port contract ───────────────────────────────────

    struct MockReviewFixRunner {
        result: Result<RunReviewFixOutput, ReviewFixRunnerError>,
    }

    impl MockReviewFixRunner {
        fn returning(result: Result<RunReviewFixOutput, ReviewFixRunnerError>) -> Self {
            Self { result }
        }
    }

    impl ReviewFixRunner for MockReviewFixRunner {
        fn run_fix(
            &self,
            _command: RunReviewFixCommand,
        ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
            match &self.result {
                Ok(out) => {
                    Ok(RunReviewFixOutput { status: out.status.clone(), exit_code: out.exit_code })
                }
                Err(e) => Err(match e {
                    ReviewFixRunnerError::SmokeTestFailed(s) => {
                        ReviewFixRunnerError::SmokeTestFailed(s.clone())
                    }
                    ReviewFixRunnerError::SpawnFailed(s) => {
                        ReviewFixRunnerError::SpawnFailed(s.clone())
                    }
                    ReviewFixRunnerError::SentinelNotFound(s) => {
                        ReviewFixRunnerError::SentinelNotFound(s.clone())
                    }
                    ReviewFixRunnerError::Unexpected(s) => {
                        ReviewFixRunnerError::Unexpected(s.clone())
                    }
                }),
            }
        }
    }

    #[test]
    fn test_review_fix_runner_mock_completed_scenario() {
        let runner = MockReviewFixRunner::returning(Ok(RunReviewFixOutput {
            status: "completed".to_owned(),
            exit_code: 0,
        }));
        let out = runner.run_fix(make_valid_command()).unwrap();
        assert_eq!(out.status, "completed");
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn test_review_fix_runner_mock_blocked_cross_scope_scenario() {
        let runner = MockReviewFixRunner::returning(Ok(RunReviewFixOutput {
            status: "blocked_cross_scope".to_owned(),
            exit_code: 2,
        }));
        let out = runner.run_fix(make_valid_command()).unwrap();
        assert_eq!(out.status, "blocked_cross_scope");
        assert_eq!(out.exit_code, 2);
    }

    #[test]
    fn test_review_fix_runner_mock_failed_scenario() {
        let runner = MockReviewFixRunner::returning(Ok(RunReviewFixOutput {
            status: "failed".to_owned(),
            exit_code: 1,
        }));
        let out = runner.run_fix(make_valid_command()).unwrap();
        assert_eq!(out.status, "failed");
        assert_eq!(out.exit_code, 1);
    }

    #[test]
    fn test_review_fix_runner_mock_sentinel_not_found_scenario() {
        let runner = MockReviewFixRunner::returning(Err(ReviewFixRunnerError::SentinelNotFound(
            "no sentinel".to_owned(),
        )));
        match runner.run_fix(make_valid_command()) {
            Err(e) => assert!(matches!(e, ReviewFixRunnerError::SentinelNotFound(_))),
            Ok(_) => panic!("expected Err(SentinelNotFound), got Ok"),
        }
    }
}
