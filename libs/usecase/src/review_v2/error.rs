use thiserror::Error;

use crate::capability_exec::ProviderName;
use crate::program_runner::ProgramExitCode;
use domain::review_v2::{ReviewReaderError, ScopeName};

const MAX_REVIEWER_DIAGNOSTIC_BYTES: usize = 4 * 1024;

/// Validation failures for bounded redacted reviewer diagnostic text.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewerDiagnosticValidationError {
    #[error("reviewer diagnostic must not be empty")]
    Empty,
    #[error("reviewer diagnostic exceeds 4096 UTF-8 bytes")]
    TooLong,
}

/// A validated reviewer diagnostic text value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerDiagnostic(String);

impl ReviewerDiagnostic {
    /// Validates a redacted diagnostic using the specification's byte bound.
    ///
    /// # Errors
    ///
    /// Returns [`ReviewerDiagnosticValidationError::Empty`] for empty or
    /// whitespace-only text and [`ReviewerDiagnosticValidationError::TooLong`]
    /// when the UTF-8 representation exceeds 4096 bytes.
    pub fn try_new(value: String) -> Result<Self, ReviewerDiagnosticValidationError> {
        if value.trim().is_empty() {
            return Err(ReviewerDiagnosticValidationError::Empty);
        }
        if value.len() > MAX_REVIEWER_DIAGNOSTIC_BYTES {
            return Err(ReviewerDiagnosticValidationError::TooLong);
        }
        Ok(Self(value))
    }

    /// Returns the validated diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Describes whether a safe reviewer subprocess diagnostic is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewerProcessDiagnostic {
    Available(ReviewerDiagnostic),
    Unavailable,
}

/// Records whether an unexpected reviewer failure happened before or after
/// the provider subprocess was spawned.
#[derive(Debug, PartialEq, Eq)]
pub enum ReviewerExecutionProvenance {
    PreSpawn,
    PostSpawn,
}

/// Errors from the `Reviewer` usecase port.
#[derive(Debug)]
pub enum ReviewerError {
    /// The user explicitly cancelled the review.
    UserAbort,
    /// The provider process exited unsuccessfully.
    ProcessFailed {
        provider: ProviderName,
        exit_code: Option<ProgramExitCode>,
        diagnostic: ReviewerProcessDiagnostic,
    },
    /// The provider process exceeded its configured time limit.
    Timeout,
    /// The provider returned a successful process status but no valid verdict.
    IllegalVerdict,
    /// An adapter failure with its execution lifecycle and optional safe diagnostic.
    Unexpected { provenance: ReviewerExecutionProvenance, diagnostic: ReviewerProcessDiagnostic },
}

impl std::fmt::Display for ReviewerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserAbort => formatter.write_str("user aborted review"),
            Self::ProcessFailed { provider, exit_code, diagnostic } => write!(
                formatter,
                "reviewer process failed: provider={provider}, exit_code={}, diagnostic={}",
                render_exit_code(exit_code),
                render_process_diagnostic(diagnostic),
            ),
            Self::Timeout => formatter.write_str("reviewer timed out"),
            Self::IllegalVerdict => formatter.write_str("illegal verdict format from reviewer"),
            Self::Unexpected { diagnostic, .. } => write!(
                formatter,
                "unexpected reviewer error: {}",
                render_process_diagnostic(diagnostic),
            ),
        }
    }
}

impl std::error::Error for ReviewerError {}

fn render_exit_code(exit_code: &Option<ProgramExitCode>) -> String {
    exit_code
        .as_ref()
        .map(|code| code.as_i32().to_string())
        .unwrap_or_else(|| "unavailable".to_owned())
}

fn render_process_diagnostic(diagnostic: &ReviewerProcessDiagnostic) -> &str {
    match diagnostic {
        ReviewerProcessDiagnostic::Available(value) => value.as_str(),
        ReviewerProcessDiagnostic::Unavailable => "diagnostic_unavailable",
    }
}

/// Errors from the `DiffGetter` usecase port.
#[derive(Debug, Error)]
pub enum DiffGetError {
    #[error("diff operation failed: {0}")]
    Failed(String),
}

/// Errors from the `ReviewHasher` usecase port.
#[derive(Debug, Error)]
pub enum ReviewHasherError {
    #[error("hash computation failed: {0}")]
    Failed(String),
}

/// Errors from `ReviewCycle` orchestrator operations.
#[derive(Debug, Error)]
pub enum ReviewCycleError {
    #[error("unknown scope: {0}")]
    UnknownScope(ScopeName),
    #[error("file changed during review — before/after hash mismatch")]
    FileChangedDuringReview,
    #[error("diff error: {0}")]
    Diff(#[from] DiffGetError),
    #[error("post-review diff error: {0}")]
    PostReviewDiff(DiffGetError),
    #[error("hash error: {0}")]
    Hash(#[from] ReviewHasherError),
    #[error("post-review hash error: {0}")]
    PostReviewHash(ReviewHasherError),
    #[error("reviewer error: {0}")]
    Reviewer(#[from] ReviewerError),
    #[error("review reader error: {0}")]
    Reader(#[from] ReviewReaderError),
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{
        DiffGetError, ReviewCycleError, ReviewHasherError, ReviewerDiagnostic,
        ReviewerDiagnosticValidationError, ReviewerError, ReviewerExecutionProvenance,
        ReviewerProcessDiagnostic,
    };
    use crate::capability_exec::ProviderName;
    use crate::program_runner::ProgramExitCode;

    #[test]
    fn test_review_cycle_error_declares_distinct_post_review_variants() {
        let pre_review_diff =
            ReviewCycleError::Diff(DiffGetError::Failed("same diff failure".to_owned()));
        let post_review_diff =
            ReviewCycleError::PostReviewDiff(DiffGetError::Failed("same diff failure".to_owned()));
        let pre_review_hash =
            ReviewCycleError::Hash(ReviewHasherError::Failed("same hash failure".to_owned()));
        let post_review_hash = ReviewCycleError::PostReviewHash(ReviewHasherError::Failed(
            "same hash failure".to_owned(),
        ));

        // The enum declaration's PostReview* variants distinguish failures after a
        // verdict from Diff/Hash failures that occur before a verdict is observed.
        assert_eq!(
            pre_review_diff.to_string(),
            "diff error: diff operation failed: same diff failure"
        );
        assert_eq!(
            post_review_diff.to_string(),
            "post-review diff error: diff operation failed: same diff failure"
        );
        assert_eq!(
            pre_review_hash.to_string(),
            "hash error: hash computation failed: same hash failure"
        );
        assert_eq!(
            post_review_hash.to_string(),
            "post-review hash error: hash computation failed: same hash failure"
        );
        assert!(matches!(
            pre_review_diff,
            ReviewCycleError::Diff(DiffGetError::Failed(message))
                if message == "same diff failure"
        ));
        assert!(matches!(
            post_review_diff,
            ReviewCycleError::PostReviewDiff(DiffGetError::Failed(message))
                if message == "same diff failure"
        ));
        assert!(matches!(
            pre_review_hash,
            ReviewCycleError::Hash(ReviewHasherError::Failed(message))
                if message == "same hash failure"
        ));
        assert!(matches!(
            post_review_hash,
            ReviewCycleError::PostReviewHash(ReviewHasherError::Failed(message))
                if message == "same hash failure"
        ));
    }

    #[test]
    fn test_reviewer_diagnostic_enforces_nonempty_and_utf8_byte_bound() {
        assert_eq!(
            ReviewerDiagnostic::try_new(" ".to_owned()),
            Err(ReviewerDiagnosticValidationError::Empty)
        );
        assert_eq!(
            ReviewerDiagnostic::try_new("a".repeat(4097)),
            Err(ReviewerDiagnosticValidationError::TooLong)
        );

        let exact_boundary = "b".repeat(4096);
        assert_eq!(
            ReviewerDiagnostic::try_new(exact_boundary.clone())
                .expect("4096 UTF-8 bytes fit")
                .as_str(),
            exact_boundary
        );

        let multibyte = "界".repeat(1365);
        let diagnostic = ReviewerDiagnostic::try_new(multibyte.clone()).expect("4095 bytes fit");
        assert_eq!(diagnostic.as_str(), multibyte);

        let multibyte_over_boundary = "界".repeat(1366);
        assert_eq!(
            ReviewerDiagnostic::try_new(multibyte_over_boundary),
            Err(ReviewerDiagnosticValidationError::TooLong)
        );
    }

    #[test]
    fn test_reviewer_error_display_preserves_process_identity_and_safe_fallback() {
        let diagnostic = ReviewerDiagnostic::try_new("redacted failure".to_owned())
            .expect("diagnostic is bounded");
        let error = ReviewerError::ProcessFailed {
            provider: ProviderName::try_new("codex").expect("provider is valid"),
            exit_code: Some(ProgramExitCode::new(23)),
            diagnostic: ReviewerProcessDiagnostic::Available(diagnostic),
        };

        assert_eq!(
            error.to_string(),
            "reviewer process failed: provider=codex, exit_code=23, diagnostic=redacted failure"
        );

        let unavailable = ReviewerError::Unexpected {
            provenance: ReviewerExecutionProvenance::PostSpawn,
            diagnostic: ReviewerProcessDiagnostic::Unavailable,
        };
        assert!(unavailable.to_string().contains("diagnostic_unavailable"));
    }

    #[test]
    fn test_reviewer_error_lifts_through_review_cycle_error() {
        let error = ReviewerError::Unexpected {
            provenance: ReviewerExecutionProvenance::PreSpawn,
            diagnostic: ReviewerProcessDiagnostic::Unavailable,
        };
        let cycle_error: ReviewCycleError = error.into();
        assert!(matches!(cycle_error, ReviewCycleError::Reviewer(_)));
    }

    #[test]
    fn test_reviewer_unexpected_retains_provenance_independently_of_diagnostic() {
        let pre_spawn = ReviewerError::Unexpected {
            provenance: ReviewerExecutionProvenance::PreSpawn,
            diagnostic: ReviewerProcessDiagnostic::Unavailable,
        };
        let post_spawn = ReviewerError::Unexpected {
            provenance: ReviewerExecutionProvenance::PostSpawn,
            diagnostic: ReviewerProcessDiagnostic::Unavailable,
        };

        assert!(matches!(
            pre_spawn,
            ReviewerError::Unexpected {
                provenance: ReviewerExecutionProvenance::PreSpawn,
                diagnostic: ReviewerProcessDiagnostic::Unavailable,
            }
        ));
        assert!(matches!(
            post_spawn,
            ReviewerError::Unexpected {
                provenance: ReviewerExecutionProvenance::PostSpawn,
                diagnostic: ReviewerProcessDiagnostic::Unavailable,
            }
        ));
    }
}
