//! Next-task outcome rendering for [`crate::track::TrackDriver`].

use std::path::PathBuf;

use usecase::track_lifecycle::track_next_task::{
    TrackNextTaskCommand, TrackNextTaskError, TrackNextTaskResult, TrackNextTaskService,
};
use usecase::track_lifecycle::{TrackItemsDirectory, TrackLifecycleIdInput, TrackSelection};

use crate::render::CommandOutcome;

pub(crate) fn render_track_next_task_outcome(
    service: &dyn TrackNextTaskService,
    items_dir: PathBuf,
    track_id: Option<String>,
) -> CommandOutcome {
    let items_dir_for_error = items_dir.clone();
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(_) => return track_next_task_invalid_items_dir(&items_dir_for_error),
    };
    let track = match track_id
        .map(TrackLifecycleIdInput::try_new)
        .transpose()
        .map_err(|error| error.to_string())
    {
        Ok(track_id) => TrackSelection::from_input(track_id),
        Err(error) => return track_next_task_invalid_track_id(error),
    };
    service
        .execute(TrackNextTaskCommand { items_dir, track })
        .map(render_track_next_task_result)
        .unwrap_or_else(track_next_task_error_to_outcome)
}

fn render_track_next_task_result(result: TrackNextTaskResult) -> CommandOutcome {
    let payload = match result {
        TrackNextTaskResult::Found { task_id, description, status } => serde_json::json!({
            "task_id": task_id.as_ref(),
            "description": description.as_ref(),
            "status": status.to_string(),
        }),
        TrackNextTaskResult::NoOpenTask => serde_json::json!({
            "task_id": null,
            "description": null,
            "status": null,
        }),
    };
    CommandOutcome::success(Some(payload.to_string()))
}

fn track_next_task_error_to_outcome(error: TrackNextTaskError) -> CommandOutcome {
    track_next_task_failure(error)
}

fn track_next_task_failure(error: impl std::fmt::Display) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

fn track_next_task_invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    track_next_task_failure(legacy_error)
}

fn track_next_task_invalid_items_dir(items_dir: &std::path::Path) -> CommandOutcome {
    CommandOutcome::failure(Some(format!(
        "[ERROR] --items-dir must point to '<project-root>/track/items'; got {}",
        items_dir.display()
    )))
}
