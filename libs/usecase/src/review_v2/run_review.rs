//! Run-review application service (usecase layer).
//!
//! Wraps the `ReviewCycle` composition root so the CLI never imports
//! `domain::review_v2::Verdict`, `domain::review_v2::FastVerdict`,
//! `domain::review_v2::ReviewOutcome`, or related domain types directly
//! (CN-01 / D1).

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── ReviewRoundTypeError ──────────────────────────────────────────────────────

/// Error type for [`ReviewRoundType::parse`].
#[derive(Debug, thiserror::Error)]
pub enum ReviewRoundTypeError {
    #[error("{0}")]
    InvalidValue(String),
}

impl std::ops::Deref for ReviewRoundTypeError {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        match self {
            ReviewRoundTypeError::InvalidValue(msg) => msg.as_str(),
        }
    }
}

// ── ReviewRoundType ───────────────────────────────────────────────────────────

/// Usecase-layer enum mirroring `domain::RoundType` (Fast/Final).
///
/// Used internally by [`RunReviewService`] and its interactor when converting
/// the `round_type` string from [`RunReviewCommand`] to the domain type. The
/// CLI passes `round_type` as a plain `String` in [`RunReviewCommand`] so it
/// never needs to import `ReviewRoundType` or `domain::RoundType` directly;
/// the interactor converts the string to `ReviewRoundType` internally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewRoundType {
    Fast,
    Final,
}

impl ReviewRoundType {
    /// Parses a `&str` (`"fast"` or `"final"`) into [`ReviewRoundType`].
    ///
    /// # Errors
    ///
    /// Returns [`ReviewRoundTypeError`] for unrecognised values.
    pub fn parse(s: &str) -> Result<Self, ReviewRoundTypeError> {
        match s {
            "fast" => Ok(Self::Fast),
            "final" => Ok(Self::Final),
            other => Err(ReviewRoundTypeError::InvalidValue(format!(
                "unknown round type: '{other}' (expected 'fast' or 'final')"
            ))),
        }
    }
}

// ── RunReviewCommand ──────────────────────────────────────────────────────────

/// CQRS command object for the run-review use case (`sotp review codex-local`).
///
/// Carries standard-library-typed fields only (String, PathBuf, u64 — no domain
/// types). The interactor converts `round_type` string to [`ReviewRoundType`]
/// internally.
pub struct RunReviewCommand {
    pub track_id: String,
    pub round_type: String,
    pub group: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub base_prompt: String,
    pub briefing_file: Option<PathBuf>,
    pub items_dir: PathBuf,
}

// ── RunReviewOutput ───────────────────────────────────────────────────────────

/// DTO returned by [`RunReviewService`].
///
/// Contains `verdict_kind` (`'approved'` | `'rejected'` | `'skipped'`),
/// `skipped`, `finding_count`, and an optional human-readable `summary`.
/// Replaces direct CLI consumption of `domain::review_v2::Verdict`,
/// `domain::review_v2::FastVerdict`, and `domain::review_v2::ReviewOutcome`
/// so the CLI display layer never imports domain review types.
pub struct RunReviewOutput {
    pub verdict_kind: String,
    pub skipped: bool,
    pub finding_count: usize,
    pub summary: Option<String>,
    /// Original CLI exit code from the underlying reviewer subprocess. Preserves
    /// the convention that `findings_remain` returns exit code 2 (so callers can
    /// distinguish "review found issues" from "review subprocess failed").
    pub exit_code: u8,
}

// ── RunReviewError ────────────────────────────────────────────────────────────

/// Error type for [`RunReviewService`].
///
/// Wraps invalid track ID, invalid group name, composition root failures, and
/// reviewer execution failures without leaking `domain::review_v2::ReviewCycleError`
/// or domain review types across the usecase boundary.
#[derive(Debug, Error)]
pub enum RunReviewError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("invalid group name: {0}")]
    InvalidGroupName(String),
    #[error("composition failed: {0}")]
    CompositionFailed(String),
    #[error("reviewer failed: {0}")]
    ReviewerFailed(String),
}

// ── RunReviewService ──────────────────────────────────────────────────────────

/// Application service trait for the run-review use case (`sotp review codex-local`).
///
/// Driven by the CLI layer. Accepts [`RunReviewCommand`] containing string-typed
/// fields so the CLI never imports `domain::TrackId`, `domain::RoundType`,
/// `domain::ReviewGroupName`, `domain::review_v2::Verdict`,
/// `domain::review_v2::FastVerdict`, or `domain::review_v2::ReviewOutcome`.
/// The interactor builds the `ReviewCycle` composition root internally and
/// converts domain types to [`RunReviewOutput`].
pub trait RunReviewService: Send + Sync {
    /// Runs a review for the given command.
    ///
    /// # Errors
    ///
    /// Returns [`RunReviewError`] on ID validation, composition, or reviewer
    /// failures.
    fn run(&self, command: RunReviewCommand) -> Result<RunReviewOutput, RunReviewError>;
}

// ── RunReviewInteractor ───────────────────────────────────────────────────────

/// Concrete struct implementing [`RunReviewService`].
///
/// Builds the `ReviewCycle` composition root internally from string-typed
/// command fields, executes the review, and converts domain types to
/// [`RunReviewOutput`] before returning to the CLI.
///
/// The `run_fn` field is a function pointer supplied by the CLI composition root
/// that performs the domain+infra wiring and returns a `RunReviewOutput`.
/// This avoids violating the hexagonal boundary by not importing infrastructure
/// types from the usecase crate.
pub struct RunReviewInteractor {
    run_fn: Arc<dyn Fn(RunReviewCommand) -> Result<RunReviewOutput, RunReviewError> + Send + Sync>,
}

impl RunReviewInteractor {
    /// Creates a new interactor with the given run function.
    #[must_use]
    pub fn new(
        run_fn: Arc<
            dyn Fn(RunReviewCommand) -> Result<RunReviewOutput, RunReviewError> + Send + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl RunReviewService for RunReviewInteractor {
    fn run(&self, command: RunReviewCommand) -> Result<RunReviewOutput, RunReviewError> {
        (self.run_fn)(command)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_review_round_type_from_str_fast_succeeds() {
        let rt = ReviewRoundType::parse("fast").unwrap();
        assert_eq!(rt, ReviewRoundType::Fast);
    }

    #[test]
    fn test_review_round_type_from_str_final_succeeds() {
        let rt = ReviewRoundType::parse("final").unwrap();
        assert_eq!(rt, ReviewRoundType::Final);
    }

    #[test]
    fn test_review_round_type_from_str_unknown_returns_error() {
        let err = ReviewRoundType::parse("bad").unwrap_err();
        assert!(err.contains("bad"));
    }

    #[test]
    fn test_run_review_error_variants_exist() {
        let e1 = RunReviewError::InvalidTrackId("id".to_owned());
        assert!(matches!(e1, RunReviewError::InvalidTrackId(_)));
        let e2 = RunReviewError::InvalidGroupName("g".to_owned());
        assert!(matches!(e2, RunReviewError::InvalidGroupName(_)));
        let e3 = RunReviewError::CompositionFailed("c".to_owned());
        assert!(matches!(e3, RunReviewError::CompositionFailed(_)));
        let e4 = RunReviewError::ReviewerFailed("r".to_owned());
        assert!(matches!(e4, RunReviewError::ReviewerFailed(_)));
    }

    #[test]
    fn test_run_review_interactor_delegates_to_run_fn() {
        let run_fn = Arc::new(|_cmd: RunReviewCommand| {
            Ok(RunReviewOutput {
                verdict_kind: "approved".to_owned(),
                skipped: false,
                finding_count: 0,
                summary: None,
                exit_code: 0,
            })
        });
        let interactor = RunReviewInteractor::new(run_fn);
        let cmd = RunReviewCommand {
            track_id: "my-track-2026-04-30".to_owned(),
            round_type: "fast".to_owned(),
            group: "domain".to_owned(),
            model: "gpt-4".to_owned(),
            timeout_seconds: 120,
            base_prompt: "review".to_owned(),
            briefing_file: None,
            items_dir: PathBuf::from("track/items"),
        };
        let out = interactor.run(cmd).unwrap();
        assert_eq!(out.verdict_kind, "approved");
        assert!(!out.skipped);
    }
}
