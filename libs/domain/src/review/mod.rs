//! Review state management for track-level review workflows.
//!
//! Tracks review progress through a state machine:
//! `NotStarted` → `FastPassed` → `Approved`, with `Invalidated` on code changes.

pub mod concern;
pub mod cycle;
pub mod error;
pub mod escalation;
pub mod state;
pub mod types;

#[cfg(test)]
mod tests;

pub use concern::{ReviewConcern, ReviewConcernStreak, ReviewCycleSummary, file_path_to_concern};
pub use cycle::{
    CycleError, CycleGroupState, GroupRound, GroupRoundOutcome, GroupRoundVerdict,
    NonEmptyFindings, ReviewCycle, ReviewJson, ReviewStalenessReason, StoredFinding,
};
pub use error::ReviewError;
pub use escalation::{
    EscalationPhase, ReviewEscalationBlock, ReviewEscalationDecision, ReviewEscalationResolution,
    ReviewEscalationState,
};
pub use state::ReviewState;
pub use types::{
    CodeHash, ModelProfile, ReviewGroupState, ReviewRoundResult, ReviewStatus, RoundType, Verdict,
    extract_verdict_json_candidates_compact, extract_verdict_json_candidates_multiline,
    resolve_full_auto,
};
