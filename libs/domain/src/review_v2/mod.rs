//! Review System v2 domain types and ports.
//!
//! Pure data types with constructor validation, scope classification,
//! and persistence port traits for the scope-independent review system.

pub mod error;
pub mod ports;
pub mod scope_config;
pub mod types;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use error::{
    CommitHashError, FilePathError, ReviewHashError, ReviewReaderError, ReviewWriterError,
    ReviewerFindingError, ScopeConfigError, ScopeNameError, VerdictError,
};
pub use ports::{CommitHashReader, CommitHashWriter, ReviewExistsPort, ReviewReader, ReviewWriter};
pub use scope_config::ReviewScopeConfig;
pub use types::{
    FastVerdict, FilePath, LogInfo, MainScopeName, NonEmptyReviewerFindings, NotRequiredReason,
    RequiredReason, ReviewApprovalVerdict, ReviewHash, ReviewHashValue, ReviewOutcome, ReviewState,
    ReviewTarget, ReviewerFinding, RoundType, ScopeName, ScopeRound, Verdict,
    extract_verdict_json_candidates_compact, extract_verdict_json_candidates_multiline,
};
