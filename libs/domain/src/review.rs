//! Review state management for track-level review workflows.
//!
//! Tracks review progress through a state machine:
//! `NotStarted` → `FastPassed` → `Approved`, with `Invalidated` on code changes.

use std::collections::HashMap;
use std::fmt;

use thiserror::Error;

/// Errors from review state operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewError {
    #[error("final round requires review status fast_passed, but current status is {0}")]
    FinalRequiresFastPassed(ReviewStatus),

    #[error("code hash mismatch: review recorded against {expected}, but current code is {actual}")]
    StaleCodeHash { expected: String, actual: String },

    #[error("review status is {0}, not approved")]
    NotApproved(ReviewStatus),
}

/// Review status enum with explicit states (no null).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReviewStatus {
    #[default]
    NotStarted,
    Invalidated,
    FastPassed,
    Approved,
}

impl fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotStarted => "not_started",
            Self::Invalidated => "invalidated",
            Self::FastPassed => "fast_passed",
            Self::Approved => "approved",
        })
    }
}

/// Round type discriminant for review rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundType {
    Fast,
    Final,
}

impl fmt::Display for RoundType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fast => "fast",
            Self::Final => "final",
        })
    }
}

/// Result of a single review round for a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRoundResult {
    round: u32,
    verdict: String,
    timestamp: String,
}

impl ReviewRoundResult {
    #[must_use]
    pub fn new(round: u32, verdict: impl Into<String>, timestamp: impl Into<String>) -> Self {
        Self { round, verdict: verdict.into(), timestamp: timestamp.into() }
    }

    #[must_use]
    pub fn round(&self) -> u32 {
        self.round
    }

    #[must_use]
    pub fn verdict(&self) -> &str {
        &self.verdict
    }

    #[must_use]
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }
}

/// State of a named review group, tracking fast and final round results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewGroupState {
    fast: Option<ReviewRoundResult>,
    final_round: Option<ReviewRoundResult>,
}

impl ReviewGroupState {
    #[must_use]
    pub fn fast(&self) -> Option<&ReviewRoundResult> {
        self.fast.as_ref()
    }

    #[must_use]
    pub fn final_round(&self) -> Option<&ReviewRoundResult> {
        self.final_round.as_ref()
    }

    /// Creates a group state with only a fast round result.
    #[must_use]
    pub fn with_fast(result: ReviewRoundResult) -> Self {
        Self { fast: Some(result), final_round: None }
    }

    /// Creates a group state with only a final round result.
    #[must_use]
    pub fn with_final_only(result: ReviewRoundResult) -> Self {
        Self { fast: None, final_round: Some(result) }
    }

    /// Creates a group state with both fast and final round results.
    #[must_use]
    pub fn with_both(fast: ReviewRoundResult, final_round: ReviewRoundResult) -> Self {
        Self { fast: Some(fast), final_round: Some(final_round) }
    }
}

/// Aggregate review state for a track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewState {
    status: ReviewStatus,
    code_hash: Option<String>,
    groups: HashMap<String, ReviewGroupState>,
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
        Self { status: ReviewStatus::NotStarted, code_hash: None, groups: HashMap::new() }
    }

    /// Creates a review state with pre-set fields (used by codec deserialization).
    #[must_use]
    pub fn with_fields(
        status: ReviewStatus,
        code_hash: Option<String>,
        groups: HashMap<String, ReviewGroupState>,
    ) -> Self {
        Self { status, code_hash, groups }
    }

    #[must_use]
    pub fn status(&self) -> ReviewStatus {
        self.status
    }

    #[must_use]
    pub fn code_hash(&self) -> Option<&str> {
        self.code_hash.as_deref()
    }

    #[must_use]
    pub fn groups(&self) -> &HashMap<String, ReviewGroupState> {
        &self.groups
    }

    /// Records a review round result for a group.
    ///
    /// Validates code hash freshness and sequential escalation (fast before final).
    /// Promotes status when all expected groups report `zero_findings`.
    ///
    /// # Errors
    ///
    /// - `ReviewError::StaleCodeHash` if stored code_hash doesn't match `current_code_hash`.
    ///   Sets status to `Invalidated` as a side effect.
    /// - `ReviewError::FinalRequiresFastPassed` if round_type is `Final` but status is not
    ///   `FastPassed`.
    pub fn record_round(
        &mut self,
        round_type: RoundType,
        group: &str,
        result: ReviewRoundResult,
        expected_groups: &[String],
        current_code_hash: &str,
    ) -> Result<(), ReviewError> {
        // 1. Code hash freshness check (applies to all round types).
        // Clear code_hash on mismatch so a subsequent call with the new hash succeeds.
        if let Some(stored_hash) = self.code_hash.take() {
            if stored_hash != current_code_hash {
                self.status = ReviewStatus::Invalidated;
                // code_hash already cleared by take()
                return Err(ReviewError::StaleCodeHash {
                    expected: stored_hash,
                    actual: current_code_hash.to_owned(),
                });
            }
            // Restore hash if it matched
            self.code_hash = Some(stored_hash);
        }

        // 2. Sequential escalation check (final requires fast_passed or approved)
        if round_type == RoundType::Final
            && self.status != ReviewStatus::FastPassed
            && self.status != ReviewStatus::Approved
        {
            return Err(ReviewError::FinalRequiresFastPassed(self.status));
        }

        // 3. Set/confirm code_hash
        self.code_hash = Some(current_code_hash.to_owned());

        // 4. Record round result for the group.
        // When recording a fast round, clear any stale final_round for this group
        // since a new fast cycle invalidates previous final approvals.
        let group_state = self.groups.entry(group.to_owned()).or_default();
        match round_type {
            RoundType::Fast => {
                group_state.fast = Some(result);
                group_state.final_round = None;
            }
            RoundType::Final => group_state.final_round = Some(result),
        }

        // 5. Check promotion/demotion based on aggregated verdicts
        let all_expected_zero = expected_groups.iter().all(|eg| {
            self.groups.get(eg).is_some_and(|gs| {
                let round_result = match round_type {
                    RoundType::Fast => gs.fast.as_ref(),
                    RoundType::Final => gs.final_round.as_ref(),
                };
                round_result.is_some_and(|r| r.verdict == "zero_findings")
            })
        });

        if all_expected_zero {
            match round_type {
                RoundType::Fast => self.status = ReviewStatus::FastPassed,
                RoundType::Final => self.status = ReviewStatus::Approved,
            }
        } else {
            // Demote if current status is higher than what this round type warrants.
            // A fast round with findings should not leave status at FastPassed or Approved.
            // A final round with findings should not leave status at Approved.
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
    /// - `ReviewError::StaleCodeHash` if stored code_hash doesn't match `pre_update_hash`.
    ///   Skipped when stored code_hash is None (first round).
    /// - `ReviewError::FinalRequiresFastPassed` if round_type is Final but status is not
    ///   FastPassed/Approved.
    pub fn record_round_with_pending(
        &mut self,
        round_type: RoundType,
        group: &str,
        result: ReviewRoundResult,
        expected_groups: &[String],
        pre_update_hash: &str,
    ) -> Result<(), ReviewError> {
        // 1. Code hash freshness check — skip if None (first round).
        let taken_hash = self.code_hash.take();
        if let Some(ref stored_hash) = taken_hash {
            if stored_hash != pre_update_hash {
                self.status = ReviewStatus::Invalidated;
                return Err(ReviewError::StaleCodeHash {
                    expected: stored_hash.clone(),
                    actual: pre_update_hash.to_owned(),
                });
            }
            // hash matched — code_hash cleared by take(); will be set to PENDING below
        }

        // 2. Sequential escalation check (final requires fast_passed or approved).
        // Restore code_hash on failure so the next retry still has a valid
        // freshness baseline rather than skipping the check as if first-round.
        if round_type == RoundType::Final
            && self.status != ReviewStatus::FastPassed
            && self.status != ReviewStatus::Approved
        {
            self.code_hash = taken_hash;
            return Err(ReviewError::FinalRequiresFastPassed(self.status));
        }

        // 3. Set code_hash to the PENDING sentinel
        self.code_hash = Some("PENDING".to_owned());

        // 4. Record round result for the group.
        let group_state = self.groups.entry(group.to_owned()).or_default();
        match round_type {
            RoundType::Fast => {
                group_state.fast = Some(result);
                group_state.final_round = None;
            }
            RoundType::Final => group_state.final_round = Some(result),
        }

        // 5. Check promotion/demotion based on aggregated verdicts
        let all_expected_zero = expected_groups.iter().all(|eg| {
            self.groups.get(eg).is_some_and(|gs| {
                let round_result = match round_type {
                    RoundType::Fast => gs.fast.as_ref(),
                    RoundType::Final => gs.final_round.as_ref(),
                };
                round_result.is_some_and(|r| r.verdict == "zero_findings")
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

        Ok(())
    }

    /// Sets the code_hash to the given value.
    ///
    /// Used in the normalized hash protocol to write back the computed hash
    /// after record_round_with_pending + re-stage + hash computation.
    pub fn set_code_hash(&mut self, hash: String) {
        self.code_hash = Some(hash);
    }

    /// Checks if the review state is ready for commit.
    ///
    /// # Errors
    ///
    /// - `ReviewError::NotApproved` if status is not `Approved`.
    /// - `ReviewError::StaleCodeHash` if code_hash doesn't match. Sets status to
    ///   `Invalidated` as a side effect.
    pub fn check_commit_ready(&mut self, current_code_hash: &str) -> Result<(), ReviewError> {
        if self.status != ReviewStatus::Approved {
            return Err(ReviewError::NotApproved(self.status));
        }
        if let Some(stored_hash) = &self.code_hash {
            if stored_hash != current_code_hash {
                self.status = ReviewStatus::Invalidated;
                return Err(ReviewError::StaleCodeHash {
                    expected: stored_hash.clone(),
                    actual: current_code_hash.to_owned(),
                });
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
        self.code_hash = None;
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn round(verdict: &str) -> ReviewRoundResult {
        ReviewRoundResult::new(1, verdict, "2026-03-18T00:00:00Z")
    }

    fn zero() -> ReviewRoundResult {
        round("zero_findings")
    }

    fn findings() -> ReviewRoundResult {
        round("findings_remain")
    }

    // --- ReviewStatus tests ---

    #[test]
    fn test_review_status_default_is_not_started() {
        assert_eq!(ReviewStatus::default(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_review_status_display() {
        assert_eq!(ReviewStatus::NotStarted.to_string(), "not_started");
        assert_eq!(ReviewStatus::Invalidated.to_string(), "invalidated");
        assert_eq!(ReviewStatus::FastPassed.to_string(), "fast_passed");
        assert_eq!(ReviewStatus::Approved.to_string(), "approved");
    }

    // --- ReviewState::new tests ---

    #[test]
    fn test_review_state_new_has_not_started_status() {
        let state = ReviewState::new();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
        assert!(state.code_hash().is_none());
        assert!(state.groups().is_empty());
    }

    // --- record_round: fast round recording ---

    #[test]
    fn test_record_fast_zero_findings_for_single_group_promotes_to_fast_passed() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("abc123"));
        assert!(state.groups().get("group-a").unwrap().fast().is_some());
    }

    #[test]
    fn test_record_fast_partial_groups_does_not_promote() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        // Only one of two expected groups recorded — no promotion
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_record_fast_all_groups_zero_findings_promotes() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    #[test]
    fn test_record_fast_findings_remain_blocks_promotion() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        // group-a has findings_remain — no promotion
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_record_fast_does_not_overwrite_other_groups() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        // Both groups should exist
        assert!(state.groups().get("group-a").unwrap().fast().is_some());
        assert!(state.groups().get("group-b").unwrap().fast().is_some());
    }

    // --- record_round: final round recording ---

    #[test]
    fn test_record_final_after_fast_passed_succeeds() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);
    }

    #[test]
    fn test_record_final_without_fast_passed_fails() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123");

        assert!(matches!(
            result,
            Err(ReviewError::FinalRequiresFastPassed(ReviewStatus::NotStarted))
        ));
    }

    #[test]
    fn test_record_final_partial_groups_does_not_promote() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        // Fast pass both
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Final only for group-a
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed); // Not promoted yet
    }

    #[test]
    fn test_record_final_findings_remain_blocks_promotion() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        state.record_round(RoundType::Final, "group-a", findings(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-b", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed); // findings in A blocks
    }

    // --- record_round: code hash validation ---

    #[test]
    fn test_record_round_stale_code_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        let result = state.record_round(RoundType::Fast, "group-a", zero(), &expected, "def456");

        assert!(matches!(
            result,
            Err(ReviewError::StaleCodeHash { ref expected, ref actual })
                if expected == "abc123" && actual == "def456"
        ));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    #[test]
    fn test_record_round_first_round_sets_code_hash() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.code_hash(), Some("abc123"));
    }

    #[test]
    fn test_record_final_stale_code_hash_rejects() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        let result = state.record_round(RoundType::Final, "group-a", zero(), &expected, "new-hash");
        assert!(matches!(result, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    // --- check_commit_ready ---

    #[test]
    fn test_check_commit_ready_approved_with_matching_hash_succeeds() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        assert!(state.check_commit_ready("abc123").is_ok());
    }

    #[test]
    fn test_check_commit_ready_not_approved_fails() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        let result = state.check_commit_ready("abc123");
        assert!(matches!(result, Err(ReviewError::NotApproved(ReviewStatus::FastPassed))));
    }

    #[test]
    fn test_check_commit_ready_stale_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        let result = state.check_commit_ready("new-hash");
        assert!(matches!(result, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    // --- demotion on findings_remain ---

    #[test]
    fn test_fast_findings_after_fast_passed_demotes_to_not_started() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Record a new fast round with findings — should demote
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_fast_findings_after_approved_demotes_to_not_started() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Fast round with findings on approved track — demotes to not_started
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_final_findings_after_approved_demotes_to_fast_passed() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-b", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Final round with findings — demotes to fast_passed
        state.record_round(RoundType::Final, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    #[test]
    fn test_fast_rerun_clears_stale_final_round() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];

        // Full approval cycle
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-b", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Re-run fast for group-a with findings — should clear group-a's final_round
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
        assert!(state.groups().get("group-a").unwrap().final_round().is_none());
        // group-b's final_round should still be intact
        assert!(state.groups().get("group-b").unwrap().final_round().is_some());

        // Now re-pass fast for group-a and try to go to final
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        // group-a still has no final_round, so final aggregation should NOT promote
        // even though group-b has an old final zero_findings
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // After group-a's fast rerun cleared group-a's final, re-record group-a's final.
        // group-b's final is still valid (its fast was not re-run, same code_hash).
        // Both groups now have final zero_findings → promotes to Approved.
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);
    }

    // --- invalidate ---

    #[test]
    fn test_invalidate_sets_status_to_invalidated() {
        let mut state = ReviewState::new();
        state.invalidate();
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    #[test]
    fn test_record_round_after_stale_hash_invalidation_accepts_new_hash() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];

        // First round sets code_hash
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash1").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Stale hash → invalidation + code_hash cleared
        let err = state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash2");
        assert!(matches!(err, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
        assert!(state.code_hash().is_none());

        // Re-run with new hash should succeed (fresh start)
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash2").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("hash2"));
    }

    // --- record_round_with_pending ---

    #[test]
    fn test_record_round_with_pending_sets_code_hash_to_pending() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state
            .record_round_with_pending(RoundType::Fast, "group-a", zero(), &expected, "pre-hash")
            .unwrap();
        assert_eq!(state.code_hash(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_first_round_skips_freshness_check() {
        let mut state = ReviewState::new();
        // code_hash is None initially — freshness check must be skipped
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round_with_pending(
            RoundType::Fast,
            "group-a",
            zero(),
            &expected,
            "any-hash",
        );
        assert!(result.is_ok());
        assert_eq!(state.code_hash(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_subsequent_round_checks_freshness() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        // Set up a code_hash via record_round first
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash1").unwrap();
        assert_eq!(state.code_hash(), Some("hash1"));

        // Passing correct pre_update_hash should succeed
        let result =
            state.record_round_with_pending(RoundType::Fast, "group-a", zero(), &expected, "hash1");
        assert!(result.is_ok());
        assert_eq!(state.code_hash(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_stale_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        // Set up a code_hash
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash1").unwrap();

        // Wrong pre_update_hash → should fail
        let result = state.record_round_with_pending(
            RoundType::Fast,
            "group-a",
            zero(),
            &expected,
            "wrong-hash",
        );
        assert!(matches!(
            result,
            Err(ReviewError::StaleCodeHash { ref expected, ref actual })
                if expected == "hash1" && actual == "wrong-hash"
        ));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    #[test]
    fn test_record_round_with_pending_final_without_fast_passed_fails() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round_with_pending(
            RoundType::Final,
            "group-a",
            zero(),
            &expected,
            "any-hash",
        );
        assert!(matches!(
            result,
            Err(ReviewError::FinalRequiresFastPassed(ReviewStatus::NotStarted))
        ));
    }

    #[test]
    fn test_record_round_with_pending_records_group_result() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state
            .record_round_with_pending(RoundType::Fast, "group-a", zero(), &expected, "pre-hash")
            .unwrap();
        assert!(state.groups().get("group-a").is_some());
        assert!(state.groups().get("group-a").unwrap().fast().is_some());
    }

    // --- set_code_hash ---

    #[test]
    fn test_set_code_hash_sets_value() {
        let mut state = ReviewState::new();
        state.set_code_hash("computed-hash".to_owned());
        assert_eq!(state.code_hash(), Some("computed-hash"));
    }

    #[test]
    fn test_set_code_hash_overwrites_existing_value() {
        let mut state = ReviewState::new();
        state.set_code_hash("old-hash".to_owned());
        state.set_code_hash("new-hash".to_owned());
        assert_eq!(state.code_hash(), Some("new-hash"));
    }

    #[test]
    fn test_two_phase_hash_protocol_full_flow() {
        // Simulate the full two-phase protocol:
        // 1. record_round_with_pending with pre_update_hash
        // 2. set_code_hash with the computed post-update hash
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];

        // Phase 1: record with PENDING
        state
            .record_round_with_pending(
                RoundType::Fast,
                "group-a",
                zero(),
                &expected,
                "pre-update-hash",
            )
            .unwrap();
        assert_eq!(state.code_hash(), Some("PENDING"));

        // Phase 2: write back real hash
        state.set_code_hash("post-update-hash".to_owned());
        assert_eq!(state.code_hash(), Some("post-update-hash"));
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    // --- with_fields ---

    #[test]
    fn test_with_fields_preserves_all_fields() {
        let mut groups = HashMap::new();
        groups.insert(
            "g1".to_owned(),
            ReviewGroupState::with_fast(ReviewRoundResult::new(1, "zero_findings", "ts")),
        );

        let state = ReviewState::with_fields(
            ReviewStatus::FastPassed,
            Some("hash123".to_owned()),
            groups.clone(),
        );

        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("hash123"));
        assert_eq!(state.groups(), &groups);
    }
}
