//! Scope configuration loading and validation helpers.

use std::path::Path;

use domain::TrackId;
use domain::review_v2::ReviewScopeConfig;

use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::review_v2::load_v2_scope_config;

/// Loads just the `ReviewScopeConfig` for a given track/items_dir, without
/// initialising review/hash stores or resolving the diff base.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn load_scope_config_only(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<ReviewScopeConfig, String> {
    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let root = git.root().to_path_buf();

    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize repo root {}: {e}", root.display()))?;
    let items_dir_abs =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { root.join(items_dir) };
    // Canonicalize directly: resolve all symlinks and `..` components together.
    // `normalize_path_components` + partial-walk is unsound when intermediate path
    // components are symlinks (the `..` is stripped before the symlink is resolved,
    // so traversal can escape the repo root without detection).
    // If `items_dir` does not exist on the filesystem, we cannot verify containment
    // and treat it as if it were outside the repository root (fail-closed).
    let canonical_items_dir = items_dir_abs.canonicalize().map_err(|_| {
        format!(
            "items_dir '{}' is outside the repository root '{}' or does not exist. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        )
    })?;
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(format!(
            "items_dir '{}' is outside the repository root '{}'. \
             Only paths under the repo are allowed.",
            items_dir.display(),
            canonical_root.display()
        ));
    }

    let scope_json_path = root.join("track/review-scope.json");
    load_v2_scope_config(&scope_json_path, track_id, &root)
        .map_err(|e| format!("load review-scope.json: {e}"))
}

/// String-accepting variant of `load_scope_config_only`.
///
/// Converts `track_id_str` to `TrackId` and delegates. Returns the scope config
/// for use in the CLI composition root (via `append_scope_briefing_reference`).
/// Returns `Err` if the track ID is invalid or the config cannot be loaded.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn load_scope_config_only_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewScopeConfig, String> {
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    load_scope_config_only(&track_id, items_dir)
}

/// Validates that `scope_name` is a configured scope for the given track,
/// without resolving the diff base.
///
/// Used by `sotp review files` to enforce AC-08 ordering: scope name validation
/// runs before any diff I/O. Returns `Ok(())` if the scope name is valid and
/// known, `Err(message)` otherwise.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub fn validate_scope_for_track_str(
    track_id_str: &str,
    items_dir: &Path,
    scope_name: &str,
) -> Result<(), String> {
    use domain::review_v2::{MainScopeName, ScopeName};

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;

    let scope = if scope_name.eq_ignore_ascii_case("other") {
        ScopeName::Other
    } else {
        ScopeName::Main(
            MainScopeName::new(scope_name.to_owned())
                .map_err(|e| format!("invalid scope name '{scope_name}': {e}"))?,
        )
    };

    if scope_config.contains_scope(&scope) {
        Ok(())
    } else {
        let known: Vec<String> =
            scope_config.all_scope_names().iter().map(|n| n.to_string()).collect();
        Err(format!("Unknown scope: {scope_name}. Known scopes: {}", known.join(", ")))
    }
}

/// Validates a track ID string without exposing `domain::TrackId` to the caller.
///
/// Returns `Ok(())` if the string is a valid track ID, or `Err(reason)` if not.
/// Used by the CLI composition root to pre-validate `--track-id` arguments
/// without importing `domain::TrackId` (CN-01 / AC-03).
///
/// # Errors
/// Returns a string describing why the track ID is invalid.
pub fn validate_track_id_str(track_id_str: &str) -> Result<(), String> {
    TrackId::try_new(track_id_str).map(|_| ()).map_err(|e| e.to_string())
}

/// Validates a review group name string without exposing domain types to the caller.
///
/// Returns `Ok(())` if the string is a valid `ReviewGroupName`, or `Err(reason)`.
/// Used by the CLI composition root to pre-validate `--group` arguments
/// without importing `domain::ReviewGroupName` (CN-01 / AC-03).
///
/// # Errors
/// Returns a string describing why the group name is invalid.
pub fn validate_review_group_name_str(group_name: &str) -> Result<(), String> {
    domain::ReviewGroupName::try_new(group_name).map(|_| ()).map_err(|e| e.to_string())
}
