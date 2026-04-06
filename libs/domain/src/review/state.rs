//! V1 review state stored in metadata.json (escalation + per-group round tracking).

use std::collections::HashMap;

use super::concern::ReviewConcern;
use super::error::ReviewError;
use super::escalation::ReviewEscalationState;
use super::types::ReviewRoundResult;
use super::types::{CodeHash, ReviewGroupState, ReviewStatus, RoundType, Verdict};
use crate::ids::ReviewGroupName;

/// V1 review state stored in metadata.json.
///
/// Tracks per-group round results, overall status, code hash, and escalation state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReviewState {
    status: ReviewStatus,
    code_hash: CodeHash,
    groups: HashMap<ReviewGroupName, ReviewGroupState>,
    escalation: ReviewEscalationState,
}

impl ReviewState {
    /// Creates a new empty review state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a review state with all fields (used by codec).
    #[must_use]
    pub fn with_fields(
        status: ReviewStatus,
        code_hash: CodeHash,
        groups: HashMap<ReviewGroupName, ReviewGroupState>,
        escalation: ReviewEscalationState,
    ) -> Self {
        Self { status, code_hash, groups, escalation }
    }

    /// Returns the current review status.
    #[must_use]
    pub fn status(&self) -> ReviewStatus {
        self.status
    }

    /// Returns the current code hash.
    #[must_use]
    pub fn code_hash(&self) -> Option<&str> {
        self.code_hash.as_str()
    }

    /// Returns the code hash for serialization (includes "PENDING" literal).
    #[must_use]
    pub fn code_hash_for_serialization(&self) -> Option<&str> {
        match &self.code_hash {
            CodeHash::NotRecorded => None,
            CodeHash::Pending => Some("PENDING"),
            CodeHash::Computed(s) => Some(s.as_str()),
        }
    }

    /// Returns the per-group review states.
    #[must_use]
    pub fn groups(&self) -> &HashMap<ReviewGroupName, ReviewGroupState> {
        &self.groups
    }

    /// Returns the escalation state.
    #[must_use]
    pub fn escalation(&self) -> &ReviewEscalationState {
        &self.escalation
    }

    /// Returns a mutable reference to the escalation state.
    pub fn escalation_mut(&mut self) -> &mut ReviewEscalationState {
        &mut self.escalation
    }

    /// Records a review round with a pending code hash (two-phase protocol).
    ///
    /// Updates the group state and transitions the overall status.
    ///
    /// # Errors
    /// Returns `ReviewError::StaleCodeHash` if `pre_hash` does not match the stored hash.
    pub fn record_round_with_pending(
        &mut self,
        round_type: RoundType,
        group_name: &ReviewGroupName,
        result: ReviewRoundResult,
        _expected_groups: &[ReviewGroupName],
        pre_hash: &str,
    ) -> Result<(), ReviewError> {
        // Check staleness: if we have a computed hash, the pre_hash must match.
        if let CodeHash::Computed(stored) = &self.code_hash {
            if stored.as_str() != pre_hash {
                return Err(ReviewError::StaleCodeHash {
                    stored: stored.clone(),
                    current: pre_hash.to_owned(),
                });
            }
        }

        let group = self.groups.entry(group_name.clone()).or_default();
        match round_type {
            RoundType::Fast => group.record_fast(result),
            RoundType::Final => group.record_final(result),
        }

        // Update status based on round type and verdict.
        self.code_hash = CodeHash::Pending;
        self.status = match round_type {
            RoundType::Fast => {
                if self.all_groups_fast_passed() {
                    ReviewStatus::FastPassed
                } else {
                    ReviewStatus::Invalidated
                }
            }
            RoundType::Final => {
                if self.all_groups_approved() {
                    ReviewStatus::Approved
                } else {
                    ReviewStatus::Invalidated
                }
            }
        };

        Ok(())
    }

    /// Sets the computed code hash (second phase of two-phase protocol).
    ///
    /// # Errors
    /// Returns `ReviewError::InvalidConcern` if the hash is empty.
    pub fn set_code_hash(&mut self, hash: impl Into<String>) -> Result<(), ReviewError> {
        self.code_hash = CodeHash::computed(hash)?;
        Ok(())
    }

    /// Checks whether the current code hash matches and the status is approved.
    ///
    /// # Errors
    /// Returns `ReviewError::StaleCodeHash` if the hash doesn't match,
    /// or `ReviewError::InvalidConcern` if the review is not approved.
    pub fn check_commit_ready(&self, current_hash: &str) -> Result<(), ReviewError> {
        if self.status != ReviewStatus::Approved {
            return Err(ReviewError::InvalidConcern(format!(
                "review status is {:?}, not approved",
                self.status
            )));
        }
        if let CodeHash::Computed(stored) = &self.code_hash {
            if stored.as_str() != current_hash {
                return Err(ReviewError::StaleCodeHash {
                    stored: stored.clone(),
                    current: current_hash.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Resolves an active escalation block.
    ///
    /// # Errors
    /// Returns an error string if there is no active block or concerns don't match.
    pub fn resolve_escalation(
        &mut self,
        resolution: super::escalation::ReviewEscalationResolution,
    ) -> Result<(), String> {
        self.escalation.resolve(resolution)?;
        // Invalidate status so a fresh review is required after escalation resolution.
        self.status = ReviewStatus::Invalidated;
        self.code_hash = CodeHash::NotRecorded;
        Ok(())
    }

    fn all_groups_fast_passed(&self) -> bool {
        if self.groups.is_empty() {
            return false;
        }
        self.groups.values().all(|g| g.fast().is_some_and(|r| r.verdict() == Verdict::ZeroFindings))
    }

    fn all_groups_approved(&self) -> bool {
        if self.groups.is_empty() {
            return false;
        }
        self.groups
            .values()
            .all(|g| g.final_round().is_some_and(|r| r.verdict() == Verdict::ZeroFindings))
    }

    /// Records a concern streak update (used by record-round protocol).
    pub fn record_concern_streak(
        &mut self,
        concern: ReviewConcern,
        streak: super::concern::ReviewConcernStreak,
    ) {
        self.escalation.concern_streaks_mut().insert(concern, streak);
    }

    /// Adds a cycle summary (used by record-round protocol).
    pub fn push_cycle_summary(&mut self, summary: super::concern::ReviewCycleSummary) {
        self.escalation.recent_cycles_mut().push(summary);
    }
}
