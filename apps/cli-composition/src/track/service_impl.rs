//! `TrackServiceImpl` — implementation of `usecase::track_service::TrackService`
//! for use in the `cli_composition` factory.
//!
//! Delegates each method to the corresponding [`TrackCompositionRoot`] method,
//! converting `Result<CommandOutcome, CompositionError>` → `TrackCommandOutput`.

use std::path::PathBuf;

use usecase::track_service::{TrackCommandOutput, TrackService};

use super::composition_root::TrackCompositionRoot;

fn composition_to_output(
    result: Result<crate::CommandOutcome, crate::error::CompositionError>,
) -> TrackCommandOutput {
    match result {
        Ok(outcome) => TrackCommandOutput {
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            exit_code: outcome.exit_code,
        },
        Err(e) => TrackCommandOutput::failure(Some(format!("[ERROR] {e}"))),
    }
}

/// Implementation of [`TrackService`] that delegates to [`TrackCompositionRoot`].
///
/// Constructed by the `track_driver()` factory in `TrackCompositionRoot`.
pub struct TrackServiceImpl;

impl TrackService for TrackServiceImpl {
    fn init(
        &self,
        items_dir: PathBuf,
        track_id: String,
        description: String,
    ) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_init(
            items_dir,
            track_id,
            description,
        ))
    }

    fn transition(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        task_id: String,
        target_status: String,
        commit_hash: Option<String>,
    ) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_transition(
            items_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
        ))
    }

    fn resolve(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_resolve(items_dir, track_id))
    }

    fn branch_create(&self, items_dir: PathBuf, track_id: String) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_branch_create(items_dir, track_id))
    }

    fn branch_switch(&self, items_dir: PathBuf, track_id: String) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_branch_switch(items_dir, track_id))
    }

    fn views_validate(&self, project_root: PathBuf) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_views_validate(project_root))
    }

    fn views_sync(&self, project_root: PathBuf, track_id: Option<String>) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_views_sync(project_root, track_id))
    }

    fn add_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> TrackCommandOutput {
        // When track_id is already resolved (Some), skip re-resolution via the
        // pre-resolved variant to avoid a redundant git branch check at the
        // composition boundary.
        match track_id {
            Some(id) => composition_to_output(TrackCompositionRoot::new().track_add_task_resolved(
                items_dir,
                id,
                description,
                section,
                after,
            )),
            None => composition_to_output(TrackCompositionRoot::new().track_add_task(
                items_dir,
                None,
                description,
                section,
                after,
            )),
        }
    }

    fn set_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        status: String,
        reason: String,
    ) -> TrackCommandOutput {
        match track_id {
            Some(id) => composition_to_output(
                TrackCompositionRoot::new()
                    .track_set_override_resolved(items_dir, id, status, reason),
            ),
            None => composition_to_output(
                TrackCompositionRoot::new().track_set_override(items_dir, None, status, reason),
            ),
        }
    }

    fn clear_override(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput {
        match track_id {
            Some(id) => composition_to_output(
                TrackCompositionRoot::new().track_clear_override_resolved(items_dir, id),
            ),
            None => composition_to_output(
                TrackCompositionRoot::new().track_clear_override(items_dir, None),
            ),
        }
    }

    fn next_task(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput {
        match track_id {
            Some(id) => composition_to_output(
                TrackCompositionRoot::new().track_next_task_resolved(items_dir, id),
            ),
            None => {
                composition_to_output(TrackCompositionRoot::new().track_next_task(items_dir, None))
            }
        }
    }

    fn task_counts(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput {
        match track_id {
            Some(id) => composition_to_output(
                TrackCompositionRoot::new().track_task_counts_resolved(items_dir, id),
            ),
            None => composition_to_output(
                TrackCompositionRoot::new().track_task_counts(items_dir, None),
            ),
        }
    }

    fn archive(&self, items_dir: PathBuf, track_id: String) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().track_archive(items_dir, track_id))
    }

    fn detect_active(&self, project_root: PathBuf) -> TrackCommandOutput {
        let active = TrackCompositionRoot::new().detect_active_track_from_branch(&project_root);
        match active {
            Some(id) => TrackCommandOutput::success(Some(id)),
            None => TrackCommandOutput { stdout: Some(String::new()), stderr: None, exit_code: 0 },
        }
    }

    fn catalogue_lint_check_active_track(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        rules_file: Option<PathBuf>,
    ) -> TrackCommandOutput {
        composition_to_output(TrackCompositionRoot::new().catalogue_lint_check_active_track(
            track_id,
            workspace_root,
            rules_file,
        ))
    }
}
