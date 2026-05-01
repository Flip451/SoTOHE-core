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

use usecase::review_v2::{ScopeClassificationOutput, ScopeQueryService};

use super::compose_v2;

/// CLI arguments for `sotp review classify`.
#[derive(Debug, Args)]
pub struct ClassifyArgs {
    /// Track ID (used to expand `<track-id>` placeholders in scope patterns).
    #[arg(long)]
    pub(super) track_id: String,

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
    // Pre-validate all paths and collect every error before delegating to the
    // interactor. `classify_by_strings` short-circuits on the first invalid
    // path; collecting errors here restores the multi-error reporting that the
    // old `validate_paths` loop provided (CN-03 / AC-05).
    validate_all_paths(&args.paths)?;

    let interactor =
        compose_v2::build_scope_query_interactor_no_diff_str(&args.track_id, &args.items_dir)?;

    let classifications = interactor
        .classify_by_strings(args.paths.clone())
        .map_err(|e| format!("classify failed: {e}"))?;

    Ok(render_classifications(&classifications))
}

/// Validate every path and return a joined error if any fail.
///
/// Rules mirror `domain::FilePath::new` exactly:
/// - empty string → rejected
/// - starts with `/` (Unix absolute), `\\` (Windows UNC), or has Windows
///   drive prefix (`C:\` / `C:/`) → rejected as absolute
/// - contains `..` component (using `/` or `\` as separators) → traversal → rejected
///
/// # Errors
///
/// Returns a newline-joined string of all validation errors when any path fails.
fn validate_all_paths(paths: &[String]) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();
    for raw in paths {
        if raw.is_empty() {
            errors.push("invalid path: empty string".to_owned());
        } else if raw.starts_with('/')
            || raw.starts_with('\\')
            || raw.get(1..3).is_some_and(|p| p == ":\\" || p == ":/")
        {
            errors.push(format!(
                "invalid path '{raw}': absolute paths are not allowed (use repo-relative)"
            ));
        } else {
            // Check for `..` traversal components using both Unix and Windows separators
            // — matches FilePath::Traversal rejection exactly.
            let has_traversal = raw.split(&['/', '\\'][..]).any(|seg| seg == "..");
            if has_traversal {
                errors.push(format!(
                    "invalid path '{raw}': '..' traversal components are not allowed"
                ));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

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
