//! Review System v2 usecase layer.
//!
//! Application port traits (Reviewer, DiffGetter, ReviewHasher) and the
//! ReviewCycle orchestrator. Does not persist — callers handle ReviewWriter.

pub mod cycle;
pub mod error;
pub mod ports;
pub mod scope_query;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use cycle::ReviewCycle;
pub use error::{DiffGetError, ReviewCycleError, ReviewHasherError, ReviewerError};
pub use ports::{DiffGetter, ReviewHasher, Reviewer};
pub use scope_query::{
    PathClassification, ScopeClassification, ScopeQueryError, ScopeQueryInteractor,
    ScopeQueryService,
};
