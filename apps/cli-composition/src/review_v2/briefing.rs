//! Scope briefing reference helpers.

use std::path::Path;

use domain::TrackId;

use super::scope::load_scope_config_only;

/// Appends a scope-specific severity policy reference section to `prompt`
/// when the given scope has a `briefing_file` configured and the path is safe
/// to inject.
///
/// This is the string-accepting variant of the CLI `append_scope_briefing_reference`
/// helper. It loads scope config from the given track and items_dir, then checks
/// the configured briefing file for `scope_name`. No I/O beyond config loading.
///
/// String-accepting so the CLI never imports `domain::ScopeName` or
/// `domain::ReviewScopeConfig` (CN-01 / AC-03).
///
/// # Errors
/// Returns an error string if the track ID is invalid or the scope config
/// cannot be loaded.
pub fn append_scope_briefing_reference_str(
    prompt: &mut String,
    scope_name: &str,
    track_id_str: &str,
    items_dir: &Path,
    is_safe_path_fn: impl Fn(&str) -> bool,
) -> Result<(), String> {
    use domain::review_v2::{MainScopeName, ScopeName};

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;

    let scope = if scope_name == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(scope_name) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => {
                return Err(format!("invalid scope name '{scope_name}': {e}"));
            }
        }
    };

    let Some(briefing_path) = scope_config.briefing_file_for_scope(&scope) else {
        return Ok(());
    };

    if !is_safe_path_fn(briefing_path) {
        return Ok(());
    }

    prompt.push_str("\n\n## Scope-specific severity policy\n\n");
    prompt.push_str(&format!(
        "このレビューの scope は `{scope_name}` である。\
         以下の scope 固有 severity policy を **必ず先に Read ツールで読み込み**、\
         その方針に従って findings を選別すること:\n\n\
         - `{briefing_path}`",
    ));

    Ok(())
}

/// Returns the configured briefing file path (as `Option<String>`) for the
/// given scope in the given track's scope configuration.
///
/// This lets the CLI layer query whether a scope has a briefing configured
/// without importing `domain::review_v2::ScopeName` or `ReviewScopeConfig`
/// (CN-01 / AC-03).
///
/// Returns `Ok(None)` when the scope has no briefing configured or the scope
/// name is "other" (the implicit scope never receives scope-specific briefings).
/// Returns `Err` if the track ID or scope config cannot be loaded.
///
/// # Errors
/// Returns a human-readable error string on failure.
pub(crate) fn get_briefing_for_scope_str(
    scope_name: &str,
    track_id_str: &str,
    items_dir: &Path,
) -> Result<Option<String>, String> {
    use domain::review_v2::{MainScopeName, ScopeName};

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let scope_config = load_scope_config_only(&track_id, items_dir)?;

    let scope = if scope_name == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(scope_name) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => {
                return Err(format!("invalid scope name '{scope_name}': {e}"));
            }
        }
    };

    Ok(scope_config.briefing_file_for_scope(&scope).map(str::to_owned))
}
