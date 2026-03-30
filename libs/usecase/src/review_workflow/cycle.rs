//! Usecase functions for review cycle lifecycle management.
//!
//! Wraps domain-layer cycle operations with partition-snapshot–aware inputs.

use domain::{
    CycleError, GroupRound, GroupRoundVerdict, ReviewGroupName, ReviewJson, ReviewStalenessReason,
    RoundType, Timestamp,
};

use crate::review_workflow::groups::ReviewPartitionSnapshot;

// ---------------------------------------------------------------------------
// Start cycle
// ---------------------------------------------------------------------------

/// Input for starting a new review cycle.
pub struct StartReviewCycleInput {
    pub cycle_id: String,
    pub started_at: Timestamp,
    pub base_ref: String,
    pub snapshot: ReviewPartitionSnapshot,
}

/// Starts a new review cycle in the given review.json using the frozen partition snapshot.
///
/// The snapshot's `base_policy_hash`, `policy_hash`, and partition groups are
/// stored in the cycle for later staleness comparison.
///
/// # Errors
/// Returns `CycleError` if the cycle cannot be created (e.g., missing mandatory `other`).
pub fn start_review_cycle(
    review: &mut ReviewJson,
    input: StartReviewCycleInput,
) -> Result<(), CycleError> {
    let groups = input.snapshot.partition().to_cycle_groups();
    review.start_cycle(
        input.cycle_id,
        input.started_at,
        input.base_ref,
        input.snapshot.base_policy_hash(),
        input.snapshot.policy_hash(),
        groups,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Record round
// ---------------------------------------------------------------------------

/// Outcome for recording a review round.
pub enum RecordRoundOutcome {
    /// Reviewer succeeded with a verdict.
    Success(GroupRoundVerdict),
    /// Reviewer failed (timeout, crash, etc.).
    Failure { error_message: Option<String> },
}

/// Input for recording a group round.
pub struct RecordCycleGroupRoundInput {
    pub group_name: ReviewGroupName,
    pub round_type: RoundType,
    pub timestamp: Timestamp,
    pub outcome: RecordRoundOutcome,
    pub group_hash: String,
}

/// Errors from recording a group round.
#[derive(Debug, thiserror::Error)]
pub enum RecordCycleGroupRoundError {
    #[error("no current review cycle exists")]
    NoCurrentCycle,
    #[error("group '{0}' not found in current cycle")]
    UnknownGroup(ReviewGroupName),
    #[error("cycle error: {0}")]
    Cycle(#[from] CycleError),
}

/// Appends a review round to the specified group in the current cycle.
///
/// The caller is responsible for persisting the modified `ReviewJson` afterwards.
/// Per-group hash should be computed externally from the frozen scope and current
/// file contents.
///
/// # Errors
/// Returns `RecordCycleGroupRoundError` if no cycle exists, group is unknown,
/// or the round construction fails.
pub fn record_cycle_group_round(
    review: &mut ReviewJson,
    input: RecordCycleGroupRoundInput,
) -> Result<(), RecordCycleGroupRoundError> {
    let cycle = review.current_cycle_mut().ok_or(RecordCycleGroupRoundError::NoCurrentCycle)?;
    let group = cycle
        .group_mut(&input.group_name)
        .ok_or_else(|| RecordCycleGroupRoundError::UnknownGroup(input.group_name.clone()))?;

    // Validate non-decreasing timestamp order before mutation
    if let Some(last_round) = group.latest_round_any() {
        if input.timestamp < *last_round.timestamp() {
            return Err(RecordCycleGroupRoundError::Cycle(CycleError::Internal(format!(
                "round timestamp {} is before last recorded timestamp {}",
                input.timestamp,
                last_round.timestamp()
            ))));
        }
    }

    let round = match input.outcome {
        RecordRoundOutcome::Success(verdict) => {
            GroupRound::success(input.round_type, input.timestamp, input.group_hash, verdict)?
        }
        RecordRoundOutcome::Failure { error_message } => {
            GroupRound::failure(input.round_type, input.timestamp, input.group_hash, error_message)?
        }
    };

    group.record_round(round);
    Ok(())
}

// ---------------------------------------------------------------------------
// Staleness check
// ---------------------------------------------------------------------------

/// Checks whether a group in the current cycle is stale against the current policy.
///
/// Per-group staleness check.
///
/// Distinguishes:
/// - Base policy change → `PolicyChanged`
/// - Override/partition change (effective hash differs but base is same) → `PartitionChanged`
/// - Group membership change → `PartitionChanged`
/// - Per-group content hash mismatch → `HashMismatch`
#[must_use]
pub fn check_review_cycle_staleness(
    review: &ReviewJson,
    group_name: &ReviewGroupName,
    current: &ReviewPartitionSnapshot,
    current_group_hash: Option<&str>,
) -> Option<ReviewStalenessReason> {
    let cycle = review.current_cycle()?;
    cycle.check_group_staleness(
        group_name,
        current.base_policy_hash(),
        current.policy_hash(),
        current_group_hash,
    )
}

/// Checks staleness across all groups by comparing the cycle's group key set
/// against the current partition's key set, then checking each group individually.
///
/// Returns the first staleness reason found (policy > partition > hash), or `None`.
#[must_use]
pub fn check_cycle_staleness_any(
    review: &ReviewJson,
    current: &ReviewPartitionSnapshot,
    current_group_hashes: &std::collections::BTreeMap<ReviewGroupName, String>,
) -> Option<ReviewStalenessReason> {
    let cycle = review.current_cycle()?;

    // Check policy hashes first (most critical)
    if cycle.base_policy_hash() != current.base_policy_hash() {
        return Some(ReviewStalenessReason::PolicyChanged);
    }
    if cycle.policy_hash() != current.policy_hash() {
        return Some(ReviewStalenessReason::PartitionChanged);
    }

    // Compare group key sets: cycle groups vs current partition groups
    let cycle_keys: std::collections::BTreeSet<&ReviewGroupName> = cycle.group_names().collect();
    let current_keys: std::collections::BTreeSet<&ReviewGroupName> =
        current.partition().groups().keys().collect();
    if cycle_keys != current_keys {
        return Some(ReviewStalenessReason::PartitionChanged);
    }

    // Check per-group content hashes
    for name in cycle_keys {
        let current_hash = current_group_hashes.get(name).map(String::as_str);
        if let Some(reason) = cycle.check_group_staleness(
            name,
            current.base_policy_hash(),
            current.policy_hash(),
            current_hash,
        ) {
            return Some(reason);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Check approved
// ---------------------------------------------------------------------------

/// Structured result for review-cycle approval checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckCycleApprovedResult {
    /// All groups have approved fast + final rounds with current hashes.
    Approved,
    /// Not yet approved, with the reason.
    NotApproved(CheckCycleApprovedReason),
}

/// Why the current review cycle is not approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckCycleApprovedReason {
    /// No review cycle exists (review.json absent or empty).
    NoCycle,
    /// Current cycle is stale (policy/partition/hash changed).
    Stale(ReviewStalenessReason),
    /// Cycle exists and is fresh, but not all groups meet approval requirements.
    ApprovalRequirementsNotMet,
}

/// Checks whether the current review cycle is fully approved.
///
/// 1. Fail-closed on NoCycle.
/// 2. Check staleness — stale cycle means old rounds don't count.
/// 3. Delegate to domain `all_groups_approved` for per-group fast+final check.
#[must_use]
pub fn check_cycle_approved(
    review: &ReviewJson,
    current: &ReviewPartitionSnapshot,
    current_group_hashes: &std::collections::BTreeMap<ReviewGroupName, String>,
) -> CheckCycleApprovedResult {
    let Some(_cycle) = review.current_cycle() else {
        return CheckCycleApprovedResult::NotApproved(CheckCycleApprovedReason::NoCycle);
    };

    if let Some(reason) = check_cycle_staleness_any(review, current, current_group_hashes) {
        return CheckCycleApprovedResult::NotApproved(CheckCycleApprovedReason::Stale(reason));
    }

    // Safety: we just confirmed current_cycle() is Some
    if review.current_cycle().is_some_and(|c| c.all_groups_approved(current_group_hashes)) {
        CheckCycleApprovedResult::Approved
    } else {
        CheckCycleApprovedResult::NotApproved(CheckCycleApprovedReason::ApprovalRequirementsNotMet)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeMap;

    use domain::{
        GroupRound, GroupRoundVerdict, ReviewGroupName, ReviewJson, ReviewStalenessReason,
        RoundType, Timestamp,
    };

    use super::*;
    use crate::review_workflow::groups::GroupPartition;

    fn ts(s: &str) -> Timestamp {
        Timestamp::new(s).unwrap()
    }

    fn grn(s: &str) -> ReviewGroupName {
        ReviewGroupName::try_new(s).unwrap()
    }

    fn make_partition(group_names: &[&str]) -> GroupPartition {
        let mut groups = BTreeMap::new();
        for name in group_names {
            groups.insert(grn(name), vec![]);
        }
        groups.entry(grn("other")).or_insert_with(Vec::new);
        GroupPartition::try_new(groups).unwrap()
    }

    fn make_snapshot(base_hash: &str, effective_hash: &str) -> ReviewPartitionSnapshot {
        ReviewPartitionSnapshot::new(base_hash, effective_hash, make_partition(&["domain"]))
    }

    // -----------------------------------------------------------------------
    // Start cycle tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_start_cycle_freezes_hashes_and_partition() {
        let mut review = ReviewJson::new();
        let snapshot = make_snapshot("sha256:base", "sha256:effective");

        start_review_cycle(
            &mut review,
            StartReviewCycleInput {
                cycle_id: "c1".into(),
                started_at: ts("2026-03-30T10:00:00Z"),
                base_ref: "main".into(),
                snapshot,
            },
        )
        .unwrap();

        let cycle = review.current_cycle().unwrap();
        assert_eq!(cycle.base_policy_hash(), "sha256:base");
        assert_eq!(cycle.policy_hash(), "sha256:effective");
        assert!(cycle.groups().contains_key(&grn("other")));
        assert!(cycle.groups().contains_key(&grn("domain")));
    }

    // -----------------------------------------------------------------------
    // Staleness tests
    // -----------------------------------------------------------------------

    fn review_with_cycle(base_hash: &str, effective_hash: &str) -> ReviewJson {
        let mut review = ReviewJson::new();
        let snapshot =
            ReviewPartitionSnapshot::new(base_hash, effective_hash, make_partition(&["domain"]));
        start_review_cycle(
            &mut review,
            StartReviewCycleInput {
                cycle_id: "c1".into(),
                started_at: ts("2026-03-30T10:00:00Z"),
                base_ref: "main".into(),
                snapshot,
            },
        )
        .unwrap();
        // Record a round so hash mismatch checks work
        let cycle = review.current_cycle_mut().unwrap();
        let domain = cycle.group_mut(&grn("domain")).unwrap();
        domain.record_round(
            GroupRound::success(
                RoundType::Fast,
                ts("2026-03-30T10:01:00Z"),
                "hash-current",
                GroupRoundVerdict::ZeroFindings,
            )
            .unwrap(),
        );
        review
    }

    #[test]
    fn test_staleness_none_when_unchanged() {
        let review = review_with_cycle("sha256:base", "sha256:eff");
        let current = make_snapshot("sha256:base", "sha256:eff");
        let result =
            check_review_cycle_staleness(&review, &grn("domain"), &current, Some("hash-current"));
        assert!(result.is_none());
    }

    #[test]
    fn test_staleness_policy_changed_when_base_differs() {
        let review = review_with_cycle("sha256:base-old", "sha256:eff");
        let current = make_snapshot("sha256:base-new", "sha256:eff");
        let result =
            check_review_cycle_staleness(&review, &grn("domain"), &current, Some("hash-current"));
        assert_eq!(result, Some(ReviewStalenessReason::PolicyChanged));
    }

    #[test]
    fn test_staleness_partition_changed_when_effective_differs() {
        let review = review_with_cycle("sha256:base", "sha256:eff-old");
        let current = make_snapshot("sha256:base", "sha256:eff-new");
        let result =
            check_review_cycle_staleness(&review, &grn("domain"), &current, Some("hash-current"));
        assert_eq!(result, Some(ReviewStalenessReason::PartitionChanged));
    }

    #[test]
    fn test_staleness_partition_changed_when_group_missing() {
        let review = review_with_cycle("sha256:base", "sha256:eff");
        let current = make_snapshot("sha256:base", "sha256:eff");
        // Query a group that doesn't exist in the cycle
        let result = check_review_cycle_staleness(
            &review,
            &grn("new-group"),
            &current,
            Some("hash-current"),
        );
        assert_eq!(result, Some(ReviewStalenessReason::PartitionChanged));
    }

    #[test]
    fn test_staleness_hash_mismatch() {
        let review = review_with_cycle("sha256:base", "sha256:eff");
        let current = make_snapshot("sha256:base", "sha256:eff");
        let result =
            check_review_cycle_staleness(&review, &grn("domain"), &current, Some("hash-changed"));
        assert_eq!(result, Some(ReviewStalenessReason::HashMismatch));
    }

    #[test]
    fn test_staleness_returns_none_for_empty_review() {
        let review = ReviewJson::new();
        let current = make_snapshot("sha256:base", "sha256:eff");
        let result =
            check_review_cycle_staleness(&review, &grn("domain"), &current, Some("hash-current"));
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // check_cycle_staleness_any tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_any_staleness_none_when_unchanged() {
        let review = review_with_cycle("sha256:base", "sha256:eff");
        let current = make_snapshot("sha256:base", "sha256:eff");
        let mut hashes = BTreeMap::new();
        hashes.insert(grn("domain"), "hash-current".into());
        hashes.insert(grn("other"), "".into());

        let result = check_cycle_staleness_any(&review, &current, &hashes);
        assert!(result.is_none());
    }

    #[test]
    fn test_any_staleness_detects_group_key_set_drift() {
        let review = review_with_cycle("sha256:base", "sha256:eff");
        // Current partition has an extra group "infra" not in cycle
        let mut groups = BTreeMap::new();
        groups.insert(grn("domain"), vec![]);
        groups.insert(grn("infra"), vec![]);
        groups.insert(grn("other"), vec![]);
        let partition = GroupPartition::try_new(groups).unwrap();
        let current = ReviewPartitionSnapshot::new("sha256:base", "sha256:eff", partition);
        let hashes = BTreeMap::new();

        let result = check_cycle_staleness_any(&review, &current, &hashes);
        assert_eq!(result, Some(ReviewStalenessReason::PartitionChanged));
    }

    #[test]
    fn test_any_staleness_detects_hash_mismatch() {
        let review = review_with_cycle("sha256:base", "sha256:eff");
        let current = make_snapshot("sha256:base", "sha256:eff");
        let mut hashes = BTreeMap::new();
        hashes.insert(grn("domain"), "hash-changed".into());
        hashes.insert(grn("other"), "".into());

        let result = check_cycle_staleness_any(&review, &current, &hashes);
        assert_eq!(result, Some(ReviewStalenessReason::HashMismatch));
    }

    #[test]
    fn test_any_staleness_returns_none_for_empty_review() {
        let review = ReviewJson::new();
        let current = make_snapshot("sha256:base", "sha256:eff");
        let result = check_cycle_staleness_any(&review, &current, &BTreeMap::new());
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // record_cycle_group_round tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_round_success_appends() {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("domain"),
                round_type: RoundType::Final,
                timestamp: ts("2026-03-30T10:05:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash-final".into(),
            },
        )
        .unwrap();

        let cycle = review.current_cycle().unwrap();
        let domain = cycle.group(&grn("domain")).unwrap();
        // Original fast round + new final round = 2
        assert_eq!(domain.rounds().len(), 2);
        assert!(domain.rounds().last().unwrap().is_successful_zero_findings());
    }

    #[test]
    fn test_record_round_failure_appends() {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("domain"),
                round_type: RoundType::Fast,
                timestamp: ts("2026-03-30T10:05:00Z"),
                outcome: RecordRoundOutcome::Failure { error_message: Some("timeout".into()) },
                group_hash: "hash-fail".into(),
            },
        )
        .unwrap();

        let cycle = review.current_cycle().unwrap();
        let domain = cycle.group(&grn("domain")).unwrap();
        assert_eq!(domain.rounds().len(), 2);
        assert!(!domain.rounds().last().unwrap().is_successful_zero_findings());
    }

    #[test]
    fn test_record_round_other_group_does_not_affect_zero_findings_group() {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        // domain has a zero_findings fast round from review_with_cycle
        // Record a round on "other" — domain should be untouched
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("other"),
                round_type: RoundType::Fast,
                timestamp: ts("2026-03-30T10:05:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash-other".into(),
            },
        )
        .unwrap();

        let cycle = review.current_cycle().unwrap();
        // domain still has exactly 1 round
        assert_eq!(cycle.group(&grn("domain")).unwrap().rounds().len(), 1);
        // other now has 1 round
        assert_eq!(cycle.group(&grn("other")).unwrap().rounds().len(), 1);
    }

    #[test]
    fn test_record_round_no_cycle_returns_error() {
        let mut review = ReviewJson::new();
        let result = record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("domain"),
                round_type: RoundType::Fast,
                timestamp: ts("2026-03-30T10:05:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash".into(),
            },
        );
        assert!(matches!(result, Err(RecordCycleGroupRoundError::NoCurrentCycle)));
    }

    #[test]
    fn test_record_round_unknown_group_returns_error() {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        let result = record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("nonexistent"),
                round_type: RoundType::Fast,
                timestamp: ts("2026-03-30T10:05:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash".into(),
            },
        );
        assert!(matches!(result, Err(RecordCycleGroupRoundError::UnknownGroup(_))));
    }

    #[test]
    fn test_record_round_rejects_out_of_order_timestamp() {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        // review_with_cycle records a round at 10:01:00Z
        let result = record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("domain"),
                round_type: RoundType::Fast,
                timestamp: ts("2026-03-30T09:00:00Z"), // before 10:01:00Z
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash".into(),
            },
        );
        assert!(matches!(result, Err(RecordCycleGroupRoundError::Cycle(_))));
        // ReviewJson was not mutated
        assert_eq!(
            review.current_cycle().unwrap().group(&grn("domain")).unwrap().rounds().len(),
            1
        );
    }

    // -----------------------------------------------------------------------
    // check_cycle_approved tests
    // -----------------------------------------------------------------------

    fn fully_approved_review() -> (ReviewJson, ReviewPartitionSnapshot) {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        // domain already has fast zero_findings from review_with_cycle
        // Add final for domain
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("domain"),
                round_type: RoundType::Final,
                timestamp: ts("2026-03-30T10:02:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash-current".into(),
            },
        )
        .unwrap();
        // Add fast + final for other
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("other"),
                round_type: RoundType::Fast,
                timestamp: ts("2026-03-30T10:03:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash-other".into(),
            },
        )
        .unwrap();
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("other"),
                round_type: RoundType::Final,
                timestamp: ts("2026-03-30T10:04:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash-other".into(),
            },
        )
        .unwrap();
        let snapshot = make_snapshot("sha256:b", "sha256:e");
        (review, snapshot)
    }

    #[test]
    fn test_check_approved_returns_approved() {
        let (review, snapshot) = fully_approved_review();
        let mut hashes = BTreeMap::new();
        hashes.insert(grn("domain"), "hash-current".into());
        hashes.insert(grn("other"), "hash-other".into());

        let result = check_cycle_approved(&review, &snapshot, &hashes);
        assert_eq!(result, CheckCycleApprovedResult::Approved);
    }

    #[test]
    fn test_check_approved_no_cycle() {
        let review = ReviewJson::new();
        let snapshot = make_snapshot("sha256:b", "sha256:e");
        let result = check_cycle_approved(&review, &snapshot, &BTreeMap::new());
        assert_eq!(
            result,
            CheckCycleApprovedResult::NotApproved(CheckCycleApprovedReason::NoCycle)
        );
    }

    #[test]
    fn test_check_approved_stale_hash() {
        let (review, _) = fully_approved_review();
        // Use different base hash to trigger PolicyChanged
        let snapshot = make_snapshot("sha256:different", "sha256:e");
        let result = check_cycle_approved(&review, &snapshot, &BTreeMap::new());
        assert_eq!(
            result,
            CheckCycleApprovedResult::NotApproved(CheckCycleApprovedReason::Stale(
                ReviewStalenessReason::PolicyChanged
            ))
        );
    }

    #[test]
    fn test_check_approved_missing_final() {
        // review_with_cycle only has fast for domain, no final
        let review = review_with_cycle("sha256:b", "sha256:e");
        let snapshot = make_snapshot("sha256:b", "sha256:e");
        let mut hashes = BTreeMap::new();
        hashes.insert(grn("domain"), "hash-current".into());
        hashes.insert(grn("other"), "".into());

        let result = check_cycle_approved(&review, &snapshot, &hashes);
        assert_eq!(
            result,
            CheckCycleApprovedResult::NotApproved(
                CheckCycleApprovedReason::ApprovalRequirementsNotMet
            )
        );
    }

    // -----------------------------------------------------------------------
    // T008: Full workflow regression tests (acceptance criteria coverage)
    // -----------------------------------------------------------------------

    /// AC: new review cycle creation freezes base_ref, policy_hash, group scopes, mandatory other
    #[test]
    fn test_regression_cycle_creation_freezes_all_fields() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("domain"), vec![]);
        groups.insert(grn("infra"), vec![]);
        groups.insert(grn("other"), vec![]);
        let partition = GroupPartition::try_new(groups).unwrap();
        let snapshot = ReviewPartitionSnapshot::new("sha256:base", "sha256:eff", partition);

        let mut review = ReviewJson::new();
        start_review_cycle(
            &mut review,
            StartReviewCycleInput {
                cycle_id: "regression-1".into(),
                started_at: ts("2026-03-30T12:00:00Z"),
                base_ref: "main".into(),
                snapshot,
            },
        )
        .unwrap();

        let cycle = review.current_cycle().unwrap();
        assert_eq!(cycle.base_ref(), "main");
        assert_eq!(cycle.base_policy_hash(), "sha256:base");
        assert_eq!(cycle.policy_hash(), "sha256:eff");
        assert_eq!(cycle.groups().len(), 3);
        assert!(cycle.groups().contains_key(&grn("other")));
    }

    /// AC: zero_findings group is not invalidated by other group's round
    #[test]
    fn test_regression_zero_findings_group_independent() {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        // domain has fast zero_findings

        // Record multiple rounds on "other" — domain should be untouched
        for i in 0..3 {
            record_cycle_group_round(
                &mut review,
                RecordCycleGroupRoundInput {
                    group_name: grn("other"),
                    round_type: RoundType::Fast,
                    timestamp: ts(&format!("2026-03-30T10:0{i}:00Z")),
                    outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                    group_hash: format!("hash-other-{i}"),
                },
            )
            .unwrap();
        }

        let cycle = review.current_cycle().unwrap();
        assert_eq!(cycle.group(&grn("domain")).unwrap().rounds().len(), 1);
        assert_eq!(cycle.group(&grn("other")).unwrap().rounds().len(), 3);
    }

    /// AC: stale cycle requires new cycle (old rounds don't count for approval)
    #[test]
    fn test_regression_stale_cycle_not_approved() {
        let (review, _) = fully_approved_review();
        let mut hashes = BTreeMap::new();
        hashes.insert(grn("domain"), "hash-current".into());
        hashes.insert(grn("other"), "hash-other".into());

        // With matching snapshot → approved
        let snapshot_match = make_snapshot("sha256:b", "sha256:e");
        assert_eq!(
            check_cycle_approved(&review, &snapshot_match, &hashes),
            CheckCycleApprovedResult::Approved
        );

        // With changed base policy → stale, not approved
        let snapshot_stale = make_snapshot("sha256:changed", "sha256:e");
        assert!(matches!(
            check_cycle_approved(&review, &snapshot_stale, &hashes),
            CheckCycleApprovedResult::NotApproved(CheckCycleApprovedReason::Stale(_))
        ));
    }

    /// AC: final round required for all groups (including other)
    #[test]
    fn test_regression_final_required_for_all_groups() {
        let mut review = review_with_cycle("sha256:b", "sha256:e");
        let snapshot = make_snapshot("sha256:b", "sha256:e");

        // Add fast + final for domain
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("domain"),
                round_type: RoundType::Final,
                timestamp: ts("2026-03-30T10:02:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash-current".into(),
            },
        )
        .unwrap();
        // other has fast only — final missing
        record_cycle_group_round(
            &mut review,
            RecordCycleGroupRoundInput {
                group_name: grn("other"),
                round_type: RoundType::Fast,
                timestamp: ts("2026-03-30T10:03:00Z"),
                outcome: RecordRoundOutcome::Success(GroupRoundVerdict::ZeroFindings),
                group_hash: "hash-other".into(),
            },
        )
        .unwrap();

        let mut hashes = BTreeMap::new();
        hashes.insert(grn("domain"), "hash-current".into());
        hashes.insert(grn("other"), "hash-other".into());

        // Fast-only for other → not approved (final required)
        let result = check_cycle_approved(&review, &snapshot, &hashes);
        assert_eq!(
            result,
            CheckCycleApprovedResult::NotApproved(
                CheckCycleApprovedReason::ApprovalRequirementsNotMet
            )
        );
    }

    /// AC: NoCycle returns not-approved (planning-only check is CLI level)
    #[test]
    fn test_regression_no_cycle_not_approved() {
        let review = ReviewJson::new();
        let snapshot = make_snapshot("sha256:b", "sha256:e");
        assert_eq!(
            check_cycle_approved(&review, &snapshot, &BTreeMap::new()),
            CheckCycleApprovedResult::NotApproved(CheckCycleApprovedReason::NoCycle)
        );
    }
}
