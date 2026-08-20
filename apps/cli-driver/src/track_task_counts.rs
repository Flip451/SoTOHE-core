//! Task-counts outcome rendering for [`crate::track::TrackDriver`].

use std::path::PathBuf;

use usecase::track_lifecycle::track_task_counts::{
    TrackTaskCountsCommand, TrackTaskCountsError, TrackTaskCountsResult, TrackTaskCountsService,
};
use usecase::track_lifecycle::{TrackItemsDirectory, TrackLifecycleIdInput, TrackSelection};

use crate::render::CommandOutcome;

/// Render a task-counts command through the injected application service.
pub(crate) fn render_track_task_counts_outcome(
    service: &dyn TrackTaskCountsService,
    items_dir: PathBuf,
    track_id: Option<String>,
) -> CommandOutcome {
    let items_dir_for_error = items_dir.clone();
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(_) => return track_task_counts_invalid_items_dir(&items_dir_for_error),
    };
    let track = match track_id
        .map(TrackLifecycleIdInput::try_new)
        .transpose()
        .map_err(|error| error.to_string())
    {
        Ok(track_id) => TrackSelection::from_input(track_id),
        Err(error) => return track_task_counts_invalid_track_id(error),
    };
    service
        .execute(TrackTaskCountsCommand { items_dir, track })
        .map(render_track_task_counts_result)
        .unwrap_or_else(track_task_counts_error_to_outcome)
}

fn render_track_task_counts_result(result: TrackTaskCountsResult) -> CommandOutcome {
    let json = format!(
        r#"{{"total":{},"todo":{},"in_progress":{},"done":{},"skipped":{}}}"#,
        result.total.value(),
        result.todo.value(),
        result.in_progress.value(),
        result.done.value(),
        result.skipped.value()
    );
    CommandOutcome::success(Some(json))
}

fn track_task_counts_error_to_outcome(error: TrackTaskCountsError) -> CommandOutcome {
    track_task_counts_failure(error)
}

fn track_task_counts_failure(error: impl std::fmt::Display) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

fn track_task_counts_invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    track_task_counts_failure(legacy_error)
}

fn track_task_counts_invalid_items_dir(items_dir: &std::path::Path) -> CommandOutcome {
    CommandOutcome::failure(Some(format!(
        "[ERROR] --items-dir must point to '<project-root>/track/items'; got {}",
        items_dir.display()
    )))
}
