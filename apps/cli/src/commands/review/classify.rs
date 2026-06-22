//! `sotp review classify` — per-path scope classification reporter.
//!
//! Validates each input path via `FilePath::new` (Empty / Absolute / Traversal
//! rejection) and prints `<path>TAB<scope-csv>` lines for the classification
//! result. Pure-logic command — does not touch the diff getter even though the
//! `ScopeQueryInteractor` is wired with one (per ADR D6 the interactor's
//! `classify` method is I/O-free).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

/// CLI arguments for `sotp review classify`.
#[derive(Debug, Args)]
pub struct ClassifyArgs {
    /// Track ID (used to expand `<track-id>` placeholders in scope patterns).
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub(super) track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub(super) items_dir: PathBuf,

    /// One or more repo-relative paths to classify.
    #[arg(num_args = 1.., required = true)]
    pub(super) paths: Vec<String>,
}

pub(super) fn execute_classify(args: &ClassifyArgs) -> ExitCode {
    match run_classify(args) {
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

fn run_classify(args: &ClassifyArgs) -> Result<String, String> {
    // READ resolution: anchors git discovery to the repo owning args.items_dir so that
    // track ID resolution uses the same repository as the scope config (AC-19 / CN-02).
    let track_id =
        crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir)?;

    let outcome = cli_composition::ReviewCompositionRoot::new()
        .review_classify(args.paths.clone(), Some(track_id), args.items_dir.clone())
        .map_err(|e| e.to_string())?;
    outcome.stdout.ok_or_else(|| "review classify returned no output".to_owned())
}

#[cfg(test)]
use usecase::review_v2::ScopeClassificationOutput;

#[cfg(test)]
fn render_classifications(classifications: &[ScopeClassificationOutput]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for entry in classifications {
        let scope = entry.scopes.join(",");
        let _ = writeln!(out, "{path}\t{scope}", path = entry.path);
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ── render_classifications ─────────────────────────────────────

    #[test]
    fn test_render_classifications_named_uses_tab_separator() {
        let entries = vec![ScopeClassificationOutput {
            path: "libs/domain/src/lib.rs".to_owned(),
            scopes: vec!["domain".to_owned()],
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "libs/domain/src/lib.rs\tdomain\n");
    }

    #[test]
    fn test_render_classifications_other_uses_literal_other() {
        let entries = vec![ScopeClassificationOutput {
            path: "Cargo.toml".to_owned(),
            scopes: vec!["other".to_owned()],
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "Cargo.toml\tother\n");
    }

    #[test]
    fn test_render_classifications_excluded_uses_literal_excluded() {
        let entries = vec![ScopeClassificationOutput {
            path: "track/registry.md".to_owned(),
            scopes: vec!["<excluded>".to_owned()],
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "track/registry.md\t<excluded>\n");
    }

    #[test]
    fn test_render_classifications_preserves_input_order() {
        let entries = vec![
            ScopeClassificationOutput {
                path: "Cargo.toml".to_owned(),
                scopes: vec!["other".to_owned()],
            },
            ScopeClassificationOutput {
                path: "libs/domain/src/lib.rs".to_owned(),
                scopes: vec!["domain".to_owned()],
            },
            ScopeClassificationOutput {
                path: "track/registry.md".to_owned(),
                scopes: vec!["<excluded>".to_owned()],
            },
        ];
        let out = render_classifications(&entries);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Cargo.toml\tother");
        assert_eq!(lines[1], "libs/domain/src/lib.rs\tdomain");
        assert_eq!(lines[2], "track/registry.md\t<excluded>");
    }

    #[test]
    fn test_render_classifications_multi_match_emits_csv() {
        // The scopes Vec from classify_by_strings is already sorted.
        let entries = vec![ScopeClassificationOutput {
            path: "shared/foo.rs".to_owned(),
            scopes: vec!["alpha".to_owned(), "beta".to_owned()],
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "shared/foo.rs\talpha,beta\n");
    }

    #[test]
    fn test_render_classifications_empty_returns_empty_string() {
        let out = render_classifications(&[]);
        assert!(out.is_empty());
    }
}
