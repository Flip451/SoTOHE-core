//! Branch strategy port for the usecase layer.

use domain::branch_strategy::MergeMethod;

/// Port for reading the effective branch strategy.
///
/// Implemented by `JsonConfigBranchStrategyAdapter` (reads from config file) and
/// `SnapshotBranchStrategyAdapter` (reads from metadata snapshot).
/// Adapters live in `libs/infrastructure/src/branch_strategy.rs` (wired in T004).
pub trait BranchStrategyPort: Send + Sync {
    /// Returns the base branch name (branch from which track branches are created).
    fn base_branch(&self) -> &str;

    /// Returns the merge target branch name (branch into which track branches are merged).
    fn merge_target(&self) -> &str;

    /// Returns the merge method (Squash, Merge, or Rebase).
    fn merge_method(&self) -> MergeMethod;

    /// Returns the track branch prefix. Always returns `"track/"` per CN-04.
    fn track_prefix(&self) -> &str;
}
