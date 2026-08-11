//! Run-review-fix application service (usecase layer).
//!
//! Wraps the `ReviewFixRunner` secondary port so the CLI never imports
//! infrastructure types directly (CN-01 / D1).
//! Mirrors the structure of `run_review.rs` and the `Reviewer` port in `ports.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use domain::TrackId;

use crate::capability_exec::{ModelName, ReasoningEffort};
use crate::git_workflow::DiagnosticText;
use crate::review_v2::{ReviewRoundType, ReviewScopeName, ReviewScopeNameValidationError};

// ── RunReviewFixCommand ───────────────────────────────────────────────────────

/// CQRS command for the run-review-fix use case (`sotp review fix-local`).
///
/// Carries values validated at its own usecase boundary. Maps to the 4 CLI flags:
/// `--scope` / `--briefing-file` / `--track-id` / `--round-type`.
/// `--reviewer-model` and `--scope-files` are removed: the fixer skill
/// self-resolves the reviewer model from `agent-profiles.json` and the scope
/// boundary via `bin/sotp review files --scope <scope>` (ADR 2026-06-01-2300
/// D1/D3). `round_type` is a plain `String` (converted to `ReviewRoundType`
/// directly by the CLI). The `model` field covers the fixer's own optional
/// model override.
#[derive(Clone)]
pub struct RunReviewFixCommand {
    scope: String,
    briefing_content: SubagentBriefingContent,
    track_id: String,
    repository_root: PathBuf,
    round_type: ReviewRoundType,
    model: Option<ModelName>,
}

impl RunReviewFixCommand {
    /// Creates a command from values already validated at the usecase boundary.
    #[must_use]
    pub fn new_resolved(
        scope: ReviewScopeName,
        briefing_content: SubagentBriefingContent,
        resolution: ReviewFixResolution,
        round_type: ReviewRoundType,
        model: Option<ModelName>,
    ) -> Self {
        Self {
            scope: scope.as_str().to_owned(),
            briefing_content,
            track_id: resolution.track_id.as_str().to_owned(),
            repository_root: resolution.repository_root,
            round_type,
            model,
        }
    }

    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    #[must_use]
    pub fn briefing_content(&self) -> &SubagentBriefingContent {
        &self.briefing_content
    }

    #[must_use]
    pub fn track_id(&self) -> &str {
        &self.track_id
    }

    #[must_use]
    pub fn repository_root(&self) -> &std::path::Path {
        &self.repository_root
    }

    #[must_use]
    pub fn round_type(&self) -> &ReviewRoundType {
        &self.round_type
    }

    #[must_use]
    pub fn model(&self) -> Option<&ModelName> {
        self.model.as_ref()
    }
}

/// Usecase-owned review-track boundary value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTrackId(String);

impl ReviewTrackId {
    pub fn try_new(value: String) -> Result<Self, ReviewTrackIdValidationError> {
        TrackId::try_new(value.clone()).map_err(|error| {
            ReviewTrackIdValidationError::Invalid(diagnostic_message(error.to_string()))
        })?;
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentBriefingContent(String);

impl SubagentBriefingContent {
    pub fn try_new(value: String) -> Result<Self, SubagentBriefingContentValidationError> {
        if value.len() > 64 * 1024 {
            return Err(SubagentBriefingContentValidationError::ExceedsMaximumBytes);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum SubagentBriefingContentValidationError {
    #[error("review-fix briefing content exceeds the 65536-byte limit")]
    ExceedsMaximumBytes,
}

/// Resolver-proven association between the active track and its repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewFixResolution {
    track_id: ReviewTrackId,
    repository_root: PathBuf,
}

impl ReviewFixResolution {
    #[must_use]
    pub fn new(track_id: ReviewTrackId, repository_root: PathBuf) -> Self {
        Self { track_id, repository_root }
    }

    #[must_use]
    pub fn track_id(&self) -> &ReviewTrackId {
        &self.track_id
    }

    #[must_use]
    pub fn repository_root(&self) -> &std::path::Path {
        &self.repository_root
    }
}

#[derive(Debug, Error)]
pub enum ReviewTrackIdValidationError {
    #[error("invalid review track ID: {}", .0.as_str())]
    Invalid(DiagnosticText),
}

/// Validated delivery request waiting for the current branch to be resolved.
pub struct RunReviewFixRequest {
    scope: ReviewScopeName,
    briefing_file: PathBuf,
    explicit_track_id: Option<ReviewTrackId>,
    items_dir: PathBuf,
    round_type: ReviewRoundType,
    model: Option<ModelName>,
}

impl RunReviewFixRequest {
    pub fn try_new(
        scope: String,
        briefing_file: PathBuf,
        explicit_track_id: Option<String>,
        items_dir: PathBuf,
        round_type: String,
        model: Option<String>,
    ) -> Result<Self, RunReviewFixCommandValidationError> {
        let scope = ReviewScopeName::try_new(scope).map_err(|error| match error {
            ReviewScopeNameValidationError::Invalid(detail) => {
                RunReviewFixCommandValidationError::InvalidScope(detail)
            }
        })?;
        let explicit_track_id = explicit_track_id.map(ReviewTrackId::try_new).transpose().map_err(
            |error| match error {
                ReviewTrackIdValidationError::Invalid(detail) => {
                    RunReviewFixCommandValidationError::InvalidTrackId(detail)
                }
            },
        )?;
        let round_type = ReviewRoundType::parse(&round_type).map_err(|error| {
            RunReviewFixCommandValidationError::InvalidRoundType(diagnostic_message(
                error.to_string(),
            ))
        })?;
        let model = model.map(ModelName::try_new).transpose().map_err(|error| {
            RunReviewFixCommandValidationError::InvalidModel(diagnostic_message(error.to_string()))
        })?;
        Ok(Self { scope, briefing_file, explicit_track_id, items_dir, round_type, model })
    }
}

/// Rejection returned while constructing a [`RunReviewFixCommand`].
#[derive(Debug, Error)]
pub enum RunReviewFixCommandValidationError {
    #[error("invalid review-fix scope: {}", .0.as_str())]
    InvalidScope(DiagnosticText),
    #[error("invalid review-fix track ID: {}", .0.as_str())]
    InvalidTrackId(DiagnosticText),
    #[error("invalid review-fix round type: {}", .0.as_str())]
    InvalidRoundType(DiagnosticText),
    #[error("invalid review-fix model: {}", .0.as_str())]
    InvalidModel(DiagnosticText),
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
    /// Optional diagnostic message to surface on stderr when the run is blocked
    /// or failed (e.g., smoke-test failure detail). Empty when the run completed
    /// successfully.
    pub stderr: Option<String>,
}

// ── ReviewFixRunnerError ──────────────────────────────────────────────────────

/// Error type for the [`ReviewFixRunner`] secondary port.
///
/// `SmokeTestFailed` covers forbidden sandbox flag or codex version range
/// failures (CN-04). `SpawnFailed` covers codex exec launch failure.
/// `SentinelNotFound` covers the case where no `REVIEW_FIX_STATUS` sentinel
/// was found in the output (AC-08). `SubagentDispatchRequired` carries an
/// in-host delegation request without losing its typed payload. `Unexpected`
/// wraps any other error.
#[derive(Debug, Error)]
pub enum ReviewFixRunnerError {
    #[error("smoke test failed: {0}")]
    SmokeTestFailed(DiagnosticText),
    #[error("spawn failed: {0}")]
    SpawnFailed(DiagnosticText),
    #[error("sentinel not found in output: {0}")]
    SentinelNotFound(DiagnosticText),
    #[error("external review-fix runner dispatch required")]
    SubagentDispatchRequired(Box<SubagentDispatchInstruction>),
    #[error("unexpected error: {0}")]
    Unexpected(DiagnosticText),
}

// ── RunReviewFixError ─────────────────────────────────────────────────────────

/// Validated name of an in-host review-fix subagent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentName(domain::NonEmptyString);

impl SubagentName {
    /// Validates and wraps an in-host review-fix subagent name.
    ///
    /// # Errors
    ///
    /// Returns [`SubagentNameValidationError`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: String) -> Result<Self, SubagentNameValidationError> {
        domain::NonEmptyString::try_new(value).map(Self).map_err(|error| {
            SubagentNameValidationError::Invalid(diagnostic_message(error.to_string()))
        })
    }

    /// Returns the validated subagent name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

/// Usecase-owned validation error for [`SubagentName`].
#[derive(Debug, Error)]
pub enum SubagentNameValidationError {
    #[error("invalid subagent name: {}", .0.as_str())]
    Invalid(DiagnosticText),
}

impl std::fmt::Display for SubagentName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Typed instruction for an external review-fix runner dispatch.
///
/// The usecase transports provider-neutral values only. The CLI driver owns
/// rendering this instruction for the orchestrator-facing stdout protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentDispatchInstruction {
    pub agent: SubagentName,
    pub model: ModelName,
    pub effort: ReasoningEffort,
    pub scope: ReviewScopeName,
    pub briefing_content: SubagentBriefingContent,
    pub track_id: ReviewTrackId,
    /// Resolver-proven repository root for the in-host fixer dispatch.
    pub repository_root: PathBuf,
    pub round_type: ReviewRoundType,
}

/// Error type for [`RunReviewFixService`].
///
/// `SubagentDispatchRequired` signals that the request must be delegated to an
/// external review-fix runner. The typed payload is rendered only by the CLI
/// driver, which owns the external stdout protocol.
///
/// Raw argument validation failures are owned by [`RunReviewFixCommandValidationError`].
/// `FixRunnerFailed` wraps every [`ReviewFixRunnerError`] from the port,
/// including smoke-test and subagent-dispatch outcomes. `EmptyScopeFiles` is
/// removed — the fixer skill self-resolves the scope boundary (ADR
/// 2026-06-01-2300 D1).
#[derive(Debug, Error)]
pub enum RunReviewFixError {
    #[error("fix runner failed: {0}")]
    FixRunnerFailed(ReviewFixRunnerError),
    #[error("track resolution failed: {0}")]
    TrackResolution(#[from] ReviewFixTrackResolveError),
    #[error("briefing load failed: {0}")]
    BriefingLoad(#[from] ReviewFixBriefingLoadError),
    #[error("explicit track '{}' does not match current branch track '{}'", explicit.as_str(), resolved.as_str())]
    TrackMismatch { explicit: ReviewTrackId, resolved: ReviewTrackId },
}

/// Builds a non-empty diagnostic payload for [`RunReviewFixError`].
#[must_use]
fn diagnostic_message(value: impl Into<String>) -> DiagnosticText {
    let mut value = value.into();
    if value.trim().is_empty() {
        value = "review-fix diagnostic detail unavailable".to_owned();
    }
    DiagnosticText::new(value)
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

pub trait ReviewFixTrackResolverPort: Send + Sync {
    fn resolve_current_track(
        &self,
        items_dir: &Path,
    ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError>;
}

#[derive(Debug, Error)]
pub enum ReviewFixTrackResolveError {
    #[error("could not read current branch: {}", .0.as_str())]
    BranchReadFailed(DiagnosticText),
    #[error("current branch is not a track branch: {}", .0.as_str())]
    NonTrackBranch(DiagnosticText),
}

#[derive(Debug, Error)]
pub enum ReviewFixBriefingLoadError {
    #[error("review-fix briefing file is not trusted: {0}")]
    UntrustedFile(DiagnosticText),
    #[error("could not read review-fix briefing: {0}")]
    ReadFailed(DiagnosticText),
    #[error("review-fix briefing content is invalid: {0}")]
    InvalidContent(SubagentBriefingContentValidationError),
}

pub trait ReviewFixBriefingLoaderPort: Send + Sync {
    fn load_briefing_content(
        &self,
        repository_root: &Path,
        briefing_file: &Path,
    ) -> Result<SubagentBriefingContent, ReviewFixBriefingLoadError>;
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
    /// Returns [`RunReviewFixError`] on argument validation, track resolution,
    /// or runner failures.
    fn run(&self, request: RunReviewFixRequest) -> Result<RunReviewFixOutput, RunReviewFixError>;
}

// ── RunReviewFixInteractor ────────────────────────────────────────────────────

/// Concrete interactor implementing [`RunReviewFixService`].
///
/// Delegates the validated command to the injected [`ReviewFixRunner`] port. Converts
/// [`ReviewFixRunnerError`] to [`RunReviewFixError`] without leaking infra types.
/// The `run_fn` field (function pointer supplied by `cli-composition`) performs
/// the domain+infra wiring — mirroring the `RunReviewInteractor` pattern.
pub struct RunReviewFixInteractor {
    track_resolver: Arc<dyn ReviewFixTrackResolverPort>,
    briefing_loader: Arc<dyn ReviewFixBriefingLoaderPort>,
    runner: Arc<dyn ReviewFixRunner>,
}

impl RunReviewFixInteractor {
    /// Creates a new interactor with the given run function.
    #[must_use]
    pub fn new(
        track_resolver: Arc<dyn ReviewFixTrackResolverPort>,
        briefing_loader: Arc<dyn ReviewFixBriefingLoaderPort>,
        runner: Arc<dyn ReviewFixRunner>,
    ) -> Self {
        Self { track_resolver, briefing_loader, runner }
    }
}

impl RunReviewFixService for RunReviewFixInteractor {
    fn run(&self, request: RunReviewFixRequest) -> Result<RunReviewFixOutput, RunReviewFixError> {
        let resolved = self
            .track_resolver
            .resolve_current_track(&request.items_dir)
            .map_err(RunReviewFixError::TrackResolution)?;
        if let Some(explicit) = request.explicit_track_id {
            if explicit != *resolved.track_id() {
                return Err(RunReviewFixError::TrackMismatch {
                    explicit,
                    resolved: resolved.track_id().clone(),
                });
            }
        }
        let briefing_content = self
            .briefing_loader
            .load_briefing_content(resolved.repository_root(), &request.briefing_file)
            .map_err(RunReviewFixError::BriefingLoad)?;
        let command = RunReviewFixCommand::new_resolved(
            request.scope,
            briefing_content,
            resolved,
            request.round_type,
            request.model,
        );
        let out = self.runner.run_fix(command).map_err(RunReviewFixError::FixRunnerFailed)?;
        // Validate the returned DTO: status must be one of the three sentinels,
        // and exit_code must match the canonical mapping (completed=0,
        // blocked_cross_scope=2, failed=1). Mismatched output is surfaced as an
        // error so the boundary never leaks an inconsistent DTO to the caller.
        let expected_exit_code = match out.status.as_str() {
            "completed" => 0,
            "blocked_cross_scope" => 2,
            "failed" => 1,
            other => {
                return Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(
                    diagnostic_message(format!(
                        "invalid status sentinel: '{other}' (expected 'completed', \
                         'blocked_cross_scope', or 'failed')"
                    )),
                )));
            }
        };
        if out.exit_code != expected_exit_code {
            return Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(
                diagnostic_message(format!(
                    "exit_code {} does not match status '{}' (expected {})",
                    out.exit_code, out.status, expected_exit_code
                )),
            )));
        }
        Ok(out)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn make_valid_request() -> RunReviewFixRequest {
        RunReviewFixRequest::try_new(
            "domain".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            None,
            PathBuf::from("track/items"),
            "fast".to_owned(),
            Some("o4-mini".to_owned()),
        )
        .expect("valid review-fix request")
    }

    fn make_valid_command() -> RunReviewFixCommand {
        RunReviewFixCommand::new_resolved(
            ReviewScopeName::try_new("domain".to_owned()).expect("valid test scope"),
            SubagentBriefingContent::try_new("briefing".to_owned()).expect("valid briefing"),
            ReviewFixResolution::new(
                ReviewTrackId::try_new("my-track-2026-05-31".to_owned())
                    .expect("valid test track ID"),
                PathBuf::from("."),
            ),
            ReviewRoundType::Fast,
            Some(ModelName::try_new("o4-mini").expect("valid test model")),
        )
    }

    #[test]
    fn test_subagent_briefing_content_accepts_limit_and_rejects_over_bound() {
        let maximum = "x".repeat(64 * 1024);

        let content = SubagentBriefingContent::try_new(maximum.clone())
            .expect("the 65536-byte limit must be accepted");
        assert_eq!(content.as_str(), maximum);
        assert!(matches!(
            SubagentBriefingContent::try_new("x".repeat(64 * 1024 + 1)),
            Err(SubagentBriefingContentValidationError::ExceedsMaximumBytes)
        ));
    }

    #[test]
    fn test_run_review_fix_command_new_resolved_preserves_validated_values() {
        let command = RunReviewFixCommand::new_resolved(
            ReviewScopeName::try_new("cli_driver".to_owned()).expect("valid test scope"),
            SubagentBriefingContent::try_new("briefing".to_owned()).expect("valid briefing"),
            ReviewFixResolution::new(
                ReviewTrackId::try_new("review-fix-command-2026".to_owned())
                    .expect("valid test track ID"),
                PathBuf::from("/test-resolved-root"),
            ),
            ReviewRoundType::Final,
            Some(ModelName::try_new("gpt-5.5").expect("valid test model")),
        );

        assert_eq!(command.scope(), "cli_driver");
        assert_eq!(command.briefing_content().as_str(), "briefing");
        assert_eq!(command.track_id(), "review-fix-command-2026");
        assert_eq!(command.repository_root(), PathBuf::from("/test-resolved-root"));
        assert!(matches!(command.round_type(), ReviewRoundType::Final));
        assert_eq!(command.model().map(ModelName::as_str), Some("gpt-5.5"));
    }

    #[test]
    fn test_run_review_fix_request_try_new_preserves_raw_values_and_accepts_other_catch_all() {
        let request = RunReviewFixRequest::try_new(
            "  cli_driver  ".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("review-fix-command-2026".to_owned()),
            PathBuf::from("track/items"),
            "final".to_owned(),
            Some("gpt-5.5".to_owned()),
        )
        .expect("valid raw review-fix values must construct a request");
        assert_eq!(request.scope.as_str(), "  cli_driver  ");
        assert_eq!(
            request.explicit_track_id.as_ref().map(ReviewTrackId::as_str),
            Some("review-fix-command-2026")
        );
        assert!(matches!(request.round_type, ReviewRoundType::Final));

        for supplied_scope in ["other", "Other"] {
            let other = RunReviewFixRequest::try_new(
                supplied_scope.to_owned(),
                PathBuf::from("tmp/reviewer-runtime/briefing.md"),
                None,
                PathBuf::from("track/items"),
                "fast".to_owned(),
                None,
            )
            .expect("case-insensitive catch-all scope must not be a validation error");
            assert_eq!(other.scope.as_str(), "other", "{supplied_scope} selects Other");
        }
    }

    #[test]
    fn test_run_review_fix_request_construction_validates_raw_delivery_boundary() {
        let request = RunReviewFixRequest::try_new(
            "Other".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            Some("review-fix-boundary-2026".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            Some("gpt-5.5".to_owned()),
        )
        .expect("valid raw delivery values must construct a request");

        assert_eq!(request.scope.as_str(), "other");
        assert_eq!(request.briefing_file, PathBuf::from("tmp/reviewer-runtime/briefing.md"));
        assert_eq!(
            request.explicit_track_id.as_ref().map(ReviewTrackId::as_str),
            Some("review-fix-boundary-2026")
        );
        assert_eq!(request.items_dir, PathBuf::from("track/items"));
        assert!(matches!(request.round_type, ReviewRoundType::Fast));
        assert_eq!(request.model.as_ref().map(ModelName::as_str), Some("gpt-5.5"));

        let cases = [
            ("", Some("review-fix-boundary-2026"), "fast", "invalid review-fix scope"),
            ("非ASCII", Some("review-fix-boundary-2026"), "fast", "invalid review-fix scope"),
            ("cli_driver", Some("Invalid Track ID"), "fast", "invalid review-fix track ID"),
            (
                "cli_driver",
                Some("review-fix-boundary-2026"),
                "unknown",
                "invalid review-fix round type",
            ),
        ];
        for (scope, track_id, round_type, expected_field) in cases {
            let error = match RunReviewFixRequest::try_new(
                scope.to_owned(),
                PathBuf::from("briefing.md"),
                track_id.map(str::to_owned),
                PathBuf::from("track/items"),
                round_type.to_owned(),
                None,
            ) {
                Ok(_) => {
                    panic!("invalid raw delivery values must be rejected at request construction")
                }
                Err(error) => error,
            };
            assert!(
                error.to_string().contains(expected_field),
                "error must identify the invalid delivery field: {error}"
            );
        }
    }

    #[test]
    fn test_run_review_fix_request_try_new_rejects_full_validation_matrix() {
        let cases = [
            (
                "empty scope",
                "".to_owned(),
                Some("review-fix-command-2026".to_owned()),
                "fast".to_owned(),
                "invalid review-fix scope",
            ),
            (
                "non-ASCII scope",
                "非ASCII".to_owned(),
                Some("review-fix-command-2026".to_owned()),
                "fast".to_owned(),
                "invalid review-fix scope",
            ),
            (
                "track ID outside the domain invariant",
                "cli_driver".to_owned(),
                Some("Invalid Track ID".to_owned()),
                "fast".to_owned(),
                "invalid review-fix track ID",
            ),
            (
                "empty explicit track ID",
                "cli_driver".to_owned(),
                Some(String::new()),
                "fast".to_owned(),
                "invalid review-fix track ID",
            ),
            (
                "unknown round type",
                "cli_driver".to_owned(),
                Some("review-fix-command-2026".to_owned()),
                "later".to_owned(),
                "invalid review-fix round type",
            ),
        ];

        for (case, scope, track_id, round_type, expected_field) in cases {
            let error = match RunReviewFixRequest::try_new(
                scope,
                PathBuf::from("briefing.md"),
                track_id,
                PathBuf::from("track/items"),
                round_type,
                None,
            ) {
                Err(error) => error,
                Ok(_) => panic!("{case} must be rejected"),
            };

            assert!(
                error.to_string().contains(expected_field),
                "{case} must identify its invalid field: {error}"
            );
            match expected_field {
                "invalid review-fix scope" => {
                    assert!(matches!(error, RunReviewFixCommandValidationError::InvalidScope(_)));
                }
                "invalid review-fix track ID" => {
                    assert!(matches!(error, RunReviewFixCommandValidationError::InvalidTrackId(_)));
                }
                "invalid review-fix round type" => {
                    assert!(matches!(
                        error,
                        RunReviewFixCommandValidationError::InvalidRoundType(_)
                    ));
                }
                _ => panic!("test case must declare a known validation field"),
            }
        }
    }

    #[test]
    fn test_run_review_fix_command_try_new_rejects_invalid_track_id() {
        let result = RunReviewFixRequest::try_new(
            "cli_driver".to_owned(),
            PathBuf::from("briefing.md"),
            Some("Invalid Track ID".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            None,
        );

        assert!(matches!(result, Err(RunReviewFixCommandValidationError::InvalidTrackId(_))));
    }

    #[test]
    fn test_review_track_id_rejects_non_domain_slug_forms() {
        for invalid in ["", "Uppercase-track", "track_name", "track--name", "track-name-"] {
            let result = ReviewTrackId::try_new(invalid.to_owned());
            assert!(
                matches!(result, Err(ReviewTrackIdValidationError::Invalid(_))),
                "{invalid} must be rejected by the domain track-ID invariant"
            );
        }
    }

    #[test]
    fn test_review_track_id_try_new_invalid_input_returns_diagnostic_error() {
        let error = ReviewTrackId::try_new("Invalid Track ID".to_owned())
            .expect_err("an invalid raw track ID must be rejected at the usecase boundary");

        match error {
            ReviewTrackIdValidationError::Invalid(detail) => {
                assert!(
                    !detail.as_str().is_empty(),
                    "the declared Invalid variant must retain diagnostic context"
                );
            }
        }
    }

    #[test]
    fn test_run_review_fix_command_try_new_rejects_invalid_round_type() {
        let result = RunReviewFixRequest::try_new(
            "cli_driver".to_owned(),
            PathBuf::from("briefing.md"),
            Some("review-fix-command-2026".to_owned()),
            PathBuf::from("track/items"),
            "later".to_owned(),
            None,
        );

        assert!(matches!(result, Err(RunReviewFixCommandValidationError::InvalidRoundType(_))));
    }

    #[test]
    fn test_run_review_fix_command_try_new_rejects_invalid_model() {
        let result = RunReviewFixRequest::try_new(
            "cli_driver".to_owned(),
            PathBuf::from("briefing.md"),
            Some("review-fix-command-2026".to_owned()),
            PathBuf::from("track/items"),
            "fast".to_owned(),
            Some(String::new()),
        );

        assert!(matches!(result, Err(RunReviewFixCommandValidationError::InvalidModel(_))));
    }

    // ── RunReviewFixError variants ────────────────────────────────────────────

    #[test]
    fn test_run_review_fix_error_fix_runner_failed_variant_exists() {
        let e = RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::SpawnFailed(
            diagnostic_message("reason"),
        ));
        assert!(matches!(
            e,
            RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::SpawnFailed(_))
        ));
    }

    #[test]
    fn test_run_review_fix_error_diagnostic_payload_display_preserves_text() {
        let cases = [
            (
                RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::SmokeTestFailed(
                    diagnostic_message("smoke detail"),
                )),
                "fix runner failed: smoke test failed: smoke detail",
            ),
            (
                RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(
                    diagnostic_message("runner detail"),
                )),
                "fix runner failed: unexpected error: runner detail",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn test_review_fix_briefing_load_error_preserves_each_failure_category() {
        let untrusted = ReviewFixBriefingLoadError::UntrustedFile(DiagnosticText::new("symlink"));
        let read_failed = ReviewFixBriefingLoadError::ReadFailed(DiagnosticText::new("missing"));
        let invalid = ReviewFixBriefingLoadError::InvalidContent(
            SubagentBriefingContentValidationError::ExceedsMaximumBytes,
        );

        assert!(untrusted.to_string().contains("not trusted: symlink"));
        assert!(read_failed.to_string().contains("could not read review-fix briefing: missing"));
        assert!(invalid.to_string().contains("content is invalid"));
    }

    #[test]
    fn test_run_review_fix_error_subagent_dispatch_required_retains_typed_instruction() {
        let instruction = SubagentDispatchInstruction {
            agent: SubagentName::try_new("review-fix-lead".to_owned())
                .expect("valid test subagent"),
            model: ModelName::try_new("claude-opus").expect("valid test model"),
            effort: ReasoningEffort::Low,
            scope: ReviewScopeName::try_new("cli_driver".to_owned())
                .expect("valid test review scope"),
            briefing_content: SubagentBriefingContent::try_new("briefing".to_owned())
                .expect("valid briefing"),
            track_id: ReviewTrackId::try_new("dispatch-render-2026".to_owned())
                .expect("valid test track ID"),
            repository_root: PathBuf::from("/resolved/repository"),
            round_type: ReviewRoundType::Fast,
        };
        let error = RunReviewFixError::FixRunnerFailed(
            ReviewFixRunnerError::SubagentDispatchRequired(Box::new(instruction.clone())),
        );

        assert!(matches!(
            error,
            RunReviewFixError::FixRunnerFailed(
                ReviewFixRunnerError::SubagentDispatchRequired(value)
            ) if *value == instruction
        ));
    }

    #[test]
    fn test_subagent_name_rejects_empty_or_whitespace_without_exposing_domain_error() {
        for raw in [String::new(), "   ".to_owned()] {
            assert!(
                matches!(SubagentName::try_new(raw), Err(SubagentNameValidationError::Invalid(_))),
                "invalid subagent names must use the usecase-owned error"
            );
        }
    }

    #[test]
    fn test_subagent_name_accepts_valid_value_and_exposes_accessor() {
        let name = SubagentName::try_new("review-fix-lead".to_owned())
            .expect("a non-empty subagent name must be accepted");

        assert_eq!(name.as_str(), "review-fix-lead");
    }

    // ── ReviewFixRunnerError variants ─────────────────────────────────────────

    #[test]
    fn test_review_fix_runner_error_smoke_test_failed_variant_exists() {
        let e = ReviewFixRunnerError::SmokeTestFailed(DiagnosticText::new("reason"));
        assert!(matches!(e, ReviewFixRunnerError::SmokeTestFailed(_)));
    }

    #[test]
    fn test_review_fix_runner_error_spawn_failed_variant_exists() {
        let e = ReviewFixRunnerError::SpawnFailed(DiagnosticText::new("reason"));
        assert!(matches!(e, ReviewFixRunnerError::SpawnFailed(_)));
    }

    #[test]
    fn test_review_fix_runner_error_sentinel_not_found_variant_exists() {
        let e = ReviewFixRunnerError::SentinelNotFound(DiagnosticText::new("no sentinel found"));
        assert!(matches!(e, ReviewFixRunnerError::SentinelNotFound(_)));
    }

    #[test]
    fn test_review_fix_runner_error_unexpected_variant_exists() {
        let e = ReviewFixRunnerError::Unexpected(DiagnosticText::new("reason"));
        assert!(matches!(e, ReviewFixRunnerError::Unexpected(_)));
    }

    // ── Interactor delegation: completed scenario ─────────────────────────────

    #[test]
    fn test_run_review_fix_interactor_delegates_completed_to_run_fn() {
        let interactor = interactor_with(Ok(RunReviewFixOutput {
            status: "completed".to_owned(),
            exit_code: 0,
            stderr: None,
        }));
        let out = interactor.run(make_valid_request()).unwrap();
        assert_eq!(out.status, "completed");
        assert_eq!(out.exit_code, 0);
    }

    // ── Interactor delegation: blocked_cross_scope scenario ───────────────────

    #[test]
    fn test_run_review_fix_interactor_delegates_blocked_cross_scope_to_run_fn() {
        let interactor = interactor_with(Ok(RunReviewFixOutput {
            status: "blocked_cross_scope".to_owned(),
            exit_code: 2,
            stderr: None,
        }));
        let out = interactor.run(make_valid_request()).unwrap();
        assert_eq!(out.status, "blocked_cross_scope");
        assert_eq!(out.exit_code, 2);
    }

    // ── Interactor delegation: failed scenario ────────────────────────────────

    #[test]
    fn test_run_review_fix_interactor_delegates_failed_to_run_fn() {
        let interactor = interactor_with(Ok(RunReviewFixOutput {
            status: "failed".to_owned(),
            exit_code: 1,
            stderr: None,
        }));
        let out = interactor.run(make_valid_request()).unwrap();
        assert_eq!(out.status, "failed");
        assert_eq!(out.exit_code, 1);
    }

    // ── Interactor delegation: run_fn error propagation ──────────────────────

    #[test]
    fn test_run_review_fix_interactor_propagates_run_fn_error() {
        let interactor = interactor_with(Err(ReviewFixRunnerError::Unexpected(
            DiagnosticText::new("runner error"),
        )));
        match interactor.run(make_valid_request()) {
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(message))) => {
                assert_eq!(message.as_str(), "runner error");
            }
            Err(error) => panic!("expected wrapped unexpected error, got {error}"),
            Ok(_) => panic!("expected Err(FixRunnerFailed), got Ok"),
        }
    }

    #[test]
    fn test_run_review_fix_interactor_preserves_all_runner_error_variants() {
        let smoke_failure = interactor_with(Err(ReviewFixRunnerError::SmokeTestFailed(
            DiagnosticText::new("smoke detail"),
        )))
        .run(make_valid_request());
        assert!(matches!(
            smoke_failure,
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::SmokeTestFailed(message)))
                if message.as_str() == "smoke detail"
        ));

        let spawn_failure = interactor_with(Err(ReviewFixRunnerError::SpawnFailed(
            DiagnosticText::new("spawn detail"),
        )))
        .run(make_valid_request());
        assert!(matches!(
            spawn_failure,
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::SpawnFailed(message)))
                if message.as_str() == "spawn detail"
        ));

        let sentinel_failure = interactor_with(Err(ReviewFixRunnerError::SentinelNotFound(
            DiagnosticText::new("sentinel detail"),
        )))
        .run(make_valid_request());
        assert!(matches!(
            sentinel_failure,
            Err(RunReviewFixError::FixRunnerFailed(
                ReviewFixRunnerError::SentinelNotFound(message)
            )) if message.as_str() == "sentinel detail"
        ));

        let unexpected_failure = interactor_with(Err(ReviewFixRunnerError::Unexpected(
            DiagnosticText::new("unexpected detail"),
        )))
        .run(make_valid_request());
        assert!(matches!(
            unexpected_failure,
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(message)))
                if message.as_str() == "unexpected detail"
        ));

        let instruction = SubagentDispatchInstruction {
            agent: SubagentName::try_new("review-fix-lead".to_owned())
                .expect("valid test subagent"),
            model: ModelName::try_new("gpt-5.5").expect("valid test model"),
            effort: ReasoningEffort::Low,
            scope: ReviewScopeName::try_new("cli_driver".to_owned())
                .expect("valid test review scope"),
            briefing_content: SubagentBriefingContent::try_new("briefing".to_owned())
                .expect("valid briefing"),
            track_id: ReviewTrackId::try_new("dispatch-runner-2026".to_owned())
                .expect("valid test track ID"),
            repository_root: PathBuf::from("/test-repository"),
            round_type: ReviewRoundType::Fast,
        };
        let dispatch_failure = interactor_with(Err(
            ReviewFixRunnerError::SubagentDispatchRequired(Box::new(instruction.clone())),
        ))
        .run(make_valid_request());
        assert!(matches!(
            dispatch_failure,
            Err(RunReviewFixError::FixRunnerFailed(
                ReviewFixRunnerError::SubagentDispatchRequired(value)
            )) if *value == instruction
        ));
    }

    // ── Interactor output validation: invalid sentinel ────────────────────────

    #[test]
    fn test_run_review_fix_interactor_invalid_status_sentinel_returns_fix_runner_failed() {
        let interactor = interactor_with(Ok(RunReviewFixOutput {
            status: "unknown_sentinel".to_owned(),
            exit_code: 99,
            stderr: None,
        }));
        match interactor.run(make_valid_request()) {
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(message))) => {
                assert!(message.as_str().contains("invalid status sentinel"));
            }
            Err(error) => panic!("expected wrapped unexpected error, got {error}"),
            Ok(_) => panic!("expected Err(FixRunnerFailed) for invalid sentinel, got Ok"),
        }
    }

    // ── Interactor output validation: mismatched exit_code ───────────────────

    #[test]
    fn test_run_review_fix_interactor_mismatched_exit_code_returns_fix_runner_failed() {
        // "completed" maps to exit_code=0; returning 2 must be rejected.
        let interactor = interactor_with(Ok(RunReviewFixOutput {
            status: "completed".to_owned(),
            exit_code: 2,
            stderr: None,
        }));
        match interactor.run(make_valid_request()) {
            Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(message))) => {
                assert!(message.as_str().contains("exit_code 2 does not match status"));
            }
            Err(error) => panic!("expected wrapped unexpected error, got {error}"),
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
                Ok(out) => Ok(RunReviewFixOutput {
                    status: out.status.clone(),
                    exit_code: out.exit_code,
                    stderr: out.stderr.clone(),
                }),
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
                    ReviewFixRunnerError::SubagentDispatchRequired(instruction) => {
                        ReviewFixRunnerError::SubagentDispatchRequired(instruction.clone())
                    }
                    ReviewFixRunnerError::Unexpected(s) => {
                        ReviewFixRunnerError::Unexpected(s.clone())
                    }
                }),
            }
        }
    }

    struct FixedTrackResolver;

    impl ReviewFixTrackResolverPort for FixedTrackResolver {
        fn resolve_current_track(
            &self,
            _items_dir: &Path,
        ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError> {
            ReviewTrackId::try_new("my-track-2026-05-31".to_owned())
                .map(|track_id| {
                    ReviewFixResolution::new(track_id, PathBuf::from("/test-repository"))
                })
                .map_err(|error| match error {
                    ReviewTrackIdValidationError::Invalid(detail) => {
                        ReviewFixTrackResolveError::NonTrackBranch(detail)
                    }
                })
        }
    }

    struct FixedBriefingLoader;

    impl ReviewFixBriefingLoaderPort for FixedBriefingLoader {
        fn load_briefing_content(
            &self,
            _repository_root: &Path,
            _briefing_file: &Path,
        ) -> Result<SubagentBriefingContent, ReviewFixBriefingLoadError> {
            SubagentBriefingContent::try_new("briefing".to_owned())
                .map_err(ReviewFixBriefingLoadError::InvalidContent)
        }
    }

    struct RecordingTrackResolver {
        result: Mutex<Option<Result<ReviewFixResolution, ReviewFixTrackResolveError>>>,
        requested_items_dirs: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl RecordingTrackResolver {
        fn returning(result: Result<ReviewFixResolution, ReviewFixTrackResolveError>) -> Self {
            Self {
                result: Mutex::new(Some(result)),
                requested_items_dirs: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl ReviewFixTrackResolverPort for RecordingTrackResolver {
        fn resolve_current_track(
            &self,
            items_dir: &Path,
        ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError> {
            self.requested_items_dirs.lock().expect("test lock").push(items_dir.to_path_buf());
            self.result
                .lock()
                .expect("test lock")
                .take()
                .expect("resolver must be called only once")
        }
    }

    struct RecordingRunner {
        calls: Arc<AtomicUsize>,
        commands: Arc<Mutex<Vec<RunReviewFixCommand>>>,
    }

    impl ReviewFixRunner for RecordingRunner {
        fn run_fix(
            &self,
            command: RunReviewFixCommand,
        ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.commands.lock().expect("test lock").push(command);
            Ok(RunReviewFixOutput { status: "completed".to_owned(), exit_code: 0, stderr: None })
        }
    }

    fn request_with(explicit_track_id: Option<&str>, items_dir: &str) -> RunReviewFixRequest {
        RunReviewFixRequest::try_new(
            "cli".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            explicit_track_id.map(str::to_owned),
            PathBuf::from(items_dir),
            "final".to_owned(),
            None,
        )
        .expect("valid test request")
    }

    fn interactor_with(
        result: Result<RunReviewFixOutput, ReviewFixRunnerError>,
    ) -> RunReviewFixInteractor {
        RunReviewFixInteractor::new(
            Arc::new(FixedTrackResolver),
            Arc::new(FixedBriefingLoader),
            Arc::new(MockReviewFixRunner::returning(result)),
        )
    }

    #[test]
    fn test_review_fix_track_resolver_port_double_receives_items_dir_and_returns_track() {
        let resolver = RecordingTrackResolver::returning(Ok(ReviewFixResolution::new(
            ReviewTrackId::try_new("resolved-track-2026".to_owned())
                .expect("valid resolved test track"),
            PathBuf::from("/test-repository"),
        )));
        let port: &dyn ReviewFixTrackResolverPort = &resolver;

        let track = port
            .resolve_current_track(Path::new("track/items/active"))
            .expect("the port double must expose the resolved track");

        assert_eq!(track.track_id().as_str(), "resolved-track-2026");
        assert_eq!(track.repository_root(), PathBuf::from("/test-repository"));
        assert_eq!(
            resolver.requested_items_dirs.lock().expect("test lock").as_slice(),
            [PathBuf::from("track/items/active")]
        );
    }

    #[test]
    fn test_run_review_fix_interactor_resolves_branch_then_invokes_runner() {
        let resolver = RecordingTrackResolver::returning(Ok(ReviewFixResolution::new(
            ReviewTrackId::try_new("resolved-track-2026".to_owned())
                .expect("valid resolved test track"),
            PathBuf::from("/test-repository"),
        )));
        let requested_items_dirs = resolver.requested_items_dirs.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let commands = Arc::new(Mutex::new(Vec::new()));
        let interactor = RunReviewFixInteractor::new(
            Arc::new(resolver),
            Arc::new(FixedBriefingLoader),
            Arc::new(RecordingRunner { calls: calls.clone(), commands: commands.clone() }),
        );

        let output = interactor
            .run(request_with(None, "track/items/active"))
            .expect("a resolved branch must reach the runner");

        assert_eq!(output.status, "completed");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            requested_items_dirs.lock().expect("test lock").as_slice(),
            [PathBuf::from("track/items/active")]
        );
        let commands = commands.lock().expect("test lock");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("one command was asserted above");
        assert_eq!(command.track_id(), "resolved-track-2026");
        assert_eq!(command.repository_root(), PathBuf::from("/test-repository"));
        assert_eq!(command.scope(), "cli");
        assert!(matches!(command.round_type(), ReviewRoundType::Final));
    }

    #[test]
    fn test_run_review_fix_service_trait_object_resolves_branch_then_invokes_runner() {
        let resolver = RecordingTrackResolver::returning(Ok(ReviewFixResolution::new(
            ReviewTrackId::try_new("service-track-2026".to_owned())
                .expect("valid resolved test track"),
            PathBuf::from("/test-repository"),
        )));
        let calls = Arc::new(AtomicUsize::new(0));
        let service: Arc<dyn RunReviewFixService> = Arc::new(RunReviewFixInteractor::new(
            Arc::new(resolver),
            Arc::new(FixedBriefingLoader),
            Arc::new(RecordingRunner {
                calls: calls.clone(),
                commands: Arc::new(Mutex::new(Vec::new())),
            }),
        ));

        let output = service
            .run(request_with(None, "track/items/service"))
            .expect("the primary service port must invoke the resolved runner");

        assert_eq!(output.status, "completed");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_run_review_fix_interactor_rejects_explicit_track_mismatch_without_running() {
        let resolver = RecordingTrackResolver::returning(Ok(ReviewFixResolution::new(
            ReviewTrackId::try_new("resolved-track-2026".to_owned())
                .expect("valid resolved test track"),
            PathBuf::from("/test-repository"),
        )));
        let calls = Arc::new(AtomicUsize::new(0));
        let interactor = RunReviewFixInteractor::new(
            Arc::new(resolver),
            Arc::new(FixedBriefingLoader),
            Arc::new(RecordingRunner {
                calls: calls.clone(),
                commands: Arc::new(Mutex::new(Vec::new())),
            }),
        );

        let error = match interactor
            .run(request_with(Some("different-track-2026"), "track/items/active"))
        {
            Err(error) => error,
            Ok(_) => panic!("an explicit ID different from the branch must fail closed"),
        };

        assert!(matches!(
            error,
            RunReviewFixError::TrackMismatch { explicit, resolved }
                if explicit.as_str() == "different-track-2026"
                    && resolved.as_str() == "resolved-track-2026"
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "mismatch must not invoke the runner");
    }

    #[test]
    fn test_run_review_fix_interactor_returns_non_track_branch_error_without_running() {
        let resolver = RecordingTrackResolver::returning(Err(
            ReviewFixTrackResolveError::NonTrackBranch(DiagnosticText::new("main")),
        ));
        let calls = Arc::new(AtomicUsize::new(0));
        let interactor = RunReviewFixInteractor::new(
            Arc::new(resolver),
            Arc::new(FixedBriefingLoader),
            Arc::new(RecordingRunner {
                calls: calls.clone(),
                commands: Arc::new(Mutex::new(Vec::new())),
            }),
        );

        let error = match interactor.run(request_with(None, "track/items/active")) {
            Err(error) => error,
            Ok(_) => panic!("a non-track branch must fail closed"),
        };

        assert!(matches!(
            error,
            RunReviewFixError::TrackResolution(ReviewFixTrackResolveError::NonTrackBranch(detail))
                if detail.as_str() == "main"
        ));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "resolution failure must not invoke the runner"
        );
    }

    #[test]
    fn test_run_review_fix_interactor_propagates_briefing_load_error_without_running() {
        struct UntrustedBriefingLoader;

        impl ReviewFixBriefingLoaderPort for UntrustedBriefingLoader {
            fn load_briefing_content(
                &self,
                _repository_root: &Path,
                _briefing_file: &Path,
            ) -> Result<SubagentBriefingContent, ReviewFixBriefingLoadError> {
                Err(ReviewFixBriefingLoadError::UntrustedFile(DiagnosticText::new(
                    "symlinked intermediate component",
                )))
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let interactor = RunReviewFixInteractor::new(
            Arc::new(FixedTrackResolver),
            Arc::new(UntrustedBriefingLoader),
            Arc::new(RecordingRunner {
                calls: calls.clone(),
                commands: Arc::new(Mutex::new(Vec::new())),
            }),
        );

        let error = match interactor.run(make_valid_request()) {
            Err(error) => error,
            Ok(_) => panic!("an untrusted briefing must fail before the runner is invoked"),
        };

        assert!(matches!(
            error,
            RunReviewFixError::BriefingLoad(ReviewFixBriefingLoadError::UntrustedFile(detail))
                if detail.as_str() == "symlinked intermediate component"
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "briefing failure must not invoke runner");
    }

    #[test]
    fn test_review_fix_runner_mock_completed_scenario() {
        let runner = MockReviewFixRunner::returning(Ok(RunReviewFixOutput {
            status: "completed".to_owned(),
            exit_code: 0,
            stderr: None,
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
            stderr: None,
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
            stderr: None,
        }));
        let out = runner.run_fix(make_valid_command()).unwrap();
        assert_eq!(out.status, "failed");
        assert_eq!(out.exit_code, 1);
    }

    #[test]
    fn test_review_fix_runner_mock_sentinel_not_found_scenario() {
        let runner = MockReviewFixRunner::returning(Err(ReviewFixRunnerError::SentinelNotFound(
            DiagnosticText::new("no sentinel"),
        )));
        match runner.run_fix(make_valid_command()) {
            Err(e) => assert!(matches!(e, ReviewFixRunnerError::SentinelNotFound(_))),
            Ok(_) => panic!("expected Err(SentinelNotFound), got Ok"),
        }
    }
}
