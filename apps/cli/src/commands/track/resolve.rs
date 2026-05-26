use crate::CliError;

use super::*;
use usecase::track_phase::TrackPhaseService as _;

pub(super) fn execute_resolve(args: ResolveArgs) -> Result<ExitCode, CliError> {
    let ResolveArgs { items_dir, track_id } = args;

    // Validate items_dir structure (must be <root>/track/items).
    resolve_project_root(&items_dir).map_err(CliError::Message)?;

    // Auto-detect is only safe when items_dir is the default (track/items
    // relative to CWD), because auto_detect uses SystemGitRepo::discover
    // from CWD.  When a custom --items-dir is supplied, require explicit id.
    let is_default_items_dir = items_dir == std::path::Path::new("track/items");

    let effective_track_id = match track_id {
        Some(id) => id,
        None if !is_default_items_dir => {
            return Err(CliError::Message(
                "resolve failed: custom --items-dir requires an explicit track-id argument"
                    .to_owned(),
            ));
        }
        None => auto_detect_track_id_from_branch()
            .map_err(|err| CliError::Message(format!("resolve failed: {err}")))?,
    };

    // Validate the track ID before any filesystem probing.
    // `items_dir.join(track_id)` would otherwise let a caller traverse outside
    // `track/items` with values like `..` or absolute paths.
    super::validate_track_id_str(&effective_track_id)
        .map_err(|err| CliError::Message(format!("resolve failed: invalid track id: {err}")))?;

    // Use TrackPhaseInteractor (usecase service) to resolve the phase.
    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let service = usecase::track_phase::TrackPhaseInteractor::new(Arc::clone(&store));

    let info = service
        .resolve(effective_track_id, items_dir)
        .map_err(|err| CliError::Message(format!("resolve failed: {err}")))?;

    println!("Current phase: {}", info.phase);
    println!("Reason: {}", info.reason);
    println!("Recommended next command: {}", info.next_command);
    if let Some(blocker) = &info.blocker {
        println!("Blocker: {blocker}");
    }

    Ok(ExitCode::SUCCESS)
}

/// Auto-detect track ID from the current git branch.
///
/// Git I/O stays here in the CLI layer; pure branch-name parsing is
/// delegated to `usecase::track_resolution::resolve_track_id_from_branch`.
fn auto_detect_track_id_from_branch() -> Result<String, String> {
    let repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;
    let branch = repo.current_branch().map_err(|e| e.to_string())?;
    usecase::track_resolution::resolve_track_id_from_branch(branch.as_deref())
        .map_err(|e| e.to_string())
}
