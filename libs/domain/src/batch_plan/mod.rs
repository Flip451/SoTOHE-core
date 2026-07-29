//! Domain model of the declared batch plan.
//!
//! Holds the per-task line estimates the planner declares for each review scope,
//! the ordered batches those tasks are assigned to, and the rejections that are
//! decidable from the plan file alone. Judgements that need the resolved
//! per-scope ceiling — the Phase 3 gate and the transition admission guard —
//! consume these values but are owned elsewhere.
//!
//! See ADR 2026-07-28-1521-scope-diff-ceiling-admission-enforcement.

mod batch;
mod document;
mod error;
mod estimate;
mod line_count;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

pub use batch::{BatchDeclaration, BatchId};
pub use document::BatchPlanDocument;
pub use error::BatchPlanValidationError;
pub use estimate::{
    IndivisibilityJustification, ScopeLineEstimate, TaskDecomposition, TaskEstimate,
};
pub use line_count::{LineCount, MeasuredScopeDiff, ScopeCeiling};
