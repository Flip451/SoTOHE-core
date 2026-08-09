//! Review System v2 usecase layer.
//!
//! Application port traits (Reviewer, DiffGetter, ReviewHasher) and the
//! ReviewCycle orchestrator. Does not persist — callers handle ReviewWriter.

pub mod aggregate_service;
pub mod check_approved;
pub mod check_zero_findings;
pub mod cycle;
pub mod error;
pub mod ports;
pub mod review_aux;
pub mod run_review;
pub mod run_review_fix;
pub mod scope_query;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use aggregate_service::{ReviewRunInput, ReviewService};
pub use check_approved::{
    ReviewApprovalDecision, ReviewApprovalOutput, ReviewCheckApprovedError,
    ReviewCheckApprovedInteractor, ReviewCheckApprovedService,
};
pub use check_zero_findings::{
    ReviewCheckZeroFindingsEvaluationError, ReviewCheckZeroFindingsInteractor,
    ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsQuery, ReviewCheckZeroFindingsService,
    ReviewCheckZeroFindingsStatePort, ReviewCheckZeroFindingsValidationError,
};
pub use cycle::ReviewCycle;
pub use error::{DiffGetError, ReviewCycleError, ReviewHasherError, ReviewerError};
pub use ports::{DiffGetter, ReviewHasher, Reviewer};
pub use review_aux::{
    ReviewAuxError, ReviewClassifyInteractor, ReviewClassifyService, ReviewFilesInteractor,
    ReviewFilesService, ReviewGetBriefingInteractor, ReviewGetBriefingService,
    ReviewResultsInteractor, ReviewResultsService, ReviewRunLocalInteractor, ReviewRunLocalOutput,
    ReviewRunLocalService, ReviewValidateScopeInteractor, ReviewValidateScopeService,
};
pub use run_review::{
    ReviewRoundType, RunReviewCommand, RunReviewError, RunReviewInteractor, RunReviewOutput,
    RunReviewService,
};
pub use run_review_fix::{
    ReviewFixRunner, ReviewFixRunnerError, ReviewGroupName, RunReviewFixCommand, RunReviewFixError,
    RunReviewFixInteractor, RunReviewFixOutput, RunReviewFixService, SubagentDispatchInstruction,
    SubagentName, TrackId,
};
pub use scope_query::{
    PathClassification, ScopeClassification, ScopeClassificationOutput, ScopeQueryError,
    ScopeQueryInteractor, ScopeQueryService,
};
