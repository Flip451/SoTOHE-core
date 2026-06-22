use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;

use crate::CliError;

pub(super) fn execute_transition(
    items_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
) -> Result<ExitCode, CliError> {
    let app = TrackCompositionRoot::new();
    let outcome = app
        .track_transition(items_dir, Some(track_id), task_id, target_status, commit_hash)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}
