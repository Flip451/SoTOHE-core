//! `sotp track lint` subcommand — runs catalogue lint rules against a layer catalogue.
//!
//! Thin CLI adapter: delegates all orchestration to [`cli_composition::CliApp`].

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;

use crate::CliError;

/// Execute the `sotp track lint` subcommand.
///
/// # Errors
///
/// Returns `CliError::Message` when the underlying `CliApp` composition fails.
pub fn execute_lint(
    workspace_root: PathBuf,
    track_id: String,
    layer_id: String,
    rules_file: Option<PathBuf>,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new()
        .track_lint(Some(track_id), layer_id, workspace_root, rules_file)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    if let Some(ref s) = outcome.stderr {
        eprintln!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_lint_rejects_invalid_track_id() {
        let dir = tempfile::tempdir().unwrap();
        // Write minimal architecture-rules.json so the loader can start up.
        let rules = r#"{"layers":[],"canonical_modules":[]}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules).unwrap();

        let result =
            execute_lint(dir.path().to_path_buf(), "../evil".to_owned(), "domain".to_owned(), None);
        assert!(result.is_err(), "path traversal track id must be rejected");
    }
}
