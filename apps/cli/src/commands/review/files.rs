//! `sotp review files` — per-scope diff file enumerator.
//!
//! Validates the requested scope name against the configured scope universe
//! **before** any diff I/O (AC-08). On valid scope, wires `ScopeQueryInteractor`
//! and prints one file path per line to stdout (CN-04 / IN-06 / AC-07).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

#[cfg(test)]
use usecase::review_v2::ScopeQueryError;

/// CLI arguments for `sotp review files`.
#[derive(Debug, Args)]
pub struct FilesArgs {
    /// Track ID (used to expand `<track-id>` placeholders in scope patterns).
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub(super) track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub(super) items_dir: PathBuf,

    /// Scope name to enumerate (`other` for the implicit unmatched scope).
    #[arg(long)]
    pub(super) scope: String,
}

pub(super) fn execute_files(args: &FilesArgs) -> ExitCode {
    match run_files(args) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_files(args: &FilesArgs) -> Result<String, String> {
    // READ resolution: anchors git discovery to the repo owning args.items_dir so that
    // track ID resolution uses the same repository as the scope config (AC-19 / CN-02).
    let track_id =
        crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir)?;

    let outcome = cli_composition::ReviewCompositionRoot::new()
        .review_files(args.scope.clone(), Some(track_id), args.items_dir.clone())
        .map_err(|e| e.to_string())?;
    outcome.stdout.ok_or_else(|| "review files returned no output".to_owned())
}

#[cfg(test)]
fn format_files_error(err: ScopeQueryError) -> String {
    match err {
        ScopeQueryError::DiffGet(inner) => format!("diff getter failed: {inner}"),
        ScopeQueryError::UnknownScope(scope) => {
            format!("Unknown scope: {scope}")
        }
        ScopeQueryError::InvalidPath { path, reason } => {
            format!("invalid path '{path}': {reason}")
        }
        ScopeQueryError::InvalidScopeName { name, reason } => {
            format!("invalid scope name '{name}': {reason}")
        }
    }
}

#[cfg(test)]
fn render_files(files: &[String]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for file in files {
        let _ = writeln!(out, "{file}");
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ── format_files_error ──────────────────────────────────────────

    #[test]
    fn test_format_files_error_diff_get_returns_human_readable() {
        let err =
            ScopeQueryError::DiffGet(usecase::review_v2::DiffGetError::Failed("boom".to_owned()));
        let msg = format_files_error(err);
        assert!(msg.starts_with("diff getter failed: "));
        assert!(msg.contains("boom"));
    }

    #[test]
    fn test_format_files_error_invalid_scope_name_shows_name() {
        let err = ScopeQueryError::InvalidScopeName {
            name: "ghost".to_owned(),
            reason: "format check".to_owned(),
        };
        let msg = format_files_error(err);
        assert!(msg.contains("invalid scope name"));
        assert!(msg.contains("ghost"));
    }

    #[test]
    fn test_format_files_error_invalid_scope_name_returns_message() {
        let err = ScopeQueryError::InvalidScopeName {
            name: "".to_owned(),
            reason: "empty name".to_owned(),
        };
        let msg = format_files_error(err);
        assert!(msg.contains("invalid scope name"));
    }

    // ── render_files ────────────────────────────────────────────────

    #[test]
    fn test_render_files_one_path_per_line_with_trailing_newline() {
        let files = vec!["libs/domain/src/lib.rs".to_owned(), "libs/usecase/src/lib.rs".to_owned()];
        let out = render_files(&files);
        assert_eq!(out, "libs/domain/src/lib.rs\nlibs/usecase/src/lib.rs\n");
    }

    #[test]
    fn test_render_files_empty_returns_empty_string() {
        let out = render_files(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn test_render_files_preserves_caller_order() {
        // The caller (interactor) decides ordering; render must not reorder.
        let files = vec!["z.rs".to_owned(), "a.rs".to_owned(), "m.rs".to_owned()];
        let out = render_files(&files);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines, vec!["z.rs", "a.rs", "m.rs"]);
    }
}
