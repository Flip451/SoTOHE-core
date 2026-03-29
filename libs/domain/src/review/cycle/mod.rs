//! Cycle-based review state model for review.json (schema_version 1).
//!
//! Represents the append-only, per-group review cycle model where each group
//! maintains independent round history and staleness is determined by
//! group-scope hash comparison.

mod round_types;

pub use round_types::{
    CycleError, CycleGroupState, GroupRound, GroupRoundOutcome, GroupRoundVerdict,
    NonEmptyFindings, ReviewStalenessReason, StoredFinding,
};

use std::collections::BTreeMap;

use crate::{ReviewGroupName, Timestamp};

use super::types::RoundType;

// ---------------------------------------------------------------------------
// ReviewCycle
// ---------------------------------------------------------------------------

/// A single review cycle with frozen context.
///
/// A cycle captures the review state at a point in time: which base ref was used,
/// what the policy hash was, and what files belong to each group. Groups
/// independently accumulate rounds within the cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCycle {
    cycle_id: String,
    started_at: Timestamp,
    base_ref: String,
    policy_hash: String,
    groups: BTreeMap<ReviewGroupName, CycleGroupState>,
}

impl ReviewCycle {
    /// Creates a new review cycle.
    ///
    /// # Errors
    /// Returns `CycleError::MissingOtherGroup` if the groups map does not
    /// contain a mandatory `"other"` group.
    pub fn new(
        cycle_id: impl Into<String>,
        started_at: Timestamp,
        base_ref: impl Into<String>,
        policy_hash: impl Into<String>,
        groups: BTreeMap<ReviewGroupName, CycleGroupState>,
    ) -> Result<Self, CycleError> {
        let other_key = ReviewGroupName::try_new("other")
            .map_err(|e| CycleError::Internal(format!("failed to create 'other' key: {e}")))?;
        if !groups.contains_key(&other_key) {
            return Err(CycleError::MissingOtherGroup);
        }
        Ok(Self {
            cycle_id: cycle_id.into(),
            started_at,
            base_ref: base_ref.into(),
            policy_hash: policy_hash.into(),
            groups,
        })
    }

    /// Creates a cycle without validation (for deserialization of trusted data).
    #[must_use]
    pub fn from_parts(
        cycle_id: String,
        started_at: Timestamp,
        base_ref: String,
        policy_hash: String,
        groups: BTreeMap<ReviewGroupName, CycleGroupState>,
    ) -> Self {
        Self { cycle_id, started_at, base_ref, policy_hash, groups }
    }

    /// Returns the cycle ID.
    #[must_use]
    pub fn cycle_id(&self) -> &str {
        &self.cycle_id
    }

    /// Returns the cycle start timestamp.
    #[must_use]
    pub fn started_at(&self) -> &Timestamp {
        &self.started_at
    }

    /// Returns the base ref (e.g., "main").
    #[must_use]
    pub fn base_ref(&self) -> &str {
        &self.base_ref
    }

    /// Returns the policy hash frozen at cycle start.
    #[must_use]
    pub fn policy_hash(&self) -> &str {
        &self.policy_hash
    }

    /// Returns the groups map.
    #[must_use]
    pub fn groups(&self) -> &BTreeMap<ReviewGroupName, CycleGroupState> {
        &self.groups
    }

    /// Returns a reference to a named group, if it exists.
    #[must_use]
    pub fn group(&self, name: &ReviewGroupName) -> Option<&CycleGroupState> {
        self.groups.get(name)
    }

    /// Returns a mutable reference to a named group, if it exists.
    pub fn group_mut(&mut self, name: &ReviewGroupName) -> Option<&mut CycleGroupState> {
        self.groups.get_mut(name)
    }

    /// Returns the group names in this cycle.
    pub fn group_names(&self) -> impl Iterator<Item = &ReviewGroupName> {
        self.groups.keys()
    }

    /// Checks whether all groups have approved status (fail-closed).
    ///
    /// For each group, the **latest** round of each type (fast and final) must be:
    /// - successful with zero findings
    /// - recorded against the current group hash
    ///
    /// The key sets must match exactly: if `current_hashes` contains groups not
    /// in the cycle (partition drift) or vice versa, returns `false`.
    ///
    /// The latest final round must come after the latest fast round in the
    /// append-only history (prevents Fast→Final→Fast bypass).
    ///
    /// A later failure overrides an earlier success for the same round type,
    /// ensuring fail-closed behavior. This is the per-group check used by
    /// `check-approved`.
    #[must_use]
    pub fn all_groups_approved(&self, current_hashes: &BTreeMap<ReviewGroupName, String>) -> bool {
        // Fail-closed: partition must match exactly
        if self.groups.len() != current_hashes.len() {
            return false;
        }
        self.groups.iter().all(|(name, group_state)| {
            let current = match current_hashes.get(name) {
                Some(h) => h,
                None => return false,
            };
            let fast_ok = group_state
                .latest_round(RoundType::Fast)
                .is_some_and(|r| r.is_successful_zero_findings() && r.hash() == current);
            let final_ok = group_state
                .latest_round(RoundType::Final)
                .is_some_and(|r| r.is_successful_zero_findings() && r.hash() == current);
            // Final must come after the latest fast (prevents Fast→Final→Fast bypass)
            fast_ok && final_ok && group_state.final_after_latest_fast()
        })
    }

    /// Determines the staleness reason for a given group, if any.
    ///
    /// Compares the cycle's frozen state against current values.
    #[must_use]
    pub fn check_group_staleness(
        &self,
        group_name: &ReviewGroupName,
        current_policy_hash: &str,
        current_group_hash: Option<&str>,
    ) -> Option<ReviewStalenessReason> {
        if self.policy_hash != current_policy_hash {
            return Some(ReviewStalenessReason::PolicyChanged);
        }
        // If the group doesn't exist in this cycle, partition changed
        let group_state = match self.groups.get(group_name) {
            Some(gs) => gs,
            None => return Some(ReviewStalenessReason::PartitionChanged),
        };
        // If the group exists in the cycle but has no current hash,
        // the group disappeared from the current partition.
        let current_hash = match current_group_hash {
            Some(h) => h,
            None => return Some(ReviewStalenessReason::PartitionChanged),
        };
        // Check hash of the most recently recorded round (any type).
        // In the append-only model, the last round is the most recent action.
        if let Some(round) = group_state.latest_round_any() {
            if round.hash() != current_hash {
                return Some(ReviewStalenessReason::HashMismatch);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// ReviewJson
// ---------------------------------------------------------------------------

/// Top-level review.json representation.
///
/// Contains the schema version and an ordered list of review cycles.
/// When no cycles exist, the review state is `NoCycle` (equivalent to
/// `NotStarted` in the legacy model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewJson {
    schema_version: u32,
    cycles: Vec<ReviewCycle>,
}

impl ReviewJson {
    /// Schema version for the cycle-based format.
    pub const SCHEMA_VERSION: u32 = 1;

    /// Creates a new empty review.json (NoCycle state).
    #[must_use]
    pub fn new() -> Self {
        Self { schema_version: Self::SCHEMA_VERSION, cycles: Vec::new() }
    }

    /// Creates a review.json from parts (for deserialization).
    ///
    /// # Errors
    /// Returns `CycleError::UnsupportedSchemaVersion` if the version is not
    /// the supported schema version (1).
    pub fn from_parts(schema_version: u32, cycles: Vec<ReviewCycle>) -> Result<Self, CycleError> {
        if schema_version != Self::SCHEMA_VERSION {
            return Err(CycleError::UnsupportedSchemaVersion {
                expected: Self::SCHEMA_VERSION,
                actual: schema_version,
            });
        }
        Ok(Self { schema_version, cycles })
    }

    /// Returns the schema version.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns all cycles.
    #[must_use]
    pub fn cycles(&self) -> &[ReviewCycle] {
        &self.cycles
    }

    /// Returns `true` if no cycles exist (NoCycle state).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cycles.is_empty()
    }

    /// Returns the current (latest) cycle, if any.
    #[must_use]
    pub fn current_cycle(&self) -> Option<&ReviewCycle> {
        self.cycles.last()
    }

    /// Returns a mutable reference to the current (latest) cycle, if any.
    pub fn current_cycle_mut(&mut self) -> Option<&mut ReviewCycle> {
        self.cycles.last_mut()
    }

    /// Starts a new review cycle, appending it to the cycle list.
    ///
    /// # Errors
    /// Returns `CycleError::MissingOtherGroup` if the groups map does not
    /// contain a mandatory `"other"` group.
    pub fn start_cycle(
        &mut self,
        cycle_id: impl Into<String>,
        started_at: Timestamp,
        base_ref: impl Into<String>,
        policy_hash: impl Into<String>,
        groups: BTreeMap<ReviewGroupName, CycleGroupState>,
    ) -> Result<&mut ReviewCycle, CycleError> {
        let cycle = ReviewCycle::new(cycle_id, started_at, base_ref, policy_hash, groups)?;
        self.cycles.push(cycle);
        self.cycles
            .last_mut()
            .ok_or_else(|| CycleError::Internal("cycle list empty after push".into()))
    }
}

impl Default for ReviewJson {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::review::types::Verdict;

    fn ts(s: &str) -> Timestamp {
        Timestamp::new(s).unwrap()
    }

    fn grn(s: &str) -> ReviewGroupName {
        ReviewGroupName::try_new(s).unwrap()
    }

    fn sample_groups() -> BTreeMap<ReviewGroupName, CycleGroupState> {
        let mut groups = BTreeMap::new();
        groups.insert(grn("domain"), CycleGroupState::new(vec!["libs/domain/src/lib.rs".into()]));
        groups.insert(grn("other"), CycleGroupState::new(vec!["Makefile.toml".into()]));
        groups
    }

    fn success_round(round_type: RoundType, hash: &str, ts_str: &str) -> GroupRound {
        GroupRound::success(round_type, ts(ts_str), hash, GroupRoundVerdict::ZeroFindings).unwrap()
    }

    fn failure_round(round_type: RoundType, hash: &str, ts_str: &str) -> GroupRound {
        GroupRound::failure(round_type, ts(ts_str), hash, Some("timeout".into())).unwrap()
    }

    // -- ReviewJson --

    #[test]
    fn test_review_json_new_is_empty() {
        let rj = ReviewJson::new();
        assert!(rj.is_empty());
        assert_eq!(rj.schema_version(), ReviewJson::SCHEMA_VERSION);
        assert!(rj.current_cycle().is_none());
    }

    #[test]
    fn test_review_json_start_cycle_adds_cycle() {
        let mut rj = ReviewJson::new();
        rj.start_cycle(
            "2026-03-29T09:47:00Z",
            ts("2026-03-29T09:47:00Z"),
            "main",
            "sha256:abc123",
            sample_groups(),
        )
        .unwrap();
        assert!(!rj.is_empty());
        assert_eq!(rj.cycles().len(), 1);
    }

    #[test]
    fn test_review_json_current_cycle_returns_latest() {
        let mut rj = ReviewJson::new();
        rj.start_cycle(
            "cycle-1",
            ts("2026-03-29T09:00:00Z"),
            "main",
            "sha256:hash1",
            sample_groups(),
        )
        .unwrap();
        rj.start_cycle(
            "cycle-2",
            ts("2026-03-29T10:00:00Z"),
            "main",
            "sha256:hash2",
            sample_groups(),
        )
        .unwrap();
        let current = rj.current_cycle().unwrap();
        assert_eq!(current.cycle_id(), "cycle-2");
    }

    #[test]
    fn test_review_json_start_cycle_requires_other_group() {
        let mut rj = ReviewJson::new();
        let mut groups = BTreeMap::new();
        groups.insert(grn("domain"), CycleGroupState::new(vec![]));
        let result =
            rj.start_cycle("cycle-1", ts("2026-03-29T09:00:00Z"), "main", "sha256:abc", groups);
        assert!(matches!(result, Err(CycleError::MissingOtherGroup)));
    }

    #[test]
    fn test_review_json_from_parts_rejects_unsupported_version() {
        let result = ReviewJson::from_parts(99, vec![]);
        assert!(matches!(
            result,
            Err(CycleError::UnsupportedSchemaVersion { expected: 1, actual: 99 })
        ));
    }

    #[test]
    fn test_review_json_from_parts_accepts_version_1() {
        let result = ReviewJson::from_parts(1, vec![]);
        assert!(result.is_ok());
    }

    // -- ReviewCycle --

    #[test]
    fn test_review_cycle_new_requires_other_group() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("cli"), CycleGroupState::new(vec![]));
        let result =
            ReviewCycle::new("cycle-1", ts("2026-03-29T09:00:00Z"), "main", "sha256:abc", groups);
        assert!(matches!(result, Err(CycleError::MissingOtherGroup)));
    }

    #[test]
    fn test_review_cycle_getters() {
        let cycle = ReviewCycle::new(
            "2026-03-29T09:47:00Z",
            ts("2026-03-29T09:47:00Z"),
            "main",
            "sha256:abc123",
            sample_groups(),
        )
        .unwrap();
        assert_eq!(cycle.cycle_id(), "2026-03-29T09:47:00Z");
        assert_eq!(cycle.base_ref(), "main");
        assert_eq!(cycle.policy_hash(), "sha256:abc123");
        assert_eq!(cycle.groups().len(), 2);
    }

    #[test]
    fn test_review_cycle_group_access() {
        let cycle = ReviewCycle::new(
            "cycle-1",
            ts("2026-03-29T09:00:00Z"),
            "main",
            "sha256:abc",
            sample_groups(),
        )
        .unwrap();
        assert!(cycle.group(&grn("domain")).is_some());
        assert!(cycle.group(&grn("other")).is_some());
        assert!(cycle.group(&grn("nonexistent")).is_none());
    }

    #[test]
    fn test_review_cycle_group_mut_record_round() {
        let mut cycle = ReviewCycle::new(
            "cycle-1",
            ts("2026-03-29T09:00:00Z"),
            "main",
            "sha256:abc",
            sample_groups(),
        )
        .unwrap();
        let domain_group = cycle.group_mut(&grn("domain")).unwrap();
        domain_group.record_round(success_round(
            RoundType::Fast,
            "rvw1:sha256:def",
            "2026-03-29T09:48:00Z",
        ));
        assert_eq!(cycle.group(&grn("domain")).unwrap().rounds().len(), 1);
    }

    // -- CycleGroupState --

    #[test]
    fn test_cycle_group_state_new_is_empty() {
        let gs = CycleGroupState::new(vec!["src/main.rs".into()]);
        assert!(gs.is_empty());
        assert_eq!(gs.scope(), &["src/main.rs"]);
    }

    #[test]
    fn test_cycle_group_state_record_round_appends() {
        let mut gs = CycleGroupState::new(vec![]);
        gs.record_round(success_round(RoundType::Fast, "hash1", "2026-03-29T09:00:00Z"));
        gs.record_round(success_round(RoundType::Final, "hash1", "2026-03-29T09:01:00Z"));
        assert_eq!(gs.rounds().len(), 2);
        assert_eq!(gs.rounds()[0].round_type(), RoundType::Fast);
        assert_eq!(gs.rounds()[1].round_type(), RoundType::Final);
    }

    #[test]
    fn test_cycle_group_state_latest_successful_fast_round() {
        let mut gs = CycleGroupState::new(vec![]);
        gs.record_round(success_round(RoundType::Fast, "hash1", "2026-03-29T09:00:00Z"));
        gs.record_round(failure_round(RoundType::Fast, "hash2", "2026-03-29T09:10:00Z"));
        gs.record_round(success_round(RoundType::Fast, "hash3", "2026-03-29T09:20:00Z"));
        let latest = gs.latest_successful_round(RoundType::Fast).unwrap();
        assert_eq!(latest.hash(), "hash3");
    }

    #[test]
    fn test_cycle_group_state_latest_successful_final_round() {
        let mut gs = CycleGroupState::new(vec![]);
        gs.record_round(success_round(RoundType::Final, "hash1", "2026-03-29T09:00:00Z"));
        gs.record_round(success_round(RoundType::Final, "hash2", "2026-03-29T10:00:00Z"));
        let latest = gs.latest_successful_round(RoundType::Final).unwrap();
        assert_eq!(latest.hash(), "hash2");
    }

    #[test]
    fn test_cycle_group_state_latest_successful_skips_failures() {
        let mut gs = CycleGroupState::new(vec![]);
        gs.record_round(success_round(RoundType::Fast, "hash1", "2026-03-29T09:00:00Z"));
        gs.record_round(failure_round(RoundType::Fast, "hash2", "2026-03-29T09:10:00Z"));
        let latest = gs.latest_successful_round(RoundType::Fast).unwrap();
        assert_eq!(latest.hash(), "hash1");
    }

    #[test]
    fn test_cycle_group_state_latest_successful_returns_none_when_all_failed() {
        let mut gs = CycleGroupState::new(vec![]);
        gs.record_round(failure_round(RoundType::Fast, "hash1", "2026-03-29T09:00:00Z"));
        assert!(gs.latest_successful_round(RoundType::Fast).is_none());
    }

    #[test]
    fn test_cycle_group_state_latest_successful_returns_none_when_empty() {
        let gs = CycleGroupState::new(vec![]);
        assert!(gs.latest_successful_round(RoundType::Fast).is_none());
        assert!(gs.latest_successful_round(RoundType::Final).is_none());
    }

    // -- GroupRound --

    #[test]
    fn test_group_round_successful_zero_findings() {
        let round = success_round(RoundType::Fast, "hash1", "2026-03-29T09:00:00Z");
        assert!(round.is_successful_zero_findings());
        assert!(round.outcome().is_success());
        assert!(round.outcome().error_message().is_none());
    }

    #[test]
    fn test_group_round_failure_not_successful() {
        let round = failure_round(RoundType::Fast, "hash1", "2026-03-29T09:00:00Z");
        assert!(!round.is_successful_zero_findings());
        assert!(!round.outcome().is_success());
        assert_eq!(round.outcome().error_message(), Some("timeout"));
    }

    #[test]
    fn test_group_round_success_with_findings_not_zero() {
        let round = GroupRound::success(
            RoundType::Fast,
            ts("2026-03-29T09:00:00Z"),
            "hash1",
            GroupRoundVerdict::findings_remain(vec![StoredFinding::new("issue", None, None, None)])
                .unwrap(),
        )
        .unwrap();
        assert!(!round.is_successful_zero_findings());
    }

    // -- GroupRoundVerdict --

    #[test]
    fn test_verdict_zero_findings() {
        let v = GroupRoundVerdict::ZeroFindings;
        assert!(v.is_zero_findings());
        assert_eq!(v.verdict(), Verdict::ZeroFindings);
        assert!(v.findings().is_empty());
    }

    #[test]
    fn test_verdict_findings_remain() {
        let findings =
            vec![StoredFinding::new("bug", Some("P1".into()), Some("src/lib.rs".into()), Some(10))];
        let v = GroupRoundVerdict::findings_remain(findings).unwrap();
        assert!(!v.is_zero_findings());
        assert_eq!(v.findings().len(), 1);
    }

    #[test]
    fn test_verdict_findings_remain_with_empty_findings_rejected() {
        let result = GroupRoundVerdict::findings_remain(vec![]);
        assert!(matches!(result, Err(CycleError::InconsistentVerdict(_))));
    }

    // -- StoredFinding --

    #[test]
    fn test_stored_finding_all_fields() {
        let f = StoredFinding::new("msg", Some("P1".into()), Some("src/main.rs".into()), Some(42));
        assert_eq!(f.message(), "msg");
        assert_eq!(f.severity(), Some("P1"));
        assert_eq!(f.file(), Some("src/main.rs"));
        assert_eq!(f.line(), Some(42));
    }

    #[test]
    fn test_stored_finding_optional_fields_none() {
        let f = StoredFinding::new("msg", None, None, None);
        assert_eq!(f.message(), "msg");
        assert!(f.severity().is_none());
        assert!(f.file().is_none());
        assert!(f.line().is_none());
    }

    // -- ReviewStalenessReason --

    #[test]
    fn test_staleness_reason_variants_are_distinct() {
        assert_ne!(ReviewStalenessReason::PolicyChanged, ReviewStalenessReason::PartitionChanged);
        assert_ne!(ReviewStalenessReason::PolicyChanged, ReviewStalenessReason::HashMismatch);
        assert_ne!(ReviewStalenessReason::PartitionChanged, ReviewStalenessReason::HashMismatch);
    }

    // -- Staleness checks --

    #[test]
    fn test_check_group_staleness_policy_changed() {
        let cycle = ReviewCycle::new(
            "cycle-1",
            ts("2026-03-29T09:00:00Z"),
            "main",
            "sha256:original",
            sample_groups(),
        )
        .unwrap();
        let result = cycle.check_group_staleness(&grn("domain"), "sha256:changed", None);
        assert_eq!(result, Some(ReviewStalenessReason::PolicyChanged));
    }

    #[test]
    fn test_check_group_staleness_partition_changed() {
        let cycle = ReviewCycle::new(
            "cycle-1",
            ts("2026-03-29T09:00:00Z"),
            "main",
            "sha256:abc",
            sample_groups(),
        )
        .unwrap();
        let result = cycle.check_group_staleness(&grn("new-group"), "sha256:abc", None);
        assert_eq!(result, Some(ReviewStalenessReason::PartitionChanged));
    }

    #[test]
    fn test_check_group_staleness_hash_mismatch() {
        let mut groups = sample_groups();
        groups.get_mut(&grn("domain")).unwrap().record_round(success_round(
            RoundType::Final,
            "hash-old",
            "2026-03-29T09:00:00Z",
        ));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let result = cycle.check_group_staleness(&grn("domain"), "sha256:abc", Some("hash-new"));
        assert_eq!(result, Some(ReviewStalenessReason::HashMismatch));
    }

    #[test]
    fn test_check_group_staleness_hash_mismatch_fast_only() {
        let mut groups = sample_groups();
        groups.get_mut(&grn("domain")).unwrap().record_round(success_round(
            RoundType::Fast,
            "hash-old",
            "2026-03-29T09:00:00Z",
        ));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let result = cycle.check_group_staleness(&grn("domain"), "sha256:abc", Some("hash-new"));
        assert_eq!(result, Some(ReviewStalenessReason::HashMismatch));
    }

    #[test]
    fn test_check_group_staleness_group_disappeared_from_partition() {
        let cycle = ReviewCycle::new(
            "cycle-1",
            ts("2026-03-29T09:00:00Z"),
            "main",
            "sha256:abc",
            sample_groups(),
        )
        .unwrap();
        let result = cycle.check_group_staleness(&grn("domain"), "sha256:abc", None);
        assert_eq!(result, Some(ReviewStalenessReason::PartitionChanged));
    }

    #[test]
    fn test_check_group_staleness_uses_latest_round_any() {
        let mut groups = sample_groups();
        let domain = groups.get_mut(&grn("domain")).unwrap();
        domain.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:00:00Z"));
        domain.record_round(success_round(RoundType::Fast, "hash-b", "2026-03-29T09:10:00Z"));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        assert!(
            cycle.check_group_staleness(&grn("domain"), "sha256:abc", Some("hash-b")).is_none()
        );
        assert_eq!(
            cycle.check_group_staleness(&grn("domain"), "sha256:abc", Some("hash-a")),
            Some(ReviewStalenessReason::HashMismatch)
        );
    }

    #[test]
    fn test_check_group_staleness_no_staleness() {
        let mut groups = sample_groups();
        groups.get_mut(&grn("domain")).unwrap().record_round(success_round(
            RoundType::Final,
            "hash-current",
            "2026-03-29T09:00:00Z",
        ));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        assert!(
            cycle
                .check_group_staleness(&grn("domain"), "sha256:abc", Some("hash-current"))
                .is_none()
        );
    }

    // -- all_groups_approved --

    #[test]
    fn test_all_groups_approved_fails_when_fast_after_final() {
        let mut groups = sample_groups();
        let domain = groups.get_mut(&grn("domain")).unwrap();
        domain.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
        domain.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:05:00Z"));
        domain.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:10:00Z"));
        let other = groups.get_mut(&grn("other")).unwrap();
        other.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
        other.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:01:00Z"));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-a".into());
        h.insert(grn("other"), "hash-a".into());
        assert!(!cycle.all_groups_approved(&h));
    }

    #[test]
    fn test_final_after_latest_fast() {
        let mut gs = CycleGroupState::new(vec![]);
        gs.record_round(success_round(RoundType::Fast, "h", "2026-03-29T09:00:00Z"));
        gs.record_round(success_round(RoundType::Final, "h", "2026-03-29T09:01:00Z"));
        assert!(gs.final_after_latest_fast());
    }

    #[test]
    fn test_final_after_latest_fast_false_when_reopened() {
        let mut gs = CycleGroupState::new(vec![]);
        gs.record_round(success_round(RoundType::Fast, "h", "2026-03-29T09:00:00Z"));
        gs.record_round(success_round(RoundType::Final, "h", "2026-03-29T09:01:00Z"));
        gs.record_round(success_round(RoundType::Fast, "h", "2026-03-29T09:02:00Z"));
        assert!(!gs.final_after_latest_fast());
    }

    #[test]
    fn test_all_groups_approved_when_all_have_fast_and_final() {
        let mut groups = sample_groups();
        for (_, gs) in groups.iter_mut() {
            gs.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
            gs.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:01:00Z"));
        }
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-a".into());
        h.insert(grn("other"), "hash-a".into());
        assert!(cycle.all_groups_approved(&h));
    }

    #[test]
    fn test_all_groups_approved_fails_without_final() {
        let mut groups = sample_groups();
        groups.get_mut(&grn("domain")).unwrap().record_round(success_round(
            RoundType::Fast,
            "hash-a",
            "2026-03-29T09:00:00Z",
        ));
        let other = groups.get_mut(&grn("other")).unwrap();
        other.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
        other.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:01:00Z"));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-a".into());
        h.insert(grn("other"), "hash-a".into());
        assert!(!cycle.all_groups_approved(&h));
    }

    #[test]
    fn test_all_groups_approved_fails_with_stale_hash() {
        let mut groups = sample_groups();
        for (_, gs) in groups.iter_mut() {
            gs.record_round(success_round(RoundType::Fast, "hash-old", "2026-03-29T09:00:00Z"));
            gs.record_round(success_round(RoundType::Final, "hash-old", "2026-03-29T09:01:00Z"));
        }
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-new".into());
        h.insert(grn("other"), "hash-old".into());
        assert!(!cycle.all_groups_approved(&h));
    }

    #[test]
    fn test_all_groups_approved_fails_with_stale_fast_hash() {
        let mut groups = sample_groups();
        let domain = groups.get_mut(&grn("domain")).unwrap();
        domain.record_round(success_round(RoundType::Fast, "hash-old", "2026-03-29T09:00:00Z"));
        domain.record_round(success_round(RoundType::Final, "hash-new", "2026-03-29T09:05:00Z"));
        let other = groups.get_mut(&grn("other")).unwrap();
        other.record_round(success_round(RoundType::Fast, "hash-new", "2026-03-29T09:00:00Z"));
        other.record_round(success_round(RoundType::Final, "hash-new", "2026-03-29T09:01:00Z"));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-new".into());
        h.insert(grn("other"), "hash-new".into());
        assert!(!cycle.all_groups_approved(&h));
    }

    #[test]
    fn test_all_groups_approved_fails_when_later_round_has_findings() {
        let mut groups = sample_groups();
        let domain = groups.get_mut(&grn("domain")).unwrap();
        domain.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
        domain.record_round(failure_round(RoundType::Fast, "hash-a", "2026-03-29T09:05:00Z"));
        domain.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:10:00Z"));
        let other = groups.get_mut(&grn("other")).unwrap();
        other.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
        other.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:01:00Z"));
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-a".into());
        h.insert(grn("other"), "hash-a".into());
        assert!(!cycle.all_groups_approved(&h));
    }

    #[test]
    fn test_all_groups_approved_fails_when_extra_group_in_hashes() {
        let mut groups = sample_groups();
        for (_, gs) in groups.iter_mut() {
            gs.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
            gs.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:01:00Z"));
        }
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-a".into());
        h.insert(grn("other"), "hash-a".into());
        h.insert(grn("new-group"), "hash-a".into());
        assert!(!cycle.all_groups_approved(&h));
    }

    #[test]
    fn test_all_groups_approved_fails_when_group_missing_from_hashes() {
        let mut groups = sample_groups();
        for (_, gs) in groups.iter_mut() {
            gs.record_round(success_round(RoundType::Fast, "hash-a", "2026-03-29T09:00:00Z"));
            gs.record_round(success_round(RoundType::Final, "hash-a", "2026-03-29T09:01:00Z"));
        }
        let cycle = ReviewCycle::from_parts(
            "cycle-1".into(),
            ts("2026-03-29T09:00:00Z"),
            "main".into(),
            "sha256:abc".into(),
            groups,
        );
        let mut h = BTreeMap::new();
        h.insert(grn("domain"), "hash-a".into());
        assert!(!cycle.all_groups_approved(&h));
    }

    // -- Group-independent round progression --

    #[test]
    fn test_group_independence_zero_findings_not_invalidated_by_other_group_retry() {
        let mut groups = sample_groups();
        groups.get_mut(&grn("domain")).unwrap().record_round(success_round(
            RoundType::Fast,
            "hash-d",
            "2026-03-29T09:00:00Z",
        ));
        let other = groups.get_mut(&grn("other")).unwrap();
        other.record_round(failure_round(RoundType::Fast, "hash-o", "2026-03-29T09:10:00Z"));
        other.record_round(success_round(RoundType::Fast, "hash-o", "2026-03-29T09:20:00Z"));
        let domain = groups.get(&grn("domain")).unwrap();
        let latest = domain.latest_successful_round(RoundType::Fast).unwrap();
        assert_eq!(latest.hash(), "hash-d");
        assert!(latest.is_successful_zero_findings());
    }

    // -- GroupRoundOutcome --

    #[test]
    fn test_group_round_outcome_success_has_verdict() {
        let outcome = GroupRoundOutcome::Success(GroupRoundVerdict::ZeroFindings);
        assert!(outcome.is_success());
        assert!(outcome.verdict().is_some());
        assert!(outcome.error_message().is_none());
    }

    #[test]
    fn test_group_round_outcome_failure_has_no_verdict() {
        let outcome = GroupRoundOutcome::Failure { error_message: Some("timeout".into()) };
        assert!(!outcome.is_success());
        assert!(outcome.verdict().is_none());
        assert_eq!(outcome.error_message(), Some("timeout"));
    }

    // -- Hash validation --

    #[test]
    fn test_group_round_rejects_empty_hash() {
        let result = GroupRound::success(
            RoundType::Fast,
            ts("2026-03-29T09:00:00Z"),
            "",
            GroupRoundVerdict::ZeroFindings,
        );
        assert!(matches!(result, Err(CycleError::InvalidHash(_))));
    }

    #[test]
    fn test_group_round_rejects_whitespace_hash() {
        let result = GroupRound::success(
            RoundType::Fast,
            ts("2026-03-29T09:00:00Z"),
            "   ",
            GroupRoundVerdict::ZeroFindings,
        );
        assert!(matches!(result, Err(CycleError::InvalidHash(_))));
    }

    #[test]
    fn test_group_round_rejects_pending_hash() {
        let result =
            GroupRound::failure(RoundType::Fast, ts("2026-03-29T09:00:00Z"), "PENDING", None);
        assert!(matches!(result, Err(CycleError::InvalidHash(_))));
    }
}
