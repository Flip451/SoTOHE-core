use domain::ImplPlanReader;

use crate::CliError;

use super::*;

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

    let track_id = TrackId::try_new(&effective_track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let (track, meta) = read_track_metadata(&items_dir, &track_id)
        .map_err(|err| CliError::Message(format!("resolve failed: {err}")))?;

    // Load impl-plan.json via FsTrackStore::load_impl_plan (fail-closed):
    // - Absent file → Ok(None) — valid for planning-only tracks (no branch, no plan).
    // - Present but corrupt/unreadable → Err — propagated as a hard failure.
    // Planning-only tracks (no branch, no plan) receive None and are reported correctly.
    let store = FsTrackStore::new(items_dir.clone());
    let impl_plan = store
        .load_impl_plan(&track_id)
        .map_err(|err| CliError::Message(format!("resolve failed: {err}")))?;

    // Note: TrackStatus::Archived is not reachable from derive_track_status();
    // archived tracks live under track/archive/ and are not resolved by this command.
    let info = domain::track_phase::resolve_phase(&track, meta.schema_version, impl_plan.as_ref());

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

/// Read-only metadata load via `infrastructure::track::codec`.
pub(super) fn read_track_metadata(
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<(domain::TrackMetadata, DocumentMeta), domain::RepositoryError> {
    infrastructure::track::fs_store::read_track_metadata(items_dir, track_id)
}
