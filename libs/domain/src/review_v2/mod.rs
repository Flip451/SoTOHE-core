//! Review System v2 domain types.
//!
//! Pure data types with constructor validation for the scope-independent
//! review system. No I/O, no port traits (those are in T002).

pub mod error;
pub mod scope_config;
pub mod types;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use error::{
    FilePathError, FindingError, ReviewHashError, ScopeConfigError, ScopeNameError, VerdictError,
};
pub use scope_config::ReviewScopeConfig;
pub use types::{
    FastVerdict, FilePath, Finding, LogInfo, MainScopeName, NonEmptyFindings, NotRequiredReason,
    RequiredReason, ReviewHash, ReviewHashValue, ReviewOutcome, ReviewState, ReviewTarget,
    ScopeName, Verdict,
};
