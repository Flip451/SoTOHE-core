//! Review System v2 usecase layer.
//!
//! Application port traits (Reviewer, DiffGetter, ReviewHasher) and the
//! ReviewCycle orchestrator. Does not persist — callers handle ReviewWriter.

pub mod check_approved;
pub mod cycle;
pub mod error;
pub mod ports;
pub mod run_review;
pub mod run_review_fix;
pub mod scope_query;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use check_approved::{
    ReviewApprovalDecision, ReviewApprovalOutput, ReviewCheckApprovedError,
    ReviewCheckApprovedInteractor, ReviewCheckApprovedService,
};
pub use cycle::ReviewCycle;
pub use error::{DiffGetError, ReviewCycleError, ReviewHasherError, ReviewerError};
pub use ports::{DiffGetter, ReviewHasher, Reviewer};
pub use run_review::{
    ReviewRoundType, RunReviewCommand, RunReviewError, RunReviewInteractor, RunReviewOutput,
    RunReviewService,
};
pub use run_review_fix::{
    ReviewFixRunner, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixError,
    RunReviewFixInteractor, RunReviewFixOutput, RunReviewFixService,
};
pub use scope_query::{
    PathClassification, ScopeClassification, ScopeClassificationOutput, ScopeQueryError,
    ScopeQueryInteractor, ScopeQueryService,
};
