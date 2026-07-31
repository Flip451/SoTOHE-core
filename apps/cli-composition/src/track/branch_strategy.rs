//! Branch strategy resolution and switch-base operation for the `track` command
//! family.
//!
//! Extracted from `mod.rs` to keep the module within the production-code line
//! limit declared by `architecture-rules.json` (`module_limits.max_lines`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// Wire a fresh [`usecase::git_workflow::TrackGitInteractor`] from the standard
/// infrastructure adapters. Every track-family call site routes through this
/// factory so the port wiring is uniform.
fn build_track_git_interactor() -> usecase::git_workflow::TrackGitInteractor {
    use infrastructure::git_cli::workflow_adapter::{FsGitWorkflowAdapter, FsWorkspaceAdapter};
    use usecase::git_workflow::{GitPrimitivePort, TrackArchiveFsPort, TrackGitInteractor};

    let git: Arc<dyn GitPrimitivePort> = Arc::new(FsGitWorkflowAdapter::new());
    let fs: Arc<dyn TrackArchiveFsPort> = Arc::new(FsWorkspaceAdapter::new());
    TrackGitInteractor::new(git, fs)
}

pub(super) fn track_git_interactor() -> usecase::git_workflow::TrackGitInteractor {
    build_track_git_interactor()
}

fn switch_base_error_to_outcome(
    base_branch: &str,
    error: usecase::git_workflow::GitWorkflowError,
) -> Result<CommandOutcome, CompositionError> {
    use usecase::git_workflow::GitWorkflowError;

    // A typed `git switch` failure renders the legacy checkout-failure outcome:
    // stdout `Failed to checkout <base>`, no stderr, and the u8-clamped exit
    // code (CN-05 bit-equivalence with the pre-track inline behavior). All other
    // errors remain typed composition errors.
    if let GitWorkflowError::SwitchFailed { exit_code, .. } = &error {
        let exit_code = u8::try_from(*exit_code).unwrap_or(1);
        return Ok(CommandOutcome {
            stdout: Some(format!("Failed to checkout {base_branch}")),
            stderr: None,
            exit_code,
        });
    }

    Err(CompositionError::Infrastructure(error.to_string()))
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
        validate_track_id_str(&track_id)?;
        let project_root = resolve_project_root(&items_dir)?;
        // No metadata.json exists yet for the track being created, so the base
        // branch is resolved from the global config (mirrors track_init).
        let snap = resolve_branch_strategy_snapshot(&project_root)?;
        let base_branch = snap.base_branch();
        let id = domain::TrackId::try_new(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;
        let interactor = build_track_git_interactor();
        interactor
            .create_track_branch(&project_root, &id, base_branch)
            .map(|()| CommandOutcome::success(None))
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))
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

        let base_branch = adapter.base_branch().to_owned();
        build_track_git_interactor()
            .switch_to_base(&project_root, &base_branch)
            .map(|msg| CommandOutcome::success(Some(msg)))
            .or_else(|e| switch_base_error_to_outcome(&base_branch, e))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::switch_base_error_to_outcome;

    fn write_branch_strategy_config(root: &Path) {
        let config_dir = root.join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
    }

    #[test]
    fn test_switch_base_error_to_outcome_checkout_failure_preserves_legacy_result()
    -> Result<(), String> {
        let result = switch_base_error_to_outcome(
            "main",
            usecase::git_workflow::GitWorkflowError::SwitchFailed {
                branch: usecase::git_workflow::DiagnosticText::new("main"),
                exit_code: 7,
            },
        );
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                return Err(format!("checkout failure should map to legacy outcome: {error}"));
            }
        };

        assert_eq!(outcome.stdout.as_deref(), Some("Failed to checkout main"));
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 7);
        Ok(())
    }

    #[test]
    fn test_switch_base_error_to_outcome_unrelated_error_stays_infrastructure_error()
    -> Result<(), String> {
        let result = switch_base_error_to_outcome(
            "main",
            usecase::git_workflow::GitWorkflowError::Unavailable(
                usecase::git_workflow::DiagnosticText::new("unexpected sync failure"),
            ),
        );
        let error = match result {
            Ok(outcome) => {
                return Err(format!(
                    "unrelated git errors must remain typed composition errors: {outcome:?}"
                ));
            }
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "git workflow unavailable: unexpected sync failure");
        Ok(())
    }

    #[test]
    fn test_track_branch_create_date_prefixed_ids_create_sorted_track_branches() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "main");
        write_branch_strategy_config(root.path());
        let items_dir = root.path().join("track").join("items");
        let composition = crate::TrackCompositionRoot::new();

        for track_id in ["2026-07-31-later-track", "2026-07-01-earlier-track"] {
            let outcome =
                composition.track_branch_create(items_dir.clone(), track_id.to_owned()).unwrap();
            assert_eq!(outcome.exit_code, 0);

            let status = Command::new("git")
                .args(["switch", "main"])
                .current_dir(root.path())
                .status()
                .unwrap();
            assert!(status.success(), "must return fixture to the configured base branch");
        }

        let output = Command::new("git")
            .args(["branch", "--list", "--sort=refname", "--format=%(refname:short)", "track/*"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        let listed_branches = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        assert_eq!(
            listed_branches,
            ["track/2026-07-01-earlier-track", "track/2026-07-31-later-track",],
            "ascending branch listing must put the earlier date first"
        );
    }
}
