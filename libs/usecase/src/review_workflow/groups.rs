//! Group partition types for per-group review scope.
//!
//! A `GroupPartition` maps `ReviewGroupName` → `Vec<RepoRelativePath>`,
//! guaranteeing the mandatory `other` group is always present.

use std::collections::BTreeMap;

use domain::{CycleGroupState, ReviewGroupName};
use thiserror::Error;

use crate::review_workflow::scope::RepoRelativePath;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from group partition construction.
#[derive(Debug, Error)]
pub enum GroupPartitionError {
    #[error("group partition must contain mandatory 'other' group")]
    MissingOtherGroup,

    #[error("failed to create 'other' group name: {0}")]
    InvalidOtherName(String),
}

// ---------------------------------------------------------------------------
// GroupPartition
// ---------------------------------------------------------------------------

/// A validated partition of changed files into review groups.
///
/// Invariant: always contains the mandatory `other` group, even if empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupPartition {
    groups: BTreeMap<ReviewGroupName, Vec<RepoRelativePath>>,
}

impl GroupPartition {
    /// Creates a new partition, validating that the mandatory `other` group exists.
    ///
    /// # Errors
    /// Returns `GroupPartitionError::MissingOtherGroup` if the map lacks `other`.
    pub fn try_new(
        groups: BTreeMap<ReviewGroupName, Vec<RepoRelativePath>>,
    ) -> Result<Self, GroupPartitionError> {
        let other_key = ReviewGroupName::try_new("other")
            .map_err(|e| GroupPartitionError::InvalidOtherName(e.to_string()))?;
        if !groups.contains_key(&other_key) {
            return Err(GroupPartitionError::MissingOtherGroup);
        }
        Ok(Self { groups })
    }

    /// Returns the partition map.
    #[must_use]
    pub fn groups(&self) -> &BTreeMap<ReviewGroupName, Vec<RepoRelativePath>> {
        &self.groups
    }

    /// Remaps this partition to the supplied group set, folding every non-member
    /// named group into the mandatory `other` bucket.
    ///
    /// The returned partition always preserves the requested group keys, even when
    /// their current scope is empty.
    ///
    /// # Errors
    /// Returns `GroupPartitionError` if the mandatory `other` group cannot be
    /// constructed.
    pub fn remap_to_group_names(
        &self,
        group_names: &std::collections::BTreeSet<ReviewGroupName>,
    ) -> Result<Self, GroupPartitionError> {
        let other_key = ReviewGroupName::try_new("other")
            .map_err(|e| GroupPartitionError::InvalidOtherName(e.to_string()))?;
        let mut filtered: BTreeMap<ReviewGroupName, Vec<RepoRelativePath>> = BTreeMap::new();

        for name in group_names {
            if *name != other_key {
                filtered.entry(name.clone()).or_default();
            }
        }

        for (name, paths) in &self.groups {
            if group_names.contains(name) && *name != other_key {
                filtered.entry(name.clone()).or_default().extend(paths.iter().cloned());
            } else {
                filtered.entry(other_key.clone()).or_default().extend(paths.iter().cloned());
            }
        }

        filtered.entry(other_key).or_default();
        Self::try_new(filtered)
    }

    /// Returns the sorted list of expected group names.
    #[must_use]
    pub fn expected_groups(&self) -> Vec<ReviewGroupName> {
        self.groups.keys().cloned().collect()
    }

    /// Converts this partition into cycle-compatible group states.
    ///
    /// Each group's scope is the list of repo-relative path strings.
    #[must_use]
    pub fn to_cycle_groups(&self) -> BTreeMap<ReviewGroupName, CycleGroupState> {
        self.groups
            .iter()
            .map(|(name, paths)| {
                let scope: Vec<String> = paths.iter().map(|p| p.as_str().to_owned()).collect();
                (name.clone(), CycleGroupState::new(scope))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ReviewPartitionSnapshot
// ---------------------------------------------------------------------------

/// A frozen snapshot of group partition + policy hashes for cycle creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPartitionSnapshot {
    /// Hash of the base review-scope.json groups (before per-track override).
    base_policy_hash: String,
    /// Hash of the effective (resolved) groups policy after override application.
    policy_hash: String,
    partition: GroupPartition,
}

impl ReviewPartitionSnapshot {
    /// Creates a new partition snapshot with both base and effective policy hashes.
    #[must_use]
    pub fn new(
        base_policy_hash: impl Into<String>,
        policy_hash: impl Into<String>,
        partition: GroupPartition,
    ) -> Self {
        Self {
            base_policy_hash: base_policy_hash.into(),
            policy_hash: policy_hash.into(),
            partition,
        }
    }

    /// Returns the base policy hash (from review-scope.json, before override).
    #[must_use]
    pub fn base_policy_hash(&self) -> &str {
        &self.base_policy_hash
    }

    /// Returns the effective policy hash (after override application).
    #[must_use]
    pub fn policy_hash(&self) -> &str {
        &self.policy_hash
    }

    /// Returns the group partition.
    #[must_use]
    pub fn partition(&self) -> &GroupPartition {
        &self.partition
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn grn(s: &str) -> ReviewGroupName {
        ReviewGroupName::try_new(s).unwrap()
    }

    fn path(s: &str) -> RepoRelativePath {
        RepoRelativePath::normalize(s).unwrap()
    }

    #[test]
    fn test_try_new_requires_other_group() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("domain"), vec![path("libs/domain/src/lib.rs")]);

        let result = GroupPartition::try_new(groups);
        assert!(matches!(result, Err(GroupPartitionError::MissingOtherGroup)));
    }

    #[test]
    fn test_try_new_accepts_other_group() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![]);
        groups.insert(grn("domain"), vec![path("libs/domain/src/lib.rs")]);

        let partition = GroupPartition::try_new(groups).unwrap();
        assert_eq!(partition.groups().len(), 2);
    }

    #[test]
    fn test_try_new_accepts_empty_other() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![]);

        let partition = GroupPartition::try_new(groups).unwrap();
        assert_eq!(partition.groups().len(), 1);
        assert!(partition.groups()[&grn("other")].is_empty());
    }

    #[test]
    fn test_expected_groups_sorted() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![]);
        groups.insert(grn("domain"), vec![]);
        groups.insert(grn("cli"), vec![]);

        let partition = GroupPartition::try_new(groups).unwrap();
        let expected = partition.expected_groups();
        assert_eq!(expected, vec![grn("cli"), grn("domain"), grn("other")]);
    }

    #[test]
    fn test_to_cycle_groups_converts_paths_to_scope_strings() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![path("Makefile.toml")]);
        groups.insert(grn("domain"), vec![path("libs/domain/src/lib.rs")]);

        let partition = GroupPartition::try_new(groups).unwrap();
        let cycle_groups = partition.to_cycle_groups();

        assert_eq!(cycle_groups.len(), 2);
        assert_eq!(cycle_groups[&grn("domain")].scope(), &["libs/domain/src/lib.rs"]);
        assert_eq!(cycle_groups[&grn("other")].scope(), &["Makefile.toml"]);
    }

    #[test]
    fn test_to_cycle_groups_empty_other_produces_empty_scope() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![]);

        let partition = GroupPartition::try_new(groups).unwrap();
        let cycle_groups = partition.to_cycle_groups();

        assert!(cycle_groups[&grn("other")].scope().is_empty());
    }

    #[test]
    fn test_snapshot_preserves_policy_hash() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![]);
        let partition = GroupPartition::try_new(groups).unwrap();

        let snapshot =
            ReviewPartitionSnapshot::new("sha256:base123", "sha256:abc123", partition.clone());
        assert_eq!(snapshot.base_policy_hash(), "sha256:base123");
        assert_eq!(snapshot.policy_hash(), "sha256:abc123");
        assert_eq!(snapshot.partition(), &partition);
    }

    #[test]
    fn test_remap_to_group_names_folds_non_members_into_other() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![path("README.md")]);
        groups.insert(grn("cli"), vec![path("apps/cli/src/lib.rs")]);
        groups.insert(grn("usecase"), vec![path("libs/usecase/src/lib.rs")]);
        let partition = GroupPartition::try_new(groups).unwrap();

        let filtered = partition
            .remap_to_group_names(&[grn("usecase"), grn("other")].into_iter().collect())
            .unwrap();

        assert_eq!(filtered.expected_groups(), vec![grn("other"), grn("usecase")]);
        assert_eq!(
            filtered.groups()[&grn("other")].iter().map(|path| path.as_str()).collect::<Vec<_>>(),
            vec!["apps/cli/src/lib.rs", "README.md"]
        );
        assert_eq!(
            filtered.groups()[&grn("usecase")].iter().map(|path| path.as_str()).collect::<Vec<_>>(),
            vec!["libs/usecase/src/lib.rs"]
        );
    }

    #[test]
    fn test_remap_to_group_names_preserves_requested_empty_groups() {
        let mut groups = BTreeMap::new();
        groups.insert(grn("other"), vec![]);
        groups.insert(grn("cli"), vec![]);
        groups.insert(grn("usecase"), vec![path("libs/usecase/src/lib.rs")]);
        let partition = GroupPartition::try_new(groups).unwrap();

        let filtered = partition
            .remap_to_group_names(&[grn("cli"), grn("usecase"), grn("other")].into_iter().collect())
            .unwrap();

        assert!(filtered.groups().contains_key(&grn("cli")));
        assert!(filtered.groups()[&grn("cli")].is_empty());
    }
}
