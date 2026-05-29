//! Null reviewer — used when the composition only needs status/check-approved
//! (no actual review invocation).

use domain::review_v2::{FastVerdict, LogInfo, ReviewTarget, Verdict};
use usecase::review_v2::{ReviewerError, ports::Reviewer};

/// Null reviewer — used when the composition only needs status/check-approved
/// (no actual review invocation). The Reviewer trait is required by ReviewCycle
/// but these operations only call `get_review_states()`.
pub(crate) struct NullReviewer;

impl Reviewer for NullReviewer {
    fn review(&self, _target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected("NullReviewer: review() must not be called".to_owned()))
    }

    fn fast_review(&self, _target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected("NullReviewer: fast_review() must not be called".to_owned()))
    }
}
