//! `sotp track catalogue-impl-signals` — diagnose SoT Chain ③ (catalogue ↔ implementation).
//!
//! Thin CLI adapter: delegates all orchestration to [`cli_composition::CliApp`].

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;

use crate::CliError;

/// Execute the `track catalogue-impl-signals` command.
///
/// # Errors
///
/// Returns `CliError` when the underlying `CliApp` composition fails.
pub fn execute_catalogue_impl_signals(
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new()
        .track_catalogue_impl_signals(Some(track_id), workspace_root, layer)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Symlinked workspace_root must be rejected before any I/O.
    #[cfg(unix)]
    #[test]
    fn test_symlinked_workspace_root_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&real_dir).unwrap();
        let link_dir = tmp.path().join("link");
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let result =
            execute_catalogue_impl_signals("test-track-2026-01-01".to_owned(), link_dir, None);
        assert!(result.is_err(), "symlinked workspace_root must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("symlink guard"), "error message must mention symlink guard: {msg}");
    }

    /// An invalid track ID must be rejected by the interactor's validation.
    #[test]
    fn test_invalid_track_id_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        // No architecture-rules.json — but invalid track ID fails before file I/O.
        let result = execute_catalogue_impl_signals(
            "bad track id!!".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "invalid track ID must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("invalid track id") || msg.contains("invalid track ID"),
            "error must mention invalid track id: {msg}"
        );
    }

    /// Missing architecture-rules.json at workspace_root must produce an error
    /// (fail-closed: the layer-bindings port cannot enumerate TDDD layers without it).
    #[test]
    fn test_missing_architecture_rules_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        // workspace_root is a real directory (not a symlink) but has no architecture-rules.json.
        let result = execute_catalogue_impl_signals(
            "test-track-2026-01-01".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "missing architecture-rules.json must return Err");
    }

    /// Symlinked `track/items` directory must be rejected by the items_dir guard.
    #[cfg(unix)]
    #[test]
    fn test_symlinked_items_dir_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let real_items = tmp.path().join("real_items");
        std::fs::create_dir_all(&real_items).unwrap();
        // Create workspace_root/track/ and symlink items → real_items.
        let track_dir = tmp.path().join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let items_link = track_dir.join("items");
        std::os::unix::fs::symlink(&real_items, &items_link).unwrap();

        let result = execute_catalogue_impl_signals(
            "test-track-2026-01-01".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "symlinked track/items must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("symlink guard"), "error message must mention symlink guard: {msg}");
    }
}
