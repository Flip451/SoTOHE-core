use thiserror::Error;

use domain::review_v2::{ReviewReaderError, ScopeName};

/// Errors from the `Reviewer` usecase port.
#[derive(Debug, Error)]
pub enum ReviewerError {
    #[error("user aborted review")]
    UserAbort,
    #[error("reviewer process aborted")]
    ReviewerAbort,
    #[error("reviewer timed out")]
    Timeout,
    #[error("illegal verdict format from reviewer")]
    IllegalVerdict,
    #[error("unexpected reviewer error: {0}")]
    Unexpected(String),
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
mod tests {
    use super::{DiffGetError, ReviewCycleError, ReviewHasherError};

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
}
