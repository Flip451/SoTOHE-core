use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::track::TrackInput;

use crate::CliError;

use super::state_ops::track_driver_outcome_to_result;

pub(super) fn execute_transition(
    items_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new().track_driver().handle(TrackInput::Transition {
        items_dir,
        track_id: Some(track_id),
        task_id,
        target_status,
        commit_hash,
    });
    track_driver_outcome_to_result(outcome)
}
