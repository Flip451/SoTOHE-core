//! Review state management for track-level review workflows (v1).
//!
//! Types required by metadata.json codec and review.json codec.
//! New review workflow logic uses `review_v2` instead.

pub mod concern;
pub mod cycle;
pub mod error;
pub mod escalation;
pub mod state;
pub mod types;

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
    ApprovedHead, CodeHash, ReviewGroupState, ReviewRoundResult, ReviewStatus, RoundType, Verdict,
    extract_verdict_json_candidates_compact, extract_verdict_json_candidates_multiline,
};
