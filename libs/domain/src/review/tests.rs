//! Tests for the review module.

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::module_inception
)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use crate::review::ReviewError;
    use crate::{
        ReviewGroupName, Timestamp,
        review::{
            CodeHash, EscalationPhase, ReviewConcern, ReviewEscalationBlock,
            ReviewEscalationDecision, ReviewEscalationResolution, ReviewEscalationState,
            ReviewGroupState, ReviewRoundResult, ReviewState, ReviewStatus, RoundType, Verdict,
        },
    };

    fn ts(s: &str) -> Timestamp {
        Timestamp::new(s).unwrap()
    }

    fn zero() -> ReviewRoundResult {
        ReviewRoundResult::new(1, Verdict::ZeroFindings, ts("2026-03-18T00:00:00Z"))
    }

    fn findings() -> ReviewRoundResult {
        let concern = ReviewConcern::try_new("test-concern").unwrap();
        ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-18T00:00:00Z"),
            vec![concern],
        )
    }

    fn gn(s: &str) -> ReviewGroupName {
        ReviewGroupName::try_new(s).unwrap()
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
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("abc123"));
        assert!(state.groups().get(&gn("group-a")).unwrap().fast().is_some());
    }

    #[test]
    fn test_record_fast_partial_groups_does_not_promote() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();

        // Only one of two expected groups recorded — no promotion
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_record_fast_all_groups_zero_findings_promotes() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    #[test]
    fn test_record_fast_findings_remain_blocks_promotion() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];
        state
            .record_round(RoundType::Fast, &gn("group-a"), findings(), &expected, "abc123")
            .unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), zero(), &expected, "abc123").unwrap();

        // group-a has findings_remain — no promotion
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_record_fast_does_not_overwrite_other_groups() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), zero(), &expected, "abc123").unwrap();

        // Both groups should exist
        assert!(state.groups().get(&gn("group-a")).unwrap().fast().is_some());
        assert!(state.groups().get(&gn("group-b")).unwrap().fast().is_some());
    }

    // --- record_round: final round recording ---

    #[test]
    fn test_record_final_after_fast_passed_succeeds() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);
    }

    #[test]
    fn test_record_final_without_fast_passed_fails() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        let result =
            state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123");

        assert!(matches!(
            result,
            Err(ReviewError::FinalRequiresFastPassed(ReviewStatus::NotStarted))
        ));
    }

    #[test]
    fn test_record_final_partial_groups_does_not_promote() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];
        // Fast pass both
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Final only for group-a
        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed); // Not promoted yet
    }

    #[test]
    fn test_record_final_findings_remain_blocks_promotion() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), zero(), &expected, "abc123").unwrap();

        state
            .record_round(RoundType::Final, &gn("group-a"), findings(), &expected, "abc123")
            .unwrap();
        state.record_round(RoundType::Final, &gn("group-b"), zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed); // findings in A blocks
    }

    // --- record_round: code hash validation ---

    #[test]
    fn test_record_round_stale_code_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();

        let result =
            state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "def456");

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
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();

        assert_eq!(state.code_hash(), Some("abc123"));
    }

    #[test]
    fn test_record_final_stale_code_hash_rejects() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        let result =
            state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "new-hash");
        assert!(matches!(result, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    // --- check_commit_ready ---

    #[test]
    fn test_check_commit_ready_approved_with_matching_hash_succeeds() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        assert!(state.check_commit_ready("abc123").is_ok());
    }

    #[test]
    fn test_check_commit_ready_not_approved_fails() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();

        let result = state.check_commit_ready("abc123");
        assert!(matches!(result, Err(ReviewError::NotApproved(ReviewStatus::FastPassed))));
    }

    #[test]
    fn test_check_commit_ready_stale_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        let result = state.check_commit_ready("new-hash");
        assert!(matches!(result, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    // --- demotion on findings_remain ---

    #[test]
    fn test_fast_findings_after_fast_passed_demotes_to_not_started() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Record a new fast round with findings — should demote
        state
            .record_round(RoundType::Fast, &gn("group-a"), findings(), &expected, "abc123")
            .unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_fast_findings_after_approved_demotes_to_not_started() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Fast round with findings on approved track — demotes to not_started
        state
            .record_round(RoundType::Fast, &gn("group-a"), findings(), &expected, "abc123")
            .unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_final_findings_after_approved_demotes_to_fast_passed() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, &gn("group-b"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Final round with findings — demotes to fast_passed
        state
            .record_round(RoundType::Final, &gn("group-a"), findings(), &expected, "abc123")
            .unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    #[test]
    fn test_fast_rerun_clears_stale_final_round() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];

        // Full approval cycle
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, &gn("group-b"), zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Re-run fast for group-a with findings — should clear group-a's final_round
        state
            .record_round(RoundType::Fast, &gn("group-a"), findings(), &expected, "abc123")
            .unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
        assert!(state.groups().get(&gn("group-a")).unwrap().final_round().is_none());
        // group-b's final_round should still be intact
        assert!(state.groups().get(&gn("group-b")).unwrap().final_round().is_some());

        // Now re-pass fast for group-a and try to go to final
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123").unwrap();
        // group-a still has no final_round, so final aggregation should NOT promote
        // even though group-b has an old final zero_findings
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // After group-a's fast rerun cleared group-a's final, re-record group-a's final.
        // group-b's final is still valid (its fast was not re-run, same code_hash).
        // Both groups now have final zero_findings → promotes to Approved.
        state.record_round(RoundType::Final, &gn("group-a"), zero(), &expected, "abc123").unwrap();
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
        let expected = vec![gn("group-a")];

        // First round sets code_hash
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "hash1").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Stale hash → invalidation + code_hash cleared
        let err = state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "hash2");
        assert!(matches!(err, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
        assert!(state.code_hash().is_none());

        // Re-run with new hash should succeed (fresh start)
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "hash2").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("hash2"));
    }

    // --- record_round_with_pending ---

    #[test]
    fn test_record_round_with_pending_sets_code_hash_to_pending() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        state
            .record_round_with_pending(
                RoundType::Fast,
                &gn("group-a"),
                zero(),
                &expected,
                "pre-hash",
            )
            .unwrap();
        // code_hash() returns None for Pending; use code_hash_for_serialization() to get "PENDING"
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_first_round_skips_freshness_check() {
        let mut state = ReviewState::new();
        // code_hash is None initially — freshness check must be skipped
        let expected = vec![gn("group-a")];
        let result = state.record_round_with_pending(
            RoundType::Fast,
            &gn("group-a"),
            zero(),
            &expected,
            "any-hash",
        );
        assert!(result.is_ok());
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_subsequent_round_checks_freshness() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        // Set up a code_hash via record_round first
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "hash1").unwrap();
        assert_eq!(state.code_hash(), Some("hash1"));

        // Passing correct pre_update_hash should succeed
        let result = state.record_round_with_pending(
            RoundType::Fast,
            &gn("group-a"),
            zero(),
            &expected,
            "hash1",
        );
        assert!(result.is_ok());
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_stale_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        // Set up a code_hash
        state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "hash1").unwrap();

        // Wrong pre_update_hash → should fail
        let result = state.record_round_with_pending(
            RoundType::Fast,
            &gn("group-a"),
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
        let expected = vec![gn("group-a")];
        let result = state.record_round_with_pending(
            RoundType::Final,
            &gn("group-a"),
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
        let expected = vec![gn("group-a")];
        state
            .record_round_with_pending(
                RoundType::Fast,
                &gn("group-a"),
                zero(),
                &expected,
                "pre-hash",
            )
            .unwrap();
        assert!(state.groups().get(&gn("group-a")).is_some());
        assert!(state.groups().get(&gn("group-a")).unwrap().fast().is_some());
    }

    // --- set_code_hash ---

    #[test]
    fn test_set_code_hash_sets_value() {
        let mut state = ReviewState::new();
        state.set_code_hash("computed-hash".to_owned()).unwrap();
        assert_eq!(state.code_hash(), Some("computed-hash"));
    }

    #[test]
    fn test_set_code_hash_overwrites_existing_value() {
        let mut state = ReviewState::new();
        state.set_code_hash("old-hash".to_owned()).unwrap();
        state.set_code_hash("new-hash".to_owned()).unwrap();
        assert_eq!(state.code_hash(), Some("new-hash"));
    }

    #[test]
    fn test_two_phase_hash_protocol_full_flow() {
        // Simulate the full two-phase protocol:
        // 1. record_round_with_pending with pre_update_hash
        // 2. set_code_hash with the computed post-update hash
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];

        // Phase 1: record with PENDING
        state
            .record_round_with_pending(
                RoundType::Fast,
                &gn("group-a"),
                zero(),
                &expected,
                "pre-update-hash",
            )
            .unwrap();
        // code_hash() returns None for Pending; serialization gives "PENDING"
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));

        // Phase 2: write back real hash
        state.set_code_hash("post-update-hash".to_owned()).unwrap();
        assert_eq!(state.code_hash(), Some("post-update-hash"));
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    // --- ReviewConcern tests ---

    #[test]
    fn test_review_concern_new_with_valid_slug_succeeds() {
        let c = ReviewConcern::try_new("domain.review").unwrap();
        assert_eq!(c.as_ref(), "domain.review");
    }

    #[test]
    fn test_review_concern_new_with_empty_string_fails() {
        let result = ReviewConcern::try_new("");
        assert!(matches!(result, Err(ReviewError::InvalidConcern(_))));
    }

    #[test]
    fn test_review_concern_new_with_whitespace_only_fails() {
        let result = ReviewConcern::try_new("   ");
        assert!(matches!(result, Err(ReviewError::InvalidConcern(_))));
    }

    #[test]
    fn test_review_concern_normalizes_to_lowercase() {
        let c = ReviewConcern::try_new("Domain.Review").unwrap();
        assert_eq!(c.as_ref(), "domain.review");
    }

    #[test]
    fn test_review_concern_trims_whitespace() {
        let c = ReviewConcern::try_new("  shell-parsing  ").unwrap();
        assert_eq!(c.as_ref(), "shell-parsing");
    }

    #[test]
    fn test_review_concern_ord_is_lexicographic() {
        let a = ReviewConcern::try_new("aaa").unwrap();
        let b = ReviewConcern::try_new("bbb").unwrap();
        assert!(a < b);
    }

    // --- ReviewEscalationState tests ---

    #[test]
    fn test_escalation_state_new_is_clear() {
        let state = ReviewEscalationState::new();
        assert_eq!(state.threshold(), 3);
        assert_eq!(state.phase(), &EscalationPhase::Clear);
        assert!(state.recent_cycles().is_empty());
        assert!(state.concern_streaks().is_empty());
        assert!(state.last_resolution().is_none());
    }

    #[test]
    fn test_escalation_state_is_blocked_returns_false_when_clear() {
        let state = ReviewEscalationState::new();
        assert!(!state.is_blocked());
    }

    #[test]
    fn test_escalation_state_is_blocked_returns_true_when_blocked() {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let state = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        assert!(state.is_blocked());
    }

    // --- EscalationActive gate tests ---

    fn blocked_review_state() -> ReviewState {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        ReviewState::with_fields(
            ReviewStatus::NotStarted,
            CodeHash::NotRecorded,
            HashMap::new(),
            escalation,
        )
    }

    #[test]
    fn test_record_round_rejects_when_escalation_blocked() {
        let mut state = blocked_review_state();
        let expected = vec![gn("group-a")];
        let result =
            state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "abc123");
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { ref concerns }) if !concerns.is_empty()),
            "expected EscalationActive, got {result:?}"
        );
    }

    #[test]
    fn test_record_round_with_pending_rejects_when_escalation_blocked() {
        let mut state = blocked_review_state();
        let expected = vec![gn("group-a")];
        let result = state.record_round_with_pending(
            RoundType::Fast,
            &gn("group-a"),
            zero(),
            &expected,
            "abc123",
        );
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { ref concerns }) if !concerns.is_empty()),
            "expected EscalationActive, got {result:?}"
        );
    }

    #[test]
    fn test_check_commit_ready_rejects_when_escalation_blocked() {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        // Use Approved status so the only block is escalation
        let mut state = ReviewState::with_fields(
            ReviewStatus::Approved,
            CodeHash::Computed("abc123".to_owned()),
            HashMap::new(),
            escalation,
        );
        let result = state.check_commit_ready("abc123");
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { ref concerns }) if !concerns.is_empty()),
            "expected EscalationActive, got {result:?}"
        );
    }

    #[test]
    fn test_escalation_check_happens_before_hash_check() {
        // Set up a state with BOTH stale hash AND blocked escalation.
        // The method must return EscalationActive (not StaleCodeHash).
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        let mut state = ReviewState::with_fields(
            ReviewStatus::NotStarted,
            CodeHash::Computed("old-hash".to_owned()),
            HashMap::new(),
            escalation,
        );
        let expected = vec![gn("group-a")];
        let result =
            state.record_round(RoundType::Fast, &gn("group-a"), zero(), &expected, "new-hash");
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { .. })),
            "expected EscalationActive before StaleCodeHash, got {result:?}"
        );
    }

    // --- ReviewRoundResult concerns tests ---

    #[test]
    fn test_review_round_result_new_has_empty_concerns() {
        let result = ReviewRoundResult::new(1, Verdict::ZeroFindings, ts("2026-03-19T00:00:00Z"));
        assert!(result.concerns().is_empty());
    }

    #[test]
    fn test_review_round_result_new_with_concerns() {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let result = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T00:00:00Z"),
            vec![concern.clone()],
        );
        assert_eq!(result.concerns(), &[concern]);
    }

    // --- with_fields ---

    #[test]
    fn test_with_fields_preserves_all_fields() {
        let mut groups = HashMap::new();
        groups.insert(
            gn("g1"),
            ReviewGroupState::with_fast(ReviewRoundResult::new(
                1,
                Verdict::ZeroFindings,
                ts("2026-03-19T00:00:00Z"),
            )),
        );

        let state = ReviewState::with_fields(
            ReviewStatus::FastPassed,
            CodeHash::Computed("hash123".to_owned()),
            groups.clone(),
            ReviewEscalationState::default(),
        );

        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("hash123"));
        assert_eq!(state.groups(), &groups);
    }

    // --- resolve_escalation tests ---

    fn blocked_state() -> ReviewState {
        let block = ReviewEscalationBlock::new(
            vec![ReviewConcern::try_new("shell-parsing").unwrap()],
            ts("2026-03-19T00:00:00Z"),
        );
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        ReviewState::with_fields(
            ReviewStatus::NotStarted,
            CodeHash::NotRecorded,
            HashMap::new(),
            escalation,
        )
    }

    fn valid_resolution() -> ReviewEscalationResolution {
        ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("shell-parsing").unwrap()],
            "search.md".to_owned(),
            "reinvention.md".to_owned(),
            ReviewEscalationDecision::ContinueSelfImplementation,
            "Justified: no suitable crate".to_owned(),
            ts("2026-03-19T01:00:00Z"),
        )
        .unwrap()
    }

    #[test]
    fn test_resolve_escalation_succeeds_with_valid_evidence() {
        let mut state = blocked_state();
        assert!(state.resolve_escalation(valid_resolution()).is_ok());
        assert_eq!(state.status(), ReviewStatus::Invalidated);
        assert!(state.code_hash().is_none());
        assert!(!state.escalation().is_blocked());
        assert!(state.escalation().last_resolution().is_some());
    }

    #[test]
    fn test_resolve_escalation_rejects_when_not_blocked() {
        let mut state = ReviewState::new();
        let result = state.resolve_escalation(valid_resolution());
        assert!(matches!(result, Err(ReviewError::EscalationNotActive)));
    }

    #[test]
    fn test_resolution_constructor_rejects_empty_workspace_search_ref() {
        let result = ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("shell-parsing").unwrap()],
            "",
            "reinvention.md",
            ReviewEscalationDecision::ContinueSelfImplementation,
            "summary",
            ts("2026-03-19T01:00:00Z"),
        );
        assert!(matches!(
            result,
            Err(ReviewError::ResolutionEvidenceMissing("workspace_search_ref"))
        ));
    }

    #[test]
    fn test_resolution_constructor_rejects_empty_reinvention_check_ref() {
        let result = ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("shell-parsing").unwrap()],
            "search.md",
            "  ",
            ReviewEscalationDecision::ContinueSelfImplementation,
            "summary",
            ts("2026-03-19T01:00:00Z"),
        );
        assert!(matches!(
            result,
            Err(ReviewError::ResolutionEvidenceMissing("reinvention_check_ref"))
        ));
    }

    #[test]
    fn test_resolution_constructor_rejects_empty_summary() {
        let result = ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("shell-parsing").unwrap()],
            "search.md",
            "reinvention.md",
            ReviewEscalationDecision::ContinueSelfImplementation,
            "",
            ts("2026-03-19T01:00:00Z"),
        );
        assert!(matches!(result, Err(ReviewError::ResolutionEvidenceMissing("summary"))));
    }

    #[test]
    fn test_resolve_escalation_rejects_mismatched_concerns() {
        let mut state = blocked_state();
        let res = ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("different-concern").unwrap()],
            "search.md",
            "reinvention.md",
            ReviewEscalationDecision::ContinueSelfImplementation,
            "summary",
            ts("2026-03-19T01:00:00Z"),
        )
        .unwrap();
        let result = state.resolve_escalation(res);
        assert!(matches!(result, Err(ReviewError::ResolutionConcernMismatch { .. })));
    }

    // --- Finding 1: expected_groups deduplication ---

    #[test]
    fn test_record_round_deduplicates_expected_groups() {
        // Passing duplicate expected_groups must not cause false cycle detection
        // (one result satisfying multiple slots).
        let mut state = ReviewState::new();
        // "group-a" appears twice in expected_groups — must be deduplicated to one entry.
        let expected_with_dups = vec![gn("group-a"), gn("group-a"), gn("group-b")];

        // Record only group-a — with duplicates this might incorrectly satisfy
        // both "group-a" slots and cause a false promotion.
        state
            .record_round(RoundType::Fast, &gn("group-a"), zero(), &expected_with_dups, "abc123")
            .unwrap();

        // After dedup, expected_groups is ["group-a", "group-b"].
        // Only group-a has recorded, so promotion must NOT happen.
        assert_eq!(
            state.status(),
            ReviewStatus::NotStarted,
            "duplicate expected_groups must not cause false promotion when only one unique group recorded"
        );

        // Now record group-b as well — both unique groups have zero_findings, so promote.
        state
            .record_round(RoundType::Fast, &gn("group-b"), zero(), &expected_with_dups, "abc123")
            .unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    // --- Finding 1: verdict/concerns consistency ---

    #[test]
    fn test_record_round_rejects_zero_findings_with_concerns() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        let concern = ReviewConcern::try_new("some-concern").unwrap();
        let result_with_concern = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::ZeroFindings,
            ts("2026-03-19T00:00:00Z"),
            vec![concern],
        );
        let result = state.record_round(
            RoundType::Fast,
            &gn("group-a"),
            result_with_concern,
            &expected,
            "abc123",
        );
        assert!(
            matches!(result, Err(ReviewError::InvalidConcern(_))),
            "expected InvalidConcern for zero_findings with non-empty concerns, got {result:?}"
        );
    }

    #[test]
    fn test_record_round_rejects_findings_remain_without_concerns() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];
        let result_no_concerns =
            ReviewRoundResult::new(1, Verdict::FindingsRemain, ts("2026-03-19T00:00:00Z"));
        let result = state.record_round(
            RoundType::Fast,
            &gn("group-a"),
            result_no_concerns,
            &expected,
            "abc123",
        );
        assert!(
            matches!(result, Err(ReviewError::InvalidConcern(_))),
            "expected InvalidConcern for findings_remain with empty concerns, got {result:?}"
        );
    }

    // --- Finding 2: resolve_escalation order-insensitive concern comparison ---

    #[test]
    fn test_resolve_escalation_accepts_reordered_concerns() {
        // Block with concerns [a, b]
        let block = ReviewEscalationBlock::new(
            vec![ReviewConcern::try_new("aaa").unwrap(), ReviewConcern::try_new("bbb").unwrap()],
            ts("2026-03-19T00:00:00Z"),
        );
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        let mut state = ReviewState::with_fields(
            ReviewStatus::NotStarted,
            CodeHash::NotRecorded,
            HashMap::new(),
            escalation,
        );

        // Resolution with concerns in reverse order [b, a]
        let resolution = ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("bbb").unwrap(), ReviewConcern::try_new("aaa").unwrap()],
            "search.md",
            "reinvention.md",
            ReviewEscalationDecision::ContinueSelfImplementation,
            "justified",
            ts("2026-03-19T01:00:00Z"),
        )
        .unwrap();
        let result = state.resolve_escalation(resolution);
        assert!(result.is_ok(), "expected Ok for reordered concerns, got {result:?}");
    }

    // --- Finding 3: escalation state updates after record_round ---

    fn round_with_concern(round: u32, concern: &str, ts_str: &str) -> ReviewRoundResult {
        let c = ReviewConcern::try_new(concern).unwrap();
        ReviewRoundResult::new_with_concerns(round, Verdict::FindingsRemain, ts(ts_str), vec![c])
    }

    fn zero_round(round: u32, ts_str: &str) -> ReviewRoundResult {
        ReviewRoundResult::new(round, Verdict::ZeroFindings, ts(ts_str))
    }

    #[test]
    fn test_escalation_triggers_after_3_consecutive_same_concern_cycles() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];

        // Round 1: fast findings with "bad-pattern"
        let r1 = round_with_concern(1, "bad-pattern", "2026-03-19T01:00:00Z");
        state.record_round(RoundType::Fast, &gn("group-a"), r1, &expected, "hash1").unwrap();

        // Round 2: fast findings with "bad-pattern" again — streak = 2
        let r2 = round_with_concern(2, "bad-pattern", "2026-03-19T02:00:00Z");
        state.record_round(RoundType::Fast, &gn("group-a"), r2, &expected, "hash1").unwrap();

        // Not yet blocked
        assert!(!state.escalation().is_blocked(), "should not be blocked after 2 rounds");

        // Round 3: fast findings with "bad-pattern" — streak = 3 → Blocked
        let r3 = round_with_concern(3, "bad-pattern", "2026-03-19T03:00:00Z");
        state.record_round(RoundType::Fast, &gn("group-a"), r3, &expected, "hash1").unwrap();

        assert!(state.escalation().is_blocked(), "should be blocked after 3 consecutive rounds");
        if let EscalationPhase::Blocked(ref block) = *state.escalation().phase() {
            assert_eq!(block.concerns().len(), 1);
            assert_eq!(block.concerns()[0].as_ref(), "bad-pattern");
        } else {
            panic!("expected Blocked phase");
        }
    }

    #[test]
    fn test_escalation_does_not_trigger_with_interrupted_streak() {
        // A → B → A → A: streak for A is 2 (reset when B appeared in round 2)
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];

        // Round 1: concern A
        let r1 = round_with_concern(1, "concern-a", "2026-03-19T01:00:00Z");
        state.record_round(RoundType::Fast, &gn("group-a"), r1, &expected, "hash1").unwrap();

        // Round 2: concern B (different) — resets A's streak
        let r2 = round_with_concern(2, "concern-b", "2026-03-19T02:00:00Z");
        state.record_round(RoundType::Fast, &gn("group-a"), r2, &expected, "hash1").unwrap();

        // Round 3: concern A again — streak for A is 1 (reset happened in round 2)
        let r3 = round_with_concern(3, "concern-a", "2026-03-19T03:00:00Z");
        state.record_round(RoundType::Fast, &gn("group-a"), r3, &expected, "hash1").unwrap();

        // Round 4: concern A — streak for A is 2 (not yet 3)
        let r4 = round_with_concern(4, "concern-a", "2026-03-19T04:00:00Z");
        state.record_round(RoundType::Fast, &gn("group-a"), r4, &expected, "hash1").unwrap();

        assert!(
            !state.escalation().is_blocked(),
            "should not be blocked: A streak is only 2 (was reset by B in round 2)"
        );
    }

    #[test]
    fn test_escalation_cycle_requires_all_groups() {
        let mut state = ReviewState::new();
        // Two expected groups — cycle only closes when both record
        let expected = vec![gn("group-a"), gn("group-b")];

        // Only group-a records 3 rounds (group-b never records)
        for i in 1u32..=3 {
            let ts_str = format!("2026-03-19T0{i}:00:00Z");
            let r = round_with_concern(i, "bad-pattern", &ts_str);
            state.record_round(RoundType::Fast, &gn("group-a"), r, &expected, "hash1").unwrap();
        }

        // Cycle never closes because group-b hasn't recorded → no escalation
        assert!(
            !state.escalation().is_blocked(),
            "partial group recording should not close a cycle or trigger escalation"
        );
    }

    #[test]
    fn test_recent_cycles_fifo_trims_at_10() {
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a")];

        // Record 12 fast rounds with zero_findings (each closes a cycle)
        for i in 1u32..=12 {
            let ts_str = format!("2026-03-19T{:02}:00:00Z", i);
            let r = zero_round(i, &ts_str);
            state.record_round(RoundType::Fast, &gn("group-a"), r, &expected, "hash1").unwrap();
        }

        assert_eq!(
            state.escalation().recent_cycles().len(),
            10,
            "recent_cycles should be trimmed to 10 (FIFO)"
        );
        // The oldest (round 1, 2) should have been evicted; round 12 should be present
        let last = state.escalation().recent_cycles().last().unwrap();
        assert_eq!(last.round(), 12);
    }

    // --- Finding 1: duplicate cycle detection ---

    #[test]
    fn test_escalation_rerecording_same_round_does_not_double_count() {
        // Two expected groups. Group A records round 1, then group B records round 1
        // → cycle closes. If group A then re-records round 1 (e.g., overwriting),
        // the cycle must NOT be counted again.
        let mut state = ReviewState::new();
        let expected = vec![gn("group-a"), gn("group-b")];

        // Both groups record findings_remain round 1 — cycle closes once
        let c = ReviewConcern::try_new("bad-pattern").unwrap();
        let r1a = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T01:00:00Z"),
            vec![c.clone()],
        );
        let r1b = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T02:00:00Z"),
            vec![c.clone()],
        );
        state.record_round(RoundType::Fast, &gn("group-a"), r1a, &expected, "hash1").unwrap();
        state.record_round(RoundType::Fast, &gn("group-b"), r1b, &expected, "hash1").unwrap();

        // After both groups record round 1, exactly 1 cycle should be counted
        assert_eq!(
            state.escalation().recent_cycles().len(),
            1,
            "one cycle after both groups record"
        );
        let streak_after_first = state
            .escalation()
            .concern_streaks()
            .get(&c)
            .map(|s| s.consecutive_rounds())
            .unwrap_or(0);
        assert_eq!(streak_after_first, 1, "streak should be 1 after first cycle");

        // Group A re-records the same round 1 (overwrite scenario)
        let r1a_again = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T03:00:00Z"),
            vec![c.clone()],
        );
        state.record_round(RoundType::Fast, &gn("group-a"), r1a_again, &expected, "hash1").unwrap();

        // The cycle for (Fast, round=1) was already counted — must NOT be double-counted
        assert_eq!(
            state.escalation().recent_cycles().len(),
            1,
            "re-recording same round must not add a second cycle"
        );
        let streak_after_rerecord = state
            .escalation()
            .concern_streaks()
            .get(&c)
            .map(|s| s.consecutive_rounds())
            .unwrap_or(0);
        assert_eq!(
            streak_after_rerecord, 1,
            "streak must not increment on re-recording same round"
        );
    }

    // --- Verdict tests ---

    #[test]
    fn test_verdict_display_zero_findings() {
        assert_eq!(Verdict::ZeroFindings.to_string(), "zero_findings");
    }

    #[test]
    fn test_verdict_display_findings_remain() {
        assert_eq!(Verdict::FindingsRemain.to_string(), "findings_remain");
    }

    #[test]
    fn test_verdict_parse_valid() {
        assert_eq!(Verdict::parse("zero_findings").unwrap(), Verdict::ZeroFindings);
        assert_eq!(Verdict::parse("findings_remain").unwrap(), Verdict::FindingsRemain);
    }

    #[test]
    fn test_verdict_parse_invalid_returns_error() {
        let result = Verdict::parse("unknown_verdict");
        assert!(
            matches!(result, Err(ReviewError::InvalidConcern(_))),
            "expected InvalidConcern for unknown verdict, got {result:?}"
        );
    }

    #[test]
    fn test_verdict_is_zero_findings() {
        assert!(Verdict::ZeroFindings.is_zero_findings());
        assert!(!Verdict::FindingsRemain.is_zero_findings());
    }
}
