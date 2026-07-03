//! Branch strategy domain types.

use crate::NonEmptyString;

/// The merge method used when integrating a track branch into the merge target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Squash,
    Merge,
    Rebase,
}

/// Immutable snapshot of branch strategy configuration captured at track init time.
///
/// Carries base_branch, merge_target, and merge_method. Created at `/track:init` time
/// and stored in `metadata.json#branch_strategy_snapshot` so that global config
/// changes do not affect in-flight tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchStrategySnapshot {
    base_branch: NonEmptyString,
    merge_target: NonEmptyString,
    merge_method: MergeMethod,
}

impl BranchStrategySnapshot {
    /// Creates a new snapshot with the given base branch, merge target, and merge method.
    pub fn new(
        base_branch: NonEmptyString,
        merge_target: NonEmptyString,
        merge_method: MergeMethod,
    ) -> Self {
        Self { base_branch, merge_target, merge_method }
    }

    /// Returns the base branch name (branch from which track branches are created).
    #[must_use]
    pub fn base_branch(&self) -> &str {
        self.base_branch.as_ref()
    }

    /// Returns the merge target branch name (branch into which track branches are merged).
    #[must_use]
    pub fn merge_target(&self) -> &str {
        self.merge_target.as_ref()
    }

    /// Returns the merge method.
    #[must_use]
    pub fn merge_method(&self) -> MergeMethod {
        self.merge_method
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_snapshot(base: &str, target: &str, method: MergeMethod) -> BranchStrategySnapshot {
        BranchStrategySnapshot::new(
            NonEmptyString::try_new(base).unwrap(),
            NonEmptyString::try_new(target).unwrap(),
            method,
        )
    }

    #[test]
    fn branch_strategy_snapshot_accessors_return_stored_values() {
        let snap = make_snapshot("main", "main", MergeMethod::Squash);
        assert_eq!(snap.base_branch(), "main");
        assert_eq!(snap.merge_target(), "main");
        assert_eq!(snap.merge_method(), MergeMethod::Squash);
    }

    #[test]
    fn branch_strategy_snapshot_develop_variant() {
        let snap = make_snapshot("develop", "develop", MergeMethod::Merge);
        assert_eq!(snap.base_branch(), "develop");
        assert_eq!(snap.merge_target(), "develop");
        assert_eq!(snap.merge_method(), MergeMethod::Merge);
    }

    #[test]
    fn merge_method_rebase_stored_correctly() {
        let snap = make_snapshot("main", "main", MergeMethod::Rebase);
        assert_eq!(snap.merge_method(), MergeMethod::Rebase);
    }

    #[test]
    fn branch_strategy_snapshot_equality() {
        let a = make_snapshot("main", "main", MergeMethod::Squash);
        let b = make_snapshot("main", "main", MergeMethod::Squash);
        assert_eq!(a, b);
    }

    #[test]
    fn branch_strategy_snapshot_inequality_on_method() {
        let a = make_snapshot("main", "main", MergeMethod::Squash);
        let b = make_snapshot("main", "main", MergeMethod::Merge);
        assert_ne!(a, b);
    }
}
