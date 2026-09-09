//! Fail-closed reviewer adapter for review results state-summary and review check-approved.

use domain::review_v2::{FastVerdict, LogInfo, ReviewTarget, Verdict};
use usecase::review_v2::{
    Reviewer, ReviewerError, ReviewerExecutionProvenance, ReviewerProcessDiagnostic,
};

/// Infrastructure secondary adapter used when the composition root only reads review state.
///
/// The review results state-summary and review check-approved paths use
/// `get_review_states` and `evaluate_approval`; they do not execute a provider. If either
/// reviewer method is reached accidentally, this adapter fails closed before any provider
/// invocation and cannot produce a successful verdict.
pub struct NullReviewer;

impl Reviewer for NullReviewer {
    fn review(&self, _target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected {
            provenance: ReviewerExecutionProvenance::PreSpawn,
            diagnostic: ReviewerProcessDiagnostic::Unavailable,
        })
    }

    fn fast_review(&self, _target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected {
            provenance: ReviewerExecutionProvenance::PreSpawn,
            diagnostic: ReviewerProcessDiagnostic::Unavailable,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_null_reviewer_review_fails_closed_before_provider_invocation() {
        let result = NullReviewer.review(&ReviewTarget::new(Vec::new()));

        assert!(matches!(
            result,
            Err(ReviewerError::Unexpected {
                provenance: ReviewerExecutionProvenance::PreSpawn,
                diagnostic: ReviewerProcessDiagnostic::Unavailable,
            })
        ));
    }

    #[test]
    fn test_null_reviewer_fast_review_fails_closed_before_provider_invocation() {
        let result = NullReviewer.fast_review(&ReviewTarget::new(Vec::new()));

        assert!(matches!(
            result,
            Err(ReviewerError::Unexpected {
                provenance: ReviewerExecutionProvenance::PreSpawn,
                diagnostic: ReviewerProcessDiagnostic::Unavailable,
            })
        ));
    }
}
