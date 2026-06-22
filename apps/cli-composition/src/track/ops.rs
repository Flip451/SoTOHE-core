//! `TrackCompositionRoot` methods for task/state operations that accept a pre-resolved track ID,
//! and miscellaneous helpers for views/branch auto-detection.
//!
//! The task-state methods skip git-based track ID resolution.  The caller is
//! responsible for validating and resolving the track ID (e.g. via
//! `track_resolve_id_for_write`) before invoking these functions.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::CommandOutcome;
use crate::error::CompositionError;
use crate::track::composition_root::TrackCompositionRoot;

use super::{
    build_branch_reader, resolve_project_root, sync_views_to_stdout, validate_track_id_str,
};

impl TrackCompositionRoot {
    /// Add a new task to a track using a pre-resolved `track_id`.
    ///
    /// Unlike `track_add_task`, this method does **not** perform git-based ID
    /// resolution.  The caller must supply a validated, already-resolved track ID.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_add_task_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;

        validate_track_id_str(&track_id).map_err(CompositionError::WiringFailed)?;

        let project_root =
            resolve_project_root(&items_dir).map_err(CompositionError::WiringFailed)?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service =
            usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

        let after_task_id = match after {
            Some(ref a)
                if a.strip_prefix('T').is_some_and(|digits| {
                    !digits.is_empty()
                        && digits.chars().all(|ch| ch.is_ascii_digit())
                        && digits.parse::<u64>().is_ok()
                }) =>
            {
                after
            }
            Some(ref a) => {
                return Err(CompositionError::WiringFailed(format!(
                    "invalid --after value {a:?}: expected T<digits> (e.g. T001)"
                )));
            }
            None => None,
        };

        let cmd = usecase::task_ops::AddTaskCommand {
            items_dir,
            track_id: track_id.clone(),
            description: description.clone(),
            section,
            after_task_id,
        };
        let output = service
            .add_task(cmd)
            .map_err(|e| CompositionError::Usecase(format!("add-task failed: {e}")))?;

        let new_task_id = output.task_id.as_deref().unwrap_or("?");
        let mut lines = vec![format!(
            "[OK] Added task {new_task_id}: {description} (track status: {})",
            output.derived_status
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Set a status override on a track using a pre-resolved `track_id`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_set_override_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
        status: String,
        reason: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;

        validate_track_id_str(&track_id).map_err(CompositionError::WiringFailed)?;

        let project_root =
            resolve_project_root(&items_dir).map_err(CompositionError::WiringFailed)?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service =
            usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

        let cmd = usecase::task_ops::SetOverrideCommand {
            items_dir,
            track_id: track_id.clone(),
            status: status.clone(),
            reason,
        };
        let output = service
            .set_override(cmd)
            .map_err(|e| CompositionError::Usecase(format!("set-override failed: {e}")))?;

        let mut lines = vec![format!(
            "[OK] Override set to '{}' (track status: {})",
            status, output.derived_status
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Clear a status override on a track using a pre-resolved `track_id`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_clear_override_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;

        validate_track_id_str(&track_id).map_err(CompositionError::WiringFailed)?;

        let project_root =
            resolve_project_root(&items_dir).map_err(CompositionError::WiringFailed)?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service =
            usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

        let cmd = usecase::task_ops::ClearOverrideCommand { items_dir, track_id: track_id.clone() };
        let output = service
            .clear_override(cmd)
            .map_err(|e| CompositionError::Usecase(format!("clear-override failed: {e}")))?;

        let mut lines =
            vec![format!("[OK] Override cleared (track status: {})", output.derived_status)];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Show the next open task using a pre-resolved `track_id` (JSON output).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_next_task_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskQueryService as _;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let service = usecase::task_ops::TaskQueryInteractor::new(Arc::clone(&store));

        let next = service
            .next_task(track_id.clone(), items_dir.clone())
            .map_err(|e| CompositionError::Usecase(format!("next-task failed: {e}")))?;

        let payload = match next {
            Some(task) => {
                let counts = service.task_counts(track_id, items_dir).map_err(|e| {
                    CompositionError::Usecase(format!("next-task failed (counts): {e}"))
                })?;
                let task_status = if counts.in_progress > 0 { "in_progress" } else { "todo" };
                serde_json::json!({
                    "task_id": task.task_id,
                    "description": task.description,
                    "status": task_status,
                })
            }
            None => {
                serde_json::json!({
                    "task_id": null,
                    "description": null,
                    "status": null,
                })
            }
        };

        Ok(CommandOutcome::success(Some(payload.to_string())))
    }

    /// Show task status counts using a pre-resolved `track_id` (JSON output).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_task_counts_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskQueryService as _;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let service = usecase::task_ops::TaskQueryInteractor::new(Arc::clone(&store));

        let counts = service
            .task_counts(track_id, items_dir)
            .map_err(|e| CompositionError::Usecase(format!("task-counts failed: {e}")))?;

        let total = counts.todo + counts.in_progress + counts.done + counts.skipped;
        let json = format!(
            r#"{{"total":{total},"todo":{},"in_progress":{},"done":{},"skipped":{}}}"#,
            counts.todo, counts.in_progress, counts.done, counts.skipped
        );

        Ok(CommandOutcome::success(Some(json)))
    }

    /// Detect the active track ID from the current git branch.
    ///
    /// Only `track/<id>` branches are resolved; any other branch (e.g. `main`,
    /// detached HEAD) or git failure resolves to `None` so the caller can fall
    /// back to registry-only mode without surfacing an error.
    ///
    /// Uses `project_root` for git discovery so that auto-detection is consistent
    /// with `--project-root` invocations and does not depend on the process CWD.
    pub fn detect_active_track_from_branch(&self, project_root: &Path) -> Option<String> {
        use usecase::track_resolution::{
            ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
        };
        let repo = infrastructure::git_cli::SystemGitRepo::discover_from(project_root).ok()?;
        let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
        interactor.resolve_active_track().ok()
    }
}
