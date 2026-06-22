//! Validation helpers and track ID resolution wrappers for track subcommands.

use std::path::{Path, PathBuf};

use cli_composition::TrackCompositionRoot;

/// Validates a track ID string by delegating to the canonical domain rule via
/// cli_composition.
///
/// # Errors
///
/// Returns an error string describing the failure.
pub(crate) fn validate_track_id_str(value: &str) -> Result<(), String> {
    TrackCompositionRoot::new().track_validate_id(value)
}

/// Validates a track branch name string (`track/<valid-track-id>`).
///
/// Mirrors the validation performed by `domain::TrackBranch::try_new` without
/// importing domain types.
///
/// # Errors
///
/// Returns an error string describing the failure.
pub(crate) fn validate_track_branch_str(value: &str) -> Result<(), String> {
    match value.strip_prefix("track/") {
        Some(slug) => validate_track_id_str(slug)
            .map_err(|_| format!("invalid track branch: '{value}' (slug part is invalid)")),
        None => Err(format!("invalid track branch: '{value}' (must be in 'track/<id>' form)")),
    }
}

// ---------------------------------------------------------------------------
// Track ID resolution — delegated to CliApp (CN-04: cli is thin composition root)
// ---------------------------------------------------------------------------

/// Resolves a track ID for a READ operation, anchored to the repository that
/// owns `items_dir`.
///
/// When `explicit_id` is `Some`, it is returned as-is.
/// When `None`, the current branch is used to derive the track ID.
/// Fail-closed on non-track branches (CN-01, AC-01, AC-02).
///
/// # Errors
///
/// Returns a human-readable error string on failure.
pub(crate) fn resolve_track_id(
    explicit_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    TrackCompositionRoot::new().track_resolve_id(explicit_id, items_dir.to_path_buf())
}

/// Resolves a track ID for a READ operation, anchored to `workspace_root`.
///
/// # Errors
///
/// Returns a human-readable error string on failure.
pub(crate) fn resolve_track_id_from_root(
    explicit_id: Option<String>,
    workspace_root: &Path,
) -> Result<String, String> {
    TrackCompositionRoot::new()
        .track_resolve_id_from_root(explicit_id, workspace_root.to_path_buf())
}

/// Resolves a track ID for a WRITE operation, anchored to `items_dir`.
///
/// The git branch is always read; `explicit_id` (if `Some`) must match the
/// branch-derived id (D7, AC-18, CN-02, CN-03). Fail-closed on mismatch or
/// on non-track branches.
///
/// # Errors
///
/// Returns a human-readable error string on failure.
pub(crate) fn resolve_track_id_for_write(
    explicit_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    TrackCompositionRoot::new().track_resolve_id_for_write(explicit_id, items_dir.to_path_buf())
}

/// Resolves a track ID for a WRITE operation, anchored to `workspace_root`.
///
/// # Errors
///
/// Returns a human-readable error string on failure.
pub(crate) fn resolve_track_id_from_root_for_write(
    explicit_id: Option<String>,
    workspace_root: &Path,
) -> Result<String, String> {
    TrackCompositionRoot::new()
        .track_resolve_id_from_root_for_write(explicit_id, workspace_root.to_path_buf())
}

pub(crate) fn resolve_project_root(items_dir: &std::path::Path) -> Result<PathBuf, String> {
    let items_name = items_dir.file_name().and_then(|name| name.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(std::path::Path::file_name).and_then(|name| name.to_str());
    let project_root = track_dir.and_then(std::path::Path::parent);

    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            // When items_dir is a bare relative path like "track/items", Path::parent()
            // returns an empty path ("") rather than ".".  An empty path passed to
            // Command::current_dir causes ENOENT on spawn (e.g. in render.rs's git
            // branch discovery).  Normalise the empty root to "." so all callers get
            // a usable current-directory path, consistent with how relative joins
            // elsewhere in the render pipeline behave.
            if root.as_os_str().is_empty() {
                Ok(PathBuf::from("."))
            } else {
                Ok(root.to_path_buf())
            }
        }
        _ => Err(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}
