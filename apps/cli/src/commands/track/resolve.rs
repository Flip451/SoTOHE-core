use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;

use crate::CliError;

use super::{ResolveArgs, resolve_project_root, resolve_track_id, validate_track_id_str};

pub(super) fn execute_resolve(args: ResolveArgs) -> Result<ExitCode, CliError> {
    let ResolveArgs { items_dir, track_id } = args;

    // Validate items_dir structure (must be <root>/track/items) unconditionally,
    // even when track_id is explicitly provided (resolve_track_id only calls
    // resolve_project_root when explicit_id is None).
    resolve_project_root(&items_dir).map_err(CliError::Message)?;

    // Delegate to resolve_track_id which anchors git discovery to the repository
    // owning items_dir (via resolve_project_root). Explicit id short-circuits git
    // discovery (CN-02 / AC-19).
    let effective_track_id = resolve_track_id(track_id, &items_dir)
        .map_err(|err| CliError::Message(format!("resolve failed: {err}")))?;

    // Validate the track ID before any filesystem probing.
    // `items_dir.join(track_id)` would otherwise let a caller traverse outside
    // `track/items` with values like `..` or absolute paths.
    validate_track_id_str(&effective_track_id)
        .map_err(|err| CliError::Message(format!("resolve failed: invalid track id: {err}")))?;

    let app = TrackCompositionRoot::new();
    let outcome = app
        .track_resolve(items_dir, Some(effective_track_id))
        .map_err(|err| CliError::Message(format!("resolve failed: {err}")))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}
