//! `sotp review files` — per-scope diff file enumerator.
//!
//! Validates the requested scope name against the configured scope universe
//! **before** any diff I/O (AC-08). On valid scope, wires `ScopeQueryInteractor`
//! and prints one `FilePath` per line to stdout (CN-04 / IN-06 / AC-07).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

use domain::review_v2::{MainScopeName, ScopeName};
use usecase::review_v2::{ScopeQueryError, ScopeQueryInteractor, ScopeQueryService};

use super::compose_v2;

/// CLI arguments for `sotp review files`.
#[derive(Debug, Args)]
pub struct FilesArgs {
    /// Track ID (used to expand `<track-id>` placeholders in scope patterns).
    #[arg(long)]
    pub(super) track_id: String,

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
    let track_id =
        domain::TrackId::try_new(&args.track_id).map_err(|e| format!("invalid --track-id: {e}"))?;

    // Step (a) — load scope_config without resolving diff base. AC-08 requires
    // scope validation to happen before diff I/O.
    let scope_config = compose_v2::load_scope_config_only(&track_id, &args.items_dir)?;

    let scope = parse_scope_name(&args.scope)?;
    if !scope_config.contains_scope(&scope) {
        return Err(format_unknown_scope_message(&args.scope, &scope_config));
    }

    // Step (c) — only after scope validation, resolve diff base and wire the
    // interactor.
    let (diff_getter, base) = compose_v2::resolve_diff_base_and_getter(&track_id, &args.items_dir)?;
    let interactor = ScopeQueryInteractor::new(scope_config, diff_getter, base);

    let files = interactor.files(scope).map_err(format_files_error)?;
    Ok(render_files(&files))
}

/// Parses a CLI scope string into a `ScopeName`. The literal `other` maps to
/// `ScopeName::Other`; any other valid identifier becomes `ScopeName::Main(...)`.
/// Format-level rejection (e.g., empty / non-ASCII / reserved-name attempts on
/// `MainScopeName::new`) is surfaced as an error string here. Whether the
/// scope is **defined** in the current track config is checked separately by
/// `contains_scope`.
fn parse_scope_name(raw: &str) -> Result<ScopeName, String> {
    if raw.eq_ignore_ascii_case("other") {
        return Ok(ScopeName::Other);
    }
    MainScopeName::new(raw)
        .map(ScopeName::Main)
        .map_err(|e| format!("invalid --scope '{raw}': {e}"))
}

/// Returns an `Unknown scope: <name>. Known scopes: <sorted list>` message
/// (AC-08). The known scopes set comes from `all_scope_names()` (named scopes
/// + the implicit `other`).
fn format_unknown_scope_message(
    raw: &str,
    scope_config: &domain::review_v2::ReviewScopeConfig,
) -> String {
    let mut names: Vec<String> =
        scope_config.all_scope_names().iter().map(ToString::to_string).collect();
    names.sort();
    format!("Unknown scope: {raw}. Known scopes: {}", names.join(", "))
}

fn format_files_error(err: ScopeQueryError) -> String {
    match err {
        ScopeQueryError::DiffGet(inner) => format!("diff getter failed: {inner}"),
        // UnknownScope should be unreachable here because run_files validates
        // scope membership first; surface as an internal error if we somehow
        // get here.
        ScopeQueryError::UnknownScope(scope) => {
            format!("internal error: scope '{scope}' rejected by interactor after CLI validation")
        }
    }
}

fn render_files(files: &[domain::review_v2::FilePath]) -> String {
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
    use domain::TrackId;
    use domain::review_v2::{FilePath, ReviewScopeConfig};

    fn fp(s: &str) -> FilePath {
        FilePath::new(s).unwrap()
    }

    fn track_id() -> TrackId {
        TrackId::try_new("test-track-2026-04-30").unwrap()
    }

    fn config_domain_usecase() -> ReviewScopeConfig {
        ReviewScopeConfig::new(
            &track_id(),
            vec![
                ("domain".to_owned(), vec!["libs/domain/**".to_owned()], None),
                ("usecase".to_owned(), vec!["libs/usecase/**".to_owned()], None),
            ],
            vec![],
            vec![],
        )
        .unwrap()
    }

    // ── parse_scope_name ───────────────────────────────────────────

    #[test]
    fn test_parse_scope_name_other_returns_other_variant() {
        let result = parse_scope_name("other").unwrap();
        assert_eq!(result, ScopeName::Other);
    }

    #[test]
    fn test_parse_scope_name_other_case_insensitive() {
        let result = parse_scope_name("Other").unwrap();
        assert_eq!(result, ScopeName::Other);
        let result = parse_scope_name("OTHER").unwrap();
        assert_eq!(result, ScopeName::Other);
    }

    #[test]
    fn test_parse_scope_name_main_returns_main_variant() {
        let result = parse_scope_name("domain").unwrap();
        match result {
            ScopeName::Main(name) => assert_eq!(name.as_str(), "domain"),
            ScopeName::Other => panic!("expected Main"),
        }
    }

    #[test]
    fn test_parse_scope_name_empty_returns_error() {
        let result = parse_scope_name("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_scope_name_non_ascii_returns_error() {
        let result = parse_scope_name("日本語");
        assert!(result.is_err());
    }

    // ── format_unknown_scope_message ────────────────────────────────

    #[test]
    fn test_format_unknown_scope_message_lists_known_scopes_sorted() {
        let cfg = config_domain_usecase();
        let msg = format_unknown_scope_message("nonexistent", &cfg);
        // Known scopes contains the named scopes (alphabetical) + "other".
        assert!(msg.starts_with("Unknown scope: nonexistent. Known scopes: "));
        assert!(msg.contains("domain"));
        assert!(msg.contains("usecase"));
        assert!(msg.contains("other"));
        // Sorted: domain, other, usecase (alphabetical).
        let scope_part = msg.strip_prefix("Unknown scope: nonexistent. Known scopes: ").unwrap();
        assert_eq!(scope_part, "domain, other, usecase");
    }

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
    fn test_format_files_error_unknown_scope_marks_internal() {
        let scope = ScopeName::Main(MainScopeName::new("ghost").unwrap());
        let err = ScopeQueryError::UnknownScope(scope);
        let msg = format_files_error(err);
        assert!(msg.starts_with("internal error: "));
    }

    // ── render_files ────────────────────────────────────────────────

    #[test]
    fn test_render_files_one_path_per_line_with_trailing_newline() {
        let files = vec![fp("libs/domain/src/lib.rs"), fp("libs/usecase/src/lib.rs")];
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
        let files = vec![fp("z.rs"), fp("a.rs"), fp("m.rs")];
        let out = render_files(&files);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines, vec!["z.rs", "a.rs", "m.rs"]);
    }
}
