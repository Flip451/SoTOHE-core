//! Branch strategy resolution and switch-base operation for the `track` command
//! family.
//!
//! Extracted from `mod.rs` to keep the module within the 700-line production
//! code limit (see `knowledge/conventions/impl-delegation-arch-guard.md`).

use std::path::{Path, PathBuf};

use crate::CommandOutcome;
use crate::error::CompositionError;
use crate::track::composition_root::TrackCompositionRoot;

use super::{resolve_project_root, resolve_track_id_inner, validate_track_id_str};

/// Resolve the effective branch strategy from `.harness/config/branch-strategy.json`
/// under `project_root` and materialize it as a [`domain::BranchStrategySnapshot`].
///
/// Used only by pre-track-existence bootstrap operations (`track_init`,
/// `track_branch_create`) that run before any per-track `metadata.json` exists to
/// snapshot from. Fail-closed (CN-03/D5): a missing or malformed config file is
/// propagated as an error, never defaulted.
pub(super) fn resolve_branch_strategy_snapshot(
    project_root: &Path,
) -> Result<domain::BranchStrategySnapshot, CompositionError> {
    use infrastructure::branch_strategy::JsonConfigBranchStrategyAdapter;
    use usecase::branch_strategy::BranchStrategyPort as _;

    let config_path = project_root.join(".harness").join("config").join("branch-strategy.json");
    let adapter = JsonConfigBranchStrategyAdapter::new(config_path)
        .map_err(|e| CompositionError::WiringFailed(format!("branch strategy config: {e}")))?;
    let base_branch = domain::NonEmptyString::try_new(adapter.base_branch())
        .map_err(|e| CompositionError::WiringFailed(format!("branch strategy base_branch: {e}")))?;
    let merge_target = domain::NonEmptyString::try_new(adapter.merge_target()).map_err(|e| {
        CompositionError::WiringFailed(format!("branch strategy merge_target: {e}"))
    })?;
    Ok(domain::BranchStrategySnapshot::new(base_branch, merge_target, adapter.merge_method()))
}

impl TrackCompositionRoot {
    /// Create a new track branch from the configured base branch.
    /// # Errors
    /// Returns `Err` when git discovery, branch strategy config resolution, or
    /// branch creation fails.
    pub fn track_branch_create(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::GitRepository;
        validate_track_id_str(&track_id)?;
        let branch_name = format!("track/{track_id}");
        let project_root = resolve_project_root(&items_dir)?;
        // No metadata.json exists yet for the track being created, so the base
        // branch is resolved from the global config (mirrors track_init).
        let snap = resolve_branch_strategy_snapshot(&project_root)?;
        let base_branch = snap.base_branch();
        let repo = infrastructure::git_cli::SystemGitRepo::discover().map_err(|e| {
            CompositionError::AdapterInit(format!("failed to discover git repository: {e}"))
        })?;
        let current = GitRepository::current_branch(&repo)
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        if current.as_deref() != Some(base_branch) {
            return Err(CompositionError::WiringFailed(format!(
                "branch create must start from '{base_branch}'; current branch is {}",
                current.as_deref().unwrap_or("<detached>")
            )));
        }
        let exists_output = repo
            .output(&["rev-parse", "--verify", "--quiet", &branch_name])
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        if exists_output.status.success() {
            return Err(CompositionError::WiringFailed(format!(
                "branch '{branch_name}' already exists"
            )));
        }
        let code = repo
            .status(&["switch", "-c", &branch_name, base_branch])
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        if code == 0 {
            Ok(CommandOutcome::success(None))
        } else {
            Err(CompositionError::Infrastructure(format!(
                "git switch -c {branch_name} {base_branch} failed"
            )))
        }
    }

    /// Switch to the base branch from the active track's `branch_strategy_snapshot` (IN-05).
    ///
    /// Reads metadata.json from `project_root`, resolves a
    /// [`infrastructure::branch_strategy::SnapshotBranchStrategyAdapter`] (CN-02: no
    /// re-read of global config for post-init operations), and runs `git switch` +
    /// `git pull` against the resolved `base_branch`.
    /// # Errors
    /// Returns `Err` when the active track cannot be resolved, its metadata cannot
    /// be read, or the underlying git operations fail.
    pub fn track_switch_base(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        use domain::TrackReader as _;
        use infrastructure::branch_strategy::SnapshotBranchStrategyAdapter;
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::branch_strategy::BranchStrategyPort as _;

        let active_track_id = resolve_track_id_inner(None, &project_root, false)?;
        let id = domain::TrackId::try_new(&active_track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;
        let items_dir = project_root.join("track").join("items");
        let store = FsTrackStore::new(items_dir);
        let metadata = store
            .find(&id)
            .map_err(|e| {
                CompositionError::Infrastructure(format!("failed to read track metadata: {e}"))
            })?
            .ok_or_else(|| {
                CompositionError::WiringFailed(format!("track '{active_track_id}' not found"))
            })?;
        let adapter =
            SnapshotBranchStrategyAdapter::new(metadata.branch_strategy_snapshot().clone());
        crate::GitCompositionRoot::new()
            .git_switch_and_pull_in(&project_root, adapter.base_branch().to_owned())
    }
}
