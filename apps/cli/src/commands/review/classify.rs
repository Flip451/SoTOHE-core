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

use domain::CommitHash;
use domain::review_v2::{FilePath, FilePathError, MainScopeName};
use infrastructure::review_v2::GitDiffGetter;
use usecase::review_v2::{
    PathClassification, ScopeClassification, ScopeQueryInteractor, ScopeQueryService,
};

use super::compose_v2;

/// Placeholder diff base for `classify`. The interactor does not call the
/// diff getter from `classify`, so the value never reaches git; using a fixed
/// 40-char zero hash keeps `CommitHash::try_new` happy without resolving an
/// actual base commit.
const CLASSIFY_PLACEHOLDER_BASE: &str = "0000000000000000000000000000000000000000";

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
    let track_id =
        domain::TrackId::try_new(&args.track_id).map_err(|e| format!("invalid --track-id: {e}"))?;

    let validated_paths = validate_paths(&args.paths)?;

    let scope_config = compose_v2::load_scope_config_only(&track_id, &args.items_dir)?;
    let base = CommitHash::try_new(CLASSIFY_PLACEHOLDER_BASE)
        .map_err(|e| format!("internal error: classify placeholder base is invalid: {e}"))?;
    let interactor = ScopeQueryInteractor::new(scope_config, GitDiffGetter, base);

    let classifications =
        interactor.classify(validated_paths).map_err(|e| format!("classify failed: {e}"))?;

    Ok(render_classifications(&classifications))
}

/// Validates each input path via `FilePath::new`. Collects all errors and
/// reports them together; returns `Err` with the joined messages if any path
/// fails (CN-03 / AC-05).
fn validate_paths(paths: &[String]) -> Result<Vec<FilePath>, String> {
    let mut validated = Vec::with_capacity(paths.len());
    let mut errors = Vec::new();
    for raw in paths {
        match FilePath::new(raw.as_str()) {
            Ok(path) => validated.push(path),
            Err(err) => errors.push(format_filepath_error(raw, &err)),
        }
    }
    if errors.is_empty() { Ok(validated) } else { Err(errors.join("\n")) }
}

fn format_filepath_error(raw: &str, err: &FilePathError) -> String {
    match err {
        FilePathError::Empty => "invalid path: empty string".to_owned(),
        FilePathError::Absolute(_) => {
            format!("invalid path '{raw}': absolute paths are not allowed (use repo-relative)")
        }
        FilePathError::Traversal(_) => {
            format!("invalid path '{raw}': '..' traversal components are not allowed")
        }
    }
}

fn render_classifications(classifications: &[PathClassification]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for entry in classifications {
        let scope = format_classification(&entry.classification);
        let _ = writeln!(out, "{path}\t{scope}", path = entry.path);
    }
    out
}

/// Formats a `ScopeClassification` for stdout output.
///
/// - `Named(head, tail)` — head and tail combined and sorted alphabetically,
///   joined with `,` (AC-02).
/// - `Other` — the literal string `other` (AC-10).
/// - `Excluded` — the literal string `<excluded>` (AC-03).
fn format_classification(classification: &ScopeClassification) -> String {
    match classification {
        ScopeClassification::Named(head, tail) => {
            let mut names: Vec<&MainScopeName> = std::iter::once(head).chain(tail).collect();
            names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(",")
        }
        ScopeClassification::Other => "other".to_owned(),
        ScopeClassification::Excluded => "<excluded>".to_owned(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn fp(s: &str) -> FilePath {
        FilePath::new(s).unwrap()
    }

    fn main_scope(name: &str) -> MainScopeName {
        MainScopeName::new(name).unwrap()
    }

    // ── format_classification ──────────────────────────────────────

    #[test]
    fn test_format_classification_named_single_returns_name() {
        let cls = ScopeClassification::Named(main_scope("domain"), vec![]);
        assert_eq!(format_classification(&cls), "domain");
    }

    #[test]
    fn test_format_classification_named_multi_returns_alphabetical_csv() {
        let cls = ScopeClassification::Named(
            main_scope("usecase"),
            vec![main_scope("domain"), main_scope("infrastructure")],
        );
        assert_eq!(format_classification(&cls), "domain,infrastructure,usecase");
    }

    #[test]
    fn test_format_classification_other_returns_literal_other() {
        assert_eq!(format_classification(&ScopeClassification::Other), "other");
    }

    #[test]
    fn test_format_classification_excluded_returns_literal_excluded() {
        assert_eq!(format_classification(&ScopeClassification::Excluded), "<excluded>");
    }

    // ── validate_paths ─────────────────────────────────────────────

    #[test]
    fn test_validate_paths_with_all_valid_returns_ok() {
        let result =
            validate_paths(&["libs/domain/src/lib.rs".to_owned(), "Cargo.toml".to_owned()]);
        let validated = result.unwrap();
        assert_eq!(validated.len(), 2);
        assert_eq!(validated[0].as_str(), "libs/domain/src/lib.rs");
        assert_eq!(validated[1].as_str(), "Cargo.toml");
    }

    #[test]
    fn test_validate_paths_with_empty_string_returns_error() {
        let result = validate_paths(&[String::new()]);
        let err = result.unwrap_err();
        assert!(err.contains("empty string"));
    }

    #[test]
    fn test_validate_paths_with_absolute_path_returns_error() {
        let result = validate_paths(&["/etc/passwd".to_owned()]);
        let err = result.unwrap_err();
        assert!(err.contains("absolute paths are not allowed"));
    }

    #[test]
    fn test_validate_paths_with_traversal_returns_error() {
        let result = validate_paths(&["../../etc/passwd".to_owned()]);
        let err = result.unwrap_err();
        assert!(err.contains("'..'"));
    }

    #[test]
    fn test_validate_paths_collects_all_errors() {
        let result = validate_paths(&[
            "valid.rs".to_owned(),
            String::new(),
            "/abs.rs".to_owned(),
            "../traverse.rs".to_owned(),
        ]);
        let err = result.unwrap_err();
        // Three error messages joined with newlines.
        assert!(err.contains("empty string"));
        assert!(err.contains("absolute paths are not allowed"));
        assert!(err.contains("'..'"));
    }

    // ── render_classifications ─────────────────────────────────────

    #[test]
    fn test_render_classifications_named_uses_tab_separator() {
        let entries = vec![PathClassification {
            path: fp("libs/domain/src/lib.rs"),
            classification: ScopeClassification::Named(main_scope("domain"), vec![]),
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "libs/domain/src/lib.rs\tdomain\n");
    }

    #[test]
    fn test_render_classifications_other_uses_literal_other() {
        let entries = vec![PathClassification {
            path: fp("Cargo.toml"),
            classification: ScopeClassification::Other,
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "Cargo.toml\tother\n");
    }

    #[test]
    fn test_render_classifications_excluded_uses_literal_excluded() {
        let entries = vec![PathClassification {
            path: fp("track/registry.md"),
            classification: ScopeClassification::Excluded,
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "track/registry.md\t<excluded>\n");
    }

    #[test]
    fn test_render_classifications_preserves_input_order() {
        let entries = vec![
            PathClassification {
                path: fp("Cargo.toml"),
                classification: ScopeClassification::Other,
            },
            PathClassification {
                path: fp("libs/domain/src/lib.rs"),
                classification: ScopeClassification::Named(main_scope("domain"), vec![]),
            },
            PathClassification {
                path: fp("track/registry.md"),
                classification: ScopeClassification::Excluded,
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
        let entries = vec![PathClassification {
            path: fp("shared/foo.rs"),
            classification: ScopeClassification::Named(
                main_scope("alpha"),
                vec![main_scope("beta")],
            ),
        }];
        let out = render_classifications(&entries);
        assert_eq!(out, "shared/foo.rs\talpha,beta\n");
    }

    #[test]
    fn test_render_classifications_empty_returns_empty_string() {
        let out = render_classifications(&[]);
        assert!(out.is_empty());
    }

    // ── placeholder base ───────────────────────────────────────────

    #[test]
    fn test_placeholder_base_is_valid_commit_hash() {
        // Documents the invariant that the placeholder constant parses successfully —
        // run_classify relies on this.
        assert!(CommitHash::try_new(CLASSIFY_PLACEHOLDER_BASE).is_ok());
    }
}
