//! Review escalation state types.

use std::collections::BTreeMap;

use super::concern::ReviewConcern;
use crate::Timestamp;

/// Phase of the review escalation state machine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EscalationPhase {
    /// No active escalation block.
    #[default]
    Clear,
    /// An escalation block is active for the given concerns.
    Blocked(ReviewEscalationBlock),
}

/// An active escalation block with the concerns that triggered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationBlock {
    concerns: Vec<ReviewConcern>,
    blocked_at: Timestamp,
}

impl ReviewEscalationBlock {
    /// Creates a new escalation block.
    #[must_use]
    pub fn new(concerns: Vec<ReviewConcern>, blocked_at: Timestamp) -> Self {
        Self { concerns, blocked_at }
    }

    /// Returns the concerns that triggered this block.
    #[must_use]
    pub fn concerns(&self) -> &[ReviewConcern] {
        &self.concerns
    }

    /// Returns the timestamp when the block was created.
    #[must_use]
    pub fn blocked_at(&self) -> &str {
        self.blocked_at.as_str()
    }
}

/// Decision made when resolving an escalation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEscalationDecision {
    AdoptWorkspaceSolution,
    AdoptExternalCrate,
    ContinueSelfImplementation,
}

/// Resolution of an escalation block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationResolution {
    blocked_concerns: Vec<ReviewConcern>,
    workspace_search_ref: String,
    reinvention_check_ref: String,
    decision: ReviewEscalationDecision,
    summary: String,
    resolved_at: Timestamp,
}

impl ReviewEscalationResolution {
    /// Creates a new resolution.
    ///
    /// # Errors
    /// Returns an error string if `blocked_concerns` is empty.
    pub fn new(
        blocked_concerns: Vec<ReviewConcern>,
        workspace_search_ref: impl Into<String>,
        reinvention_check_ref: impl Into<String>,
        decision: ReviewEscalationDecision,
        summary: impl Into<String>,
        resolved_at: Timestamp,
    ) -> Result<Self, String> {
        if blocked_concerns.is_empty() {
            return Err("blocked_concerns must not be empty".to_owned());
        }
        let workspace_search_ref = workspace_search_ref.into();
        if workspace_search_ref.trim().is_empty() {
            return Err("workspace_search_ref must not be empty".to_owned());
        }
        let reinvention_check_ref = reinvention_check_ref.into();
        if reinvention_check_ref.trim().is_empty() {
            return Err("reinvention_check_ref must not be empty".to_owned());
        }
        let summary = summary.into();
        if summary.trim().is_empty() {
            return Err("summary must not be empty".to_owned());
        }
        Ok(Self {
            blocked_concerns,
            workspace_search_ref,
            reinvention_check_ref,
            decision,
            summary,
            resolved_at,
        })
    }

    /// Returns the blocked concerns.
    #[must_use]
    pub fn blocked_concerns(&self) -> &[ReviewConcern] {
        &self.blocked_concerns
    }

    /// Returns the workspace search artifact reference.
    #[must_use]
    pub fn workspace_search_ref(&self) -> &str {
        &self.workspace_search_ref
    }

    /// Returns the reinvention check artifact reference.
    #[must_use]
    pub fn reinvention_check_ref(&self) -> &str {
        &self.reinvention_check_ref
    }

    /// Returns the decision.
    #[must_use]
    pub fn decision(&self) -> ReviewEscalationDecision {
        self.decision
    }

    /// Returns the summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Returns the resolution timestamp.
    #[must_use]
    pub fn resolved_at(&self) -> &str {
        self.resolved_at.as_str()
    }
}

/// Full escalation state including threshold, phase, and history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationState {
    threshold: u8,
    phase: EscalationPhase,
    recent_cycles: Vec<super::concern::ReviewCycleSummary>,
    concern_streaks: BTreeMap<ReviewConcern, super::concern::ReviewConcernStreak>,
    last_resolution: Option<ReviewEscalationResolution>,
}

impl Default for ReviewEscalationState {
    fn default() -> Self {
        Self {
            threshold: 3,
            phase: EscalationPhase::Clear,
            recent_cycles: Vec::new(),
            concern_streaks: BTreeMap::new(),
            last_resolution: None,
        }
    }
}

impl ReviewEscalationState {
    /// Creates a new state with all fields specified (used by codec).
    #[must_use]
    pub fn with_fields(
        threshold: u8,
        phase: EscalationPhase,
        recent_cycles: Vec<super::concern::ReviewCycleSummary>,
        concern_streaks: BTreeMap<ReviewConcern, super::concern::ReviewConcernStreak>,
        last_resolution: Option<ReviewEscalationResolution>,
    ) -> Self {
        Self { threshold, phase, recent_cycles, concern_streaks, last_resolution }
    }

    /// Returns the escalation threshold.
    #[must_use]
    pub fn threshold(&self) -> u8 {
        self.threshold
    }

    /// Returns the current phase.
    #[must_use]
    pub fn phase(&self) -> &EscalationPhase {
        &self.phase
    }

    /// Returns `true` if the escalation is blocked.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self.phase, EscalationPhase::Blocked(_))
    }

    /// Returns recent cycle summaries.
    #[must_use]
    pub fn recent_cycles(&self) -> &[super::concern::ReviewCycleSummary] {
        &self.recent_cycles
    }

    /// Returns concern streaks.
    #[must_use]
    pub fn concern_streaks(&self) -> &BTreeMap<ReviewConcern, super::concern::ReviewConcernStreak> {
        &self.concern_streaks
    }

    /// Returns the last resolution, if any.
    #[must_use]
    pub fn last_resolution(&self) -> Option<&ReviewEscalationResolution> {
        self.last_resolution.as_ref()
    }

    /// Returns a mutable reference to recent cycle summaries.
    pub fn recent_cycles_mut(&mut self) -> &mut Vec<super::concern::ReviewCycleSummary> {
        &mut self.recent_cycles
    }

    /// Returns a mutable reference to concern streaks.
    pub fn concern_streaks_mut(
        &mut self,
    ) -> &mut BTreeMap<ReviewConcern, super::concern::ReviewConcernStreak> {
        &mut self.concern_streaks
    }

    /// Resolves the active escalation block.
    ///
    /// # Errors
    /// Returns an error string if there is no active escalation block.
    pub fn resolve(&mut self, resolution: ReviewEscalationResolution) -> Result<(), String> {
        if !self.is_blocked() {
            return Err("no active escalation block to resolve".to_owned());
        }
        self.last_resolution = Some(resolution);
        self.phase = EscalationPhase::Clear;
        self.recent_cycles.clear();
        self.concern_streaks.clear();
        Ok(())
    }
}
