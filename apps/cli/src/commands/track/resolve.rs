use super::*;

pub(super) fn execute_resolve(args: ResolveArgs) -> ExitCode {
    let ResolveArgs { items_dir, track_id } = args;

    // Validate items_dir structure (must be <root>/track/items).
    if let Err(err) = resolve_project_root(&items_dir) {
        eprintln!("{err}");
        return ExitCode::FAILURE;
    }

    // Auto-detect is only safe when items_dir is the default (track/items
    // relative to CWD), because auto_detect uses SystemGitRepo::discover
    // from CWD.  When a custom --items-dir is supplied, require explicit id.
    let is_default_items_dir = items_dir == std::path::Path::new("track/items");

    let effective_track_id = match track_id {
        Some(id) => id,
        None if !is_default_items_dir => {
            eprintln!("resolve failed: custom --items-dir requires an explicit track-id argument");
            return ExitCode::FAILURE;
        }
        None => match auto_detect_track_id_from_branch() {
            Ok(id) => id,
            Err(err) => {
                eprintln!("resolve failed: {err}");
                return ExitCode::FAILURE;
            }
        },
    };

    let track_id = match TrackId::new(&effective_track_id) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("invalid track id: {err}");
            return ExitCode::FAILURE;
        }
    };

    let (track, meta) = match read_track_metadata(&items_dir, &track_id) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("resolve failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    // Fail-closed: reject branchless v3 tracks that violate planning-only invariants.
    // Both raw status (from JSON) and domain-derived status (from tasks) must be planned.
    if meta.schema_version == 3 && track.branch().is_none() {
        let raw = meta.original_status.as_deref();
        let derived = track.status();
        if raw != Some("planned") {
            eprintln!(
                "resolve failed: track '{track_id}' is branchless v3 but raw status is '{}', \
                 not planned; metadata may be corrupt",
                raw.unwrap_or("(missing)")
            );
            return ExitCode::FAILURE;
        }
        if derived != domain::TrackStatus::Planned {
            eprintln!(
                "resolve failed: track '{track_id}' is branchless v3 but derived status is \
                 '{derived}', not planned; metadata may be corrupt"
            );
            return ExitCode::FAILURE;
        }
    }

    // Note: TrackStatus::Archived is not reachable from domain-derived status();
    // archived tracks live under track/archive/ and are not resolved by this command.
    let info = domain::track_phase::resolve_phase(&track, meta.schema_version);

    println!("Current phase: {}", info.phase);
    println!("Reason: {}", info.reason);
    println!("Recommended next command: {}", info.next_command);
    if let Some(blocker) = &info.blocker {
        println!("Blocker: {blocker}");
    }

    ExitCode::SUCCESS
}

/// Auto-detect track ID from the current git branch.
///
/// Git I/O stays here in the CLI layer; pure branch-name parsing is
/// delegated to `usecase::track_resolution::resolve_track_id_from_branch`.
fn auto_detect_track_id_from_branch() -> Result<String, String> {
    let repo = SystemGitRepo::discover()?;
    let branch = repo.current_branch()?;
    usecase::track_resolution::resolve_track_id_from_branch(branch.as_deref())
}

/// Read-only metadata load via `infrastructure::track::codec`.
///
/// Uses the codec directly to avoid constructing a `FsFileLockManager`
/// (which eagerly creates the `.locks` directory as a side effect).
pub(super) fn read_track_metadata(
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<(domain::TrackMetadata, DocumentMeta), String> {
    infrastructure::track::fs_store::read_track_metadata(items_dir, track_id)
}
