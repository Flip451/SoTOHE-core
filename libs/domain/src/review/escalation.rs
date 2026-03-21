//! Review escalation types: ReviewEscalationBlock, ReviewEscalationDecision,
//! ReviewEscalationResolution, EscalationPhase, and ReviewEscalationState.

use std::collections::BTreeMap;

use super::concern::{ReviewConcern, ReviewConcernStreak, ReviewCycleSummary};
use super::error::ReviewError;
use crate::Timestamp;

/// Details of an escalation block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationBlock {
    pub(super) concerns: Vec<ReviewConcern>,
    blocked_at: Timestamp,
}

impl ReviewEscalationBlock {
    /// Creates a new `ReviewEscalationBlock`.
    #[must_use]
    pub fn new(concerns: Vec<ReviewConcern>, blocked_at: Timestamp) -> Self {
        Self { concerns, blocked_at }
    }

    /// Returns the concerns that triggered the escalation block.
    #[must_use]
    pub fn concerns(&self) -> &[ReviewConcern] {
        &self.concerns
    }

    /// Returns the timestamp string of when the block was set.
    #[must_use]
    pub fn blocked_at(&self) -> &str {
        self.blocked_at.as_str()
    }
}

/// Decision made during escalation resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEscalationDecision {
    /// Adopt a solution already present in the workspace.
    AdoptWorkspaceSolution,
    /// Adopt an external crate to solve the concern.
    AdoptExternalCrate,
    /// Continue with the current self-implementation approach.
    ContinueSelfImplementation,
}

/// Evidence and decision for resolving an escalation block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationResolution {
    pub(super) blocked_concerns: Vec<ReviewConcern>,
    workspace_search_ref: crate::NonEmptyString,
    reinvention_check_ref: crate::NonEmptyString,
    decision: ReviewEscalationDecision,
    summary: crate::NonEmptyString,
    resolved_at: Timestamp,
}

impl ReviewEscalationResolution {
    /// Creates a new `ReviewEscalationResolution`.
    ///
    /// # Errors
    ///
    /// Returns `ReviewError::ResolutionEvidenceMissing` if any of
    /// `workspace_search_ref`, `reinvention_check_ref`, or `summary` is empty.
    pub fn new(
        blocked_concerns: Vec<ReviewConcern>,
        workspace_search_ref: impl Into<String>,
        reinvention_check_ref: impl Into<String>,
        decision: ReviewEscalationDecision,
        summary: impl Into<String>,
        resolved_at: Timestamp,
    ) -> Result<Self, ReviewError> {
        let workspace_search_ref = crate::NonEmptyString::try_new(workspace_search_ref)
            .map_err(|_| ReviewError::ResolutionEvidenceMissing("workspace_search_ref"))?;
        let reinvention_check_ref = crate::NonEmptyString::try_new(reinvention_check_ref)
            .map_err(|_| ReviewError::ResolutionEvidenceMissing("reinvention_check_ref"))?;
        let summary = crate::NonEmptyString::try_new(summary)
            .map_err(|_| ReviewError::ResolutionEvidenceMissing("summary"))?;
        Ok(Self {
            blocked_concerns,
            workspace_search_ref,
            reinvention_check_ref,
            decision,
            summary,
            resolved_at,
        })
    }

    /// Returns the concerns that were blocked at the time of resolution.
    #[must_use]
    pub fn blocked_concerns(&self) -> &[ReviewConcern] {
        &self.blocked_concerns
    }

    /// Returns the reference path to the workspace search artifact.
    #[must_use]
    pub fn workspace_search_ref(&self) -> &str {
        self.workspace_search_ref.as_ref()
    }

    /// Returns the reference path to the reinvention-check artifact.
    #[must_use]
    pub fn reinvention_check_ref(&self) -> &str {
        self.reinvention_check_ref.as_ref()
    }

    /// Returns the decision made during resolution.
    #[must_use]
    pub fn decision(&self) -> ReviewEscalationDecision {
        self.decision
    }

    /// Returns the human-readable summary of the resolution.
    #[must_use]
    pub fn summary(&self) -> &str {
        self.summary.as_ref()
    }

    /// Returns the timestamp string of when the resolution was recorded.
    #[must_use]
    pub fn resolved_at(&self) -> &str {
        self.resolved_at.as_str()
    }
}

/// ADT representing the escalation phase.
///
/// `Clear` has no associated data. `Blocked` carries the block details directly,
/// making illegal states (e.g., "blocked but with no block data") unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationPhase {
    /// No active escalation block.
    Clear,
    /// Escalation is active; subsequent review operations are rejected.
    Blocked(ReviewEscalationBlock),
}

/// Aggregate escalation state composed into `ReviewState`.
///
/// Tracks streaks per concern across closed review cycles and transitions
/// to `EscalationPhase::Blocked` when a concern reaches `threshold` consecutive cycles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationState {
    pub(super) threshold: u8,
    pub(super) phase: EscalationPhase,
    pub(super) recent_cycles: Vec<ReviewCycleSummary>,
    pub(super) concern_streaks: BTreeMap<ReviewConcern, ReviewConcernStreak>,
    pub(super) last_resolution: Option<ReviewEscalationResolution>,
}

impl ReviewEscalationState {
    /// Creates a new `ReviewEscalationState` with default values.
    ///
    /// Threshold is 3, phase is `Clear`, no cycles or streaks recorded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            threshold: 3,
            phase: EscalationPhase::Clear,
            recent_cycles: Vec::new(),
            concern_streaks: BTreeMap::new(),
            last_resolution: None,
        }
    }

    /// Creates a `ReviewEscalationState` with all fields set explicitly.
    ///
    /// Used by codec deserialization.
    #[must_use]
    pub fn with_fields(
        threshold: u8,
        phase: EscalationPhase,
        recent_cycles: Vec<ReviewCycleSummary>,
        concern_streaks: BTreeMap<ReviewConcern, ReviewConcernStreak>,
        last_resolution: Option<ReviewEscalationResolution>,
    ) -> Self {
        Self { threshold, phase, recent_cycles, concern_streaks, last_resolution }
    }

    /// Returns the escalation threshold (number of consecutive cycles before blocking).
    #[must_use]
    pub fn threshold(&self) -> u8 {
        self.threshold
    }

    /// Returns the current escalation phase.
    #[must_use]
    pub fn phase(&self) -> &EscalationPhase {
        &self.phase
    }

    /// Returns the recent closed review cycle summaries (up to 10).
    #[must_use]
    pub fn recent_cycles(&self) -> &[ReviewCycleSummary] {
        &self.recent_cycles
    }

    /// Returns the per-concern streak tracking map.
    #[must_use]
    pub fn concern_streaks(&self) -> &BTreeMap<ReviewConcern, ReviewConcernStreak> {
        &self.concern_streaks
    }

    /// Returns the last escalation resolution record, if any.
    #[must_use]
    pub fn last_resolution(&self) -> Option<&ReviewEscalationResolution> {
        self.last_resolution.as_ref()
    }

    /// Returns `true` if the escalation phase is `Blocked`.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self.phase, EscalationPhase::Blocked(_))
    }
}

impl Default for ReviewEscalationState {
    fn default() -> Self {
        Self::new()
    }
}
