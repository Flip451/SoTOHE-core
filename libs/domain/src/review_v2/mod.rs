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
    CommitHashError, FilePathError, FindingError, ReviewHashError, ReviewReaderError,
    ReviewWriterError, ScopeConfigError, ScopeNameError, VerdictError,
};
pub use ports::{CommitHashReader, CommitHashWriter, ReviewReader, ReviewWriter};
pub use scope_config::ReviewScopeConfig;
pub use types::{
    FastVerdict, FilePath, Finding, LogInfo, MainScopeName, NonEmptyFindings, NotRequiredReason,
    RequiredReason, ReviewHash, ReviewHashValue, ReviewOutcome, ReviewState, ReviewTarget,
    ScopeName, Verdict, extract_verdict_json_candidates_compact,
    extract_verdict_json_candidates_multiline,
};
