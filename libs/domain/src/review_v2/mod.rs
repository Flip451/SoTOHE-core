//! Review System v2 domain types.
//!
//! Pure data types with constructor validation for the scope-independent
//! review system. No I/O, no port traits (those are in T002).

pub mod error;
pub mod types;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use error::{FindingError, ScopeNameError, VerdictError};
pub use types::{
    FastVerdict, FilePath, Finding, LogInfo, MainScopeName, NotRequiredReason, RequiredReason,
    ReviewHash, ReviewOutcome, ReviewState, ReviewTarget, ScopeName, Verdict,
};
