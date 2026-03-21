//! ReviewState aggregate root — the central state machine for track-level review workflows.

use std::collections::HashMap;

use super::concern::{ReviewConcern, ReviewConcernStreak, ReviewCycleSummary};
use super::error::ReviewError;
use super::escalation::{
    EscalationPhase, ReviewEscalationBlock, ReviewEscalationResolution, ReviewEscalationState,
};
use super::types::{
    CodeHash, ReviewGroupState, ReviewRoundResult, ReviewStatus, RoundType, Verdict,
};
use crate::{ReviewGroupName, Timestamp};

/// Aggregate review state for a track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewState {
    status: ReviewStatus,
    code_hash: CodeHash,
    groups: HashMap<ReviewGroupName, ReviewGroupState>,
    escalation: ReviewEscalationState,
}

impl Default for ReviewState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewState {
    /// Creates a new review state in `NotStarted` status.
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: ReviewStatus::NotStarted,
            code_hash: CodeHash::NotRecorded,
            groups: HashMap::new(),
            escalation: ReviewEscalationState::new(),
        }
    }

    /// Creates a review state with pre-set fields (used by codec deserialization).
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

    /// Returns the stored code hash string, if any.
    ///
    /// Returns `None` when hash is `NotRecorded` or `Pending`.
    #[must_use]
    pub fn code_hash(&self) -> Option<&str> {
        self.code_hash.as_str()
    }

    /// Returns a reference to the raw `CodeHash` ADT.
    #[must_use]
    pub fn code_hash_raw(&self) -> &CodeHash {
        &self.code_hash
    }

    /// Returns the code hash as a string suitable for serialization.
    ///
    /// - `NotRecorded` → `None`
    /// - `Pending` → `Some("PENDING")`
    /// - `Computed(s)` → `Some(s)`
    #[must_use]
    pub fn code_hash_for_serialization(&self) -> Option<&str> {
        match &self.code_hash {
            CodeHash::NotRecorded => None,
            CodeHash::Pending => Some("PENDING"),
            CodeHash::Computed(s) => Some(s),
        }
    }

    /// Returns the map of review group states.
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

    /// Records a review round result for a group.
    ///
    /// Validates escalation block, code hash freshness, and sequential escalation
    /// (fast before final). Promotes status when all expected groups report `zero_findings`.
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationActive` if escalation is blocked. Short-circuits before all
    ///   other checks.
    /// - `ReviewError::InvalidConcern` if verdict/concerns are inconsistent:
    ///   `zero_findings` with non-empty concerns, or `findings_remain` with empty concerns.
    /// - `ReviewError::StaleCodeHash` if stored code_hash doesn't match `current_code_hash`.
    ///   Sets status to `Invalidated` as a side effect.
    /// - `ReviewError::FinalRequiresFastPassed` if round_type is `Final` but status is not
    ///   `FastPassed`.
    pub fn record_round(
        &mut self,
        round_type: RoundType,
        group: &ReviewGroupName,
        result: ReviewRoundResult,
        expected_groups: &[ReviewGroupName],
        current_code_hash: &str,
    ) -> Result<(), ReviewError> {
        // 0. Escalation block check (short-circuit before all other checks).
        if let EscalationPhase::Blocked(ref block) = self.escalation.phase {
            return Err(ReviewError::EscalationActive {
                concerns: block.concerns.iter().map(|c| c.as_ref().to_owned()).collect(),
            });
        }

        // Deduplicate expected_groups to prevent one result satisfying multiple slots.
        let expected_groups: Vec<ReviewGroupName> = {
            let mut set = std::collections::BTreeSet::new();
            for g in expected_groups {
                set.insert(g.clone());
            }
            set.into_iter().collect()
        };
        let expected_groups = expected_groups.as_slice();

        // 0b. Verdict/concerns consistency check.
        Self::validate_verdict_concerns(&result)?;

        // 1. Code hash freshness check (applies to all round types).
        // NotRecorded means first round — skip freshness check.
        match &self.code_hash {
            CodeHash::NotRecorded => {}
            CodeHash::Pending => {
                // Two-phase protocol hasn't completed. Block like the old "PENDING" string.
                self.status = ReviewStatus::Invalidated;
                self.code_hash = CodeHash::NotRecorded;
                return Err(ReviewError::StaleCodeHash {
                    expected: "PENDING".to_owned(),
                    actual: current_code_hash.to_owned(),
                });
            }
            CodeHash::Computed(stored_str) => {
                if stored_str != current_code_hash {
                    let expected = stored_str.clone();
                    self.status = ReviewStatus::Invalidated;
                    self.code_hash = CodeHash::NotRecorded;
                    return Err(ReviewError::StaleCodeHash {
                        expected,
                        actual: current_code_hash.to_owned(),
                    });
                }
            }
        }

        // 2. Sequential escalation check (final requires fast_passed or approved)
        if round_type == RoundType::Final
            && self.status != ReviewStatus::FastPassed
            && self.status != ReviewStatus::Approved
        {
            return Err(ReviewError::FinalRequiresFastPassed(self.status));
        }

        // 3. Set/confirm code_hash (validated via computed())
        self.code_hash = CodeHash::computed(current_code_hash)
            .map_err(|e| ReviewError::InvalidConcern(format!("invalid code hash: {e}")))?;

        // Save timestamp before result is moved into group state.
        let timestamp = result.timestamp_value().clone();

        // 4. Record round result for the group.
        // When recording a fast round, clear any stale final_round for this group
        // since a new fast cycle invalidates previous final approvals.
        let group_state = self.groups.entry(group.clone()).or_default();
        match round_type {
            RoundType::Fast => group_state.record_fast(result),
            RoundType::Final => group_state.record_final(result),
        }

        // 5. Check promotion/demotion based on aggregated verdicts
        self.update_status_after_record(round_type, expected_groups);

        // 6. Update escalation state after recording.
        self.update_escalation_after_record(round_type, expected_groups, &timestamp);

        Ok(())
    }

    /// Records a review round with code_hash set to "PENDING" sentinel.
    ///
    /// Used in the normalized hash protocol (method D):
    /// 1. Caller computes pre-update normalized hash
    /// 2. This method: freshness check → record round → set code_hash to "PENDING"
    /// 3. Caller re-stages, computes post-update normalized hash H1
    /// 4. Caller calls set_code_hash(H1) to write back the real hash
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationActive` if escalation is blocked. Short-circuits before all
    ///   other checks.
    /// - `ReviewError::StaleCodeHash` if stored code_hash doesn't match `pre_update_hash`.
    ///   Skipped when stored code_hash is None (first round).
    /// - `ReviewError::FinalRequiresFastPassed` if round_type is Final but status is not
    ///   FastPassed/Approved.
    pub fn record_round_with_pending(
        &mut self,
        round_type: RoundType,
        group: &ReviewGroupName,
        result: ReviewRoundResult,
        expected_groups: &[ReviewGroupName],
        pre_update_hash: &str,
    ) -> Result<(), ReviewError> {
        // 0. Escalation block check (short-circuit before all other checks).
        if let EscalationPhase::Blocked(ref block) = self.escalation.phase {
            return Err(ReviewError::EscalationActive {
                concerns: block.concerns.iter().map(|c| c.as_ref().to_owned()).collect(),
            });
        }

        // Deduplicate expected_groups to prevent one result satisfying multiple slots.
        let expected_groups: Vec<ReviewGroupName> = {
            let mut set = std::collections::BTreeSet::new();
            for g in expected_groups {
                set.insert(g.clone());
            }
            set.into_iter().collect()
        };
        let expected_groups = expected_groups.as_slice();

        // 0b. Verdict/concerns consistency check.
        Self::validate_verdict_concerns(&result)?;

        // 1. Code hash freshness check — skip if NotRecorded (first round).
        let saved_hash = self.code_hash.clone();
        match &self.code_hash {
            CodeHash::NotRecorded => {}
            CodeHash::Pending => {
                // Previous two-phase protocol hasn't completed.
                self.status = ReviewStatus::Invalidated;
                self.code_hash = CodeHash::NotRecorded;
                return Err(ReviewError::StaleCodeHash {
                    expected: "PENDING".to_owned(),
                    actual: pre_update_hash.to_owned(),
                });
            }
            CodeHash::Computed(stored_str) => {
                if stored_str != pre_update_hash {
                    let expected = stored_str.clone();
                    self.status = ReviewStatus::Invalidated;
                    self.code_hash = CodeHash::NotRecorded;
                    return Err(ReviewError::StaleCodeHash {
                        expected,
                        actual: pre_update_hash.to_owned(),
                    });
                }
            }
        }

        // 2. Sequential escalation check (final requires fast_passed or approved).
        // Restore code_hash on failure so the next retry still has a valid
        // freshness baseline rather than skipping the check as if first-round.
        if round_type == RoundType::Final
            && self.status != ReviewStatus::FastPassed
            && self.status != ReviewStatus::Approved
        {
            self.code_hash = saved_hash;
            return Err(ReviewError::FinalRequiresFastPassed(self.status));
        }

        // 3. Set code_hash to the PENDING sentinel
        self.code_hash = CodeHash::Pending;

        // Save timestamp before result is moved into group state.
        let timestamp = result.timestamp_value().clone();

        // 4. Record round result for the group.
        let group_state = self.groups.entry(group.clone()).or_default();
        match round_type {
            RoundType::Fast => group_state.record_fast(result),
            RoundType::Final => group_state.record_final(result),
        }

        // 5. Check promotion/demotion based on aggregated verdicts
        self.update_status_after_record(round_type, expected_groups);

        // 6. Update escalation state after recording.
        self.update_escalation_after_record(round_type, expected_groups, &timestamp);

        Ok(())
    }

    /// Validates that verdict and concerns are consistent.
    ///
    /// # Errors
    ///
    /// - `ReviewError::InvalidConcern` if `zero_findings` verdict has non-empty concerns.
    /// - `ReviewError::InvalidConcern` if `findings_remain` verdict has empty concerns.
    fn validate_verdict_concerns(result: &ReviewRoundResult) -> Result<(), ReviewError> {
        if result.verdict().is_zero_findings() && !result.concerns().is_empty() {
            return Err(ReviewError::InvalidConcern(
                "zero_findings verdict must have empty concerns".to_owned(),
            ));
        }
        if result.verdict() == Verdict::FindingsRemain && result.concerns().is_empty() {
            return Err(ReviewError::InvalidConcern(
                "findings_remain verdict must have non-empty concerns".to_owned(),
            ));
        }
        Ok(())
    }

    /// Promotes or demotes review status based on aggregated group verdicts.
    fn update_status_after_record(
        &mut self,
        round_type: RoundType,
        expected_groups: &[ReviewGroupName],
    ) {
        let all_expected_zero = expected_groups.iter().all(|eg| {
            self.groups.get(eg).is_some_and(|gs| {
                let round_result = match round_type {
                    RoundType::Fast => gs.fast(),
                    RoundType::Final => gs.final_round(),
                };
                round_result.is_some_and(|r| r.verdict().is_zero_findings())
            })
        });

        if all_expected_zero {
            match round_type {
                RoundType::Fast => self.status = ReviewStatus::FastPassed,
                RoundType::Final => self.status = ReviewStatus::Approved,
            }
        } else {
            match round_type {
                RoundType::Fast => {
                    if self.status == ReviewStatus::FastPassed
                        || self.status == ReviewStatus::Approved
                    {
                        self.status = ReviewStatus::NotStarted;
                    }
                }
                RoundType::Final => {
                    if self.status == ReviewStatus::Approved {
                        self.status = ReviewStatus::FastPassed;
                    }
                }
            }
        }
    }

    /// Called after recording a round result. Checks if a closed cycle is complete
    /// and updates escalation state accordingly.
    fn update_escalation_after_record(
        &mut self,
        round_type: RoundType,
        expected_groups: &[ReviewGroupName],
        timestamp: &Timestamp,
    ) {
        // 1. Check if cycle is closed: all expected groups have recorded this round_type
        //    with the same round number.
        let round_numbers: Vec<Option<u32>> = expected_groups
            .iter()
            .map(|g| {
                self.groups.get(g).and_then(|gs| {
                    let rr = match round_type {
                        RoundType::Fast => gs.fast(),
                        RoundType::Final => gs.final_round(),
                    };
                    rr.map(|r| r.round())
                })
            })
            .collect();

        // All groups must have a result, and all must have the same round number.
        let first = match round_numbers.first() {
            Some(Some(n)) => *n,
            _ => return,
        };
        if !round_numbers.iter().all(|n| *n == Some(first)) {
            return;
        }

        // 1b. Duplicate cycle detection: if this (round_type, round) was already counted,
        //     skip to prevent double-counting when a group re-records the same round.
        let already_counted = self
            .escalation
            .recent_cycles
            .iter()
            .any(|c| c.round_type() == round_type && c.round() == first);
        if already_counted {
            return;
        }

        // 2. Collect concerns from all groups for this cycle (union via BTreeSet for dedup).
        let mut cycle_concerns_set = std::collections::BTreeSet::new();
        let mut group_names = Vec::new();
        for g in expected_groups {
            group_names.push(g.clone());
            if let Some(gs) = self.groups.get(g) {
                let rr = match round_type {
                    RoundType::Fast => gs.fast(),
                    RoundType::Final => gs.final_round(),
                };
                if let Some(r) = rr {
                    for c in r.concerns() {
                        cycle_concerns_set.insert(c.clone());
                    }
                }
            }
        }
        let cycle_concerns_vec: Vec<ReviewConcern> = cycle_concerns_set.iter().cloned().collect();

        // 3. Update concern_streaks.
        // Increment streaks for concerns present in this cycle.
        for concern in &cycle_concerns_vec {
            let streak =
                self.escalation.concern_streaks.entry(concern.clone()).or_insert_with(|| {
                    ReviewConcernStreak::new(0, round_type, first, timestamp.clone())
                });
            *streak = ReviewConcernStreak::new(
                streak.consecutive_rounds().saturating_add(1),
                round_type,
                first,
                timestamp.clone(),
            );
        }
        // Reset streaks for concerns NOT present in this cycle.
        self.escalation.concern_streaks.retain(|k, _| cycle_concerns_set.contains(k));

        // 4. Add to recent_cycles (FIFO, max 10).
        let summary = ReviewCycleSummary::new(
            round_type,
            first,
            timestamp.clone(),
            cycle_concerns_vec,
            group_names,
        );
        self.escalation.recent_cycles.push(summary);
        if self.escalation.recent_cycles.len() > 10 {
            self.escalation.recent_cycles.remove(0);
        }

        // 5. Check threshold → transition to Blocked if any concern streak >= threshold.
        let threshold = self.escalation.threshold;
        let blocked_concerns: Vec<ReviewConcern> = self
            .escalation
            .concern_streaks
            .iter()
            .filter(|(_, s)| s.consecutive_rounds() >= threshold)
            .map(|(k, _)| k.clone())
            .collect();

        if !blocked_concerns.is_empty() {
            self.escalation.phase = EscalationPhase::Blocked(ReviewEscalationBlock::new(
                blocked_concerns,
                timestamp.clone(),
            ));
        }
    }

    /// Sets the code_hash to the given computed value.
    ///
    /// Validates via `CodeHash::computed()`: trims whitespace, rejects empty strings
    /// and the reserved literal `"PENDING"`.
    ///
    /// Used in the normalized hash protocol to write back the computed hash
    /// after record_round_with_pending + re-stage + hash computation.
    ///
    /// # Errors
    ///
    /// Returns `ReviewError::InvalidConcern` if the hash is empty, whitespace-only,
    /// or the reserved `"PENDING"` literal.
    pub fn set_code_hash(&mut self, hash: String) -> Result<(), ReviewError> {
        self.code_hash = CodeHash::computed(hash)?;
        Ok(())
    }

    /// Checks if the review state is ready for commit.
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationActive` if escalation is blocked. Short-circuits before all
    ///   other checks.
    /// - `ReviewError::NotApproved` if status is not `Approved`.
    /// - `ReviewError::StaleCodeHash` if code_hash doesn't match. Sets status to
    ///   `Invalidated` as a side effect.
    pub fn check_commit_ready(&mut self, current_code_hash: &str) -> Result<(), ReviewError> {
        // 0. Escalation block check (short-circuit before all other checks).
        if let EscalationPhase::Blocked(ref block) = self.escalation.phase {
            return Err(ReviewError::EscalationActive {
                concerns: block.concerns.iter().map(|c| c.as_ref().to_owned()).collect(),
            });
        }

        if self.status != ReviewStatus::Approved {
            return Err(ReviewError::NotApproved(self.status));
        }
        match &self.code_hash {
            CodeHash::NotRecorded => {}
            CodeHash::Pending => {
                // Pending means the two-phase protocol hasn't completed.
                // Reject commit — the real hash was never written back.
                self.status = ReviewStatus::Invalidated;
                self.code_hash = CodeHash::NotRecorded;
                return Err(ReviewError::StaleCodeHash {
                    expected: "PENDING".to_owned(),
                    actual: current_code_hash.to_owned(),
                });
            }
            CodeHash::Computed(stored_str) => {
                if stored_str != current_code_hash {
                    let expected = stored_str.clone();
                    self.status = ReviewStatus::Invalidated;
                    self.code_hash = CodeHash::NotRecorded;
                    return Err(ReviewError::StaleCodeHash {
                        expected,
                        actual: current_code_hash.to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Invalidates the review state (e.g., when code changes are detected).
    ///
    /// Clears `code_hash` so that a subsequent `record_round` with the new hash
    /// is accepted (fresh start), preventing permanent deadlock.
    pub fn invalidate(&mut self) {
        self.status = ReviewStatus::Invalidated;
        self.code_hash = CodeHash::NotRecorded;
    }

    /// Resolves an active escalation block.
    ///
    /// Requires evidence references and a decision. On success:
    /// - clears streak state
    /// - stores the resolution record
    /// - sets `ReviewStatus::Invalidated` and clears `code_hash` (fresh start)
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationNotActive` if no escalation block is active.
    /// - `ReviewError::ResolutionEvidenceMissing` if `workspace_search_ref` or
    ///   `reinvention_check_ref` is empty.
    /// - `ReviewError::ResolutionConcernMismatch` if the resolution's `blocked_concerns`
    ///   do not match the active block's concerns.
    pub fn resolve_escalation(
        &mut self,
        resolution: ReviewEscalationResolution,
    ) -> Result<(), ReviewError> {
        // Verify escalation is active
        let block = match &self.escalation.phase {
            EscalationPhase::Blocked(b) => b.clone(),
            EscalationPhase::Clear => return Err(ReviewError::EscalationNotActive),
        };

        // Evidence fields (workspace_search_ref, reinvention_check_ref, summary)
        // are validated at construction time via NonEmptyString.

        // Validate concerns match (order-insensitive: sort both before comparing).
        let mut expected: Vec<String> =
            block.concerns.iter().map(|c| c.as_ref().to_owned()).collect();
        let mut actual: Vec<String> =
            resolution.blocked_concerns.iter().map(|c| c.as_ref().to_owned()).collect();
        expected.sort();
        actual.sort();
        if expected != actual {
            return Err(ReviewError::ResolutionConcernMismatch { expected, actual });
        }

        // Apply resolution: clear streaks, store resolution, invalidate review
        self.escalation.concern_streaks.clear();
        self.escalation.phase = EscalationPhase::Clear;
        self.escalation.last_resolution = Some(resolution);
        self.status = ReviewStatus::Invalidated;
        self.code_hash = CodeHash::NotRecorded;

        Ok(())
    }
}
