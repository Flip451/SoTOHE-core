//! Usecase functions for review cycle lifecycle management.
//!
//! Wraps domain-layer cycle operations with partition-snapshot–aware inputs.

use domain::{CycleError, ReviewGroupName, ReviewJson, ReviewStalenessReason, Timestamp};

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
}
