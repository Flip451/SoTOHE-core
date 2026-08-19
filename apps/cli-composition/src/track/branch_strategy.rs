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

use super::{resolve_project_root, validate_track_id_str};

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
    use infrastructure::track::FsTrackBranchStrategyAdapter;
    use usecase::track_lifecycle::{TrackBranchStrategyPort, TrackItemsDirectory};

    let items_dir = TrackItemsDirectory::try_new(project_root.join("track/items"))
        .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
    FsTrackBranchStrategyAdapter
        .global_for_items(&items_dir)
        .map_err(|error| CompositionError::WiringFailed(error.to_string()))
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use cli_driver::track::TrackInput;

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

    fn write_track_metadata(root: &Path, track_id: &str, base_branch: &str) {
        let track_dir = root.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{
  "schema_version": 6,
  "id": "{track_id}",
  "branch": null,
  "title": "Switch Base Track",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "branch_strategy_snapshot": {{
    "base_branch": "{base_branch}",
    "merge_target": "main",
    "merge_method": "merge"
  }}
}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_track_switch_base_call_site_preserves_cli_contract_across_migration() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "track/active-track");
        write_branch_strategy_config(root.path());
        write_track_metadata(root.path(), "active-track", "main");
        let create_main =
            Command::new("git").args(["branch", "main"]).current_dir(root.path()).status().unwrap();
        assert!(create_main.success(), "fixture must have a main branch");

        let argv_project_root = root.path().to_path_buf();
        let outcome = crate::TrackCompositionRoot::new()
            .track_driver()
            .handle(TrackInput::SwitchBase { project_root: argv_project_root.clone() });

        assert_eq!(outcome.exit_code, 0, "stderr={:?}", outcome.stderr);
        assert_eq!(argv_project_root, root.path());
        let stdout = outcome.stdout.as_deref().unwrap_or("");
        assert!(stdout.contains("Switching to main..."), "stdout={stdout}");
        assert!(
            stdout.contains("[WARN] Pull failed (may not have remote tracking branch)")
                || stdout.contains("[OK] On main, up to date."),
            "stdout={stdout}"
        );
        assert_eq!(outcome.stderr, None);

        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(branch.stdout).unwrap().trim(), "main");
    }

    #[test]
    fn test_track_switch_base_checkout_failure_preserves_legacy_cli_contract() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "track/active-track");
        write_track_metadata(root.path(), "active-track", "missing-base");

        let argv_project_root = root.path().to_path_buf();
        let outcome = crate::TrackCompositionRoot::new()
            .track_driver()
            .handle(TrackInput::SwitchBase { project_root: argv_project_root.clone() });

        assert_eq!(argv_project_root, root.path());
        assert_eq!(outcome.stdout.as_deref(), Some("Failed to checkout missing-base"));
        assert_eq!(outcome.stderr, None);
        assert_ne!(outcome.exit_code, 0);
        assert!(
            !outcome.stdout.as_deref().unwrap_or("").contains("[ERROR]"),
            "composition must not template driver presentation"
        );
    }

    #[test]
    fn test_track_branch_create_date_prefixed_ids_create_sorted_track_branches() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "main");
        write_branch_strategy_config(root.path());
        let items_dir = root.path().join("track").join("items");
        let composition = crate::TrackCompositionRoot::new();

        for track_id in ["2026-07-31-later-track", "2026-07-01-earlier-track"] {
            let outcome = composition.track_driver().handle(TrackInput::BranchCreate {
                items_dir: items_dir.clone(),
                track_id: track_id.to_owned(),
            });
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

    #[test]
    fn test_track_branch_create_suffix_form_id_preserves_branch_name() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "main");
        write_branch_strategy_config(root.path());
        let items_dir = root.path().join("track").join("items");
        let track_id = "legacy-suffix-track-2026-07-31";

        let outcome = crate::TrackCompositionRoot::new()
            .track_driver()
            .handle(TrackInput::BranchCreate { items_dir, track_id: track_id.to_owned() });

        assert_eq!(outcome.exit_code, 0);
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(branch.status.success());
        assert_eq!(
            String::from_utf8(branch.stdout).unwrap().trim(),
            format!("track/{track_id}"),
            "a suffix-form ID must remain unchanged in its track branch"
        );
    }
}
