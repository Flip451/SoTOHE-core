//! Thin dispatch entry-point for the local Codex-backed reviewer.
//!
//! Production code never imports `domain::` types directly (CN-01 / AC-03).
//! All domain conversions happen inside `infrastructure::review_v2`.
//!
//! Subprocess management (spawn, tee-stderr, timeout, kill) and Codex argv
//! construction live in `infrastructure`. This module only translates clap
//! args into a `ReviewInput` and delegates to `ReviewCompositionRoot`.

use std::io::{self, Write};
use std::process::ExitCode;

use cli_driver::{CommandOutcome, review::ReviewInput};

use super::{CodexLocalArgs, validate_auto_record_args};

pub(super) fn execute_codex_local(args: &CodexLocalArgs) -> ExitCode {
    run_execute_codex_local(args, |input| {
        cli_composition::ReviewCompositionRoot::new().review_driver().handle(input)
    })
}

pub(super) fn run_execute_codex_local(
    args: &CodexLocalArgs,
    handle: impl FnOnce(ReviewInput) -> CommandOutcome,
) -> ExitCode {
    let input = match review_input_from_args(args) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let outcome = handle(input);
    match emit_outcome_output(
        outcome.stdout.as_deref(),
        outcome.stderr.as_deref(),
        outcome.exit_code,
    ) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

pub(super) fn review_input_from_args(
    args: &CodexLocalArgs,
) -> Result<ReviewInput, crate::CliError> {
    // Validate record args before delegating to the driver (fail fast).
    let validated = validate_auto_record_args(args)?;

    Ok(ReviewInput::RunCodex {
        model: args.model.clone(),
        timeout_seconds: args.timeout_seconds,
        briefing_file: args.briefing_file.clone(),
        prompt: args.prompt.clone(),
        track_id: Some(validated.track_id),
        round_type: validated.round_type_str,
        group: validated.group_name,
        items_dir: validated.items_dir,
    })
}

fn emit_outcome_output(
    stdout: Option<&str>,
    stderr: Option<&str>,
    exit_code: u8,
) -> Result<u8, crate::CliError> {
    emit_outcome_output_to(stdout, stderr, exit_code, &mut io::stdout())
}

pub(super) fn emit_outcome_output_to<W: Write>(
    stdout: Option<&str>,
    stderr: Option<&str>,
    exit_code: u8,
    stdout_writer: &mut W,
) -> Result<u8, crate::CliError> {
    if let Some(line) = stdout {
        writeln!(stdout_writer, "{line}")?;
    }
    if let Some(line) = stderr {
        eprintln!("{line}");
    }
    Ok(exit_code)
}

// ---------------------------------------------------------------------------
// Test helpers: scope-config and safety-guard assertions (not subprocess shims)
// ---------------------------------------------------------------------------
//
// The functions below call into cli_composition to verify the scope briefing
// injection contract and the briefing-path safety guard. They are NOT subprocess
// shims — they do not spawn processes, manage sessions, or construct Codex argv.

/// Appends a scope-specific severity policy reference section to `prompt`
/// when the given scope has a `briefing_file` configured and the path is safe
/// to inject.
///
/// No domain types involved — uses string-based lookup via infrastructure.
/// Becomes a no-op when the scope has no briefing configured, the scope is
/// "other", or the configured path fails `is_safe_briefing_path`.
///
/// # Errors
/// Returns an error if scope config cannot be loaded.
#[cfg(test)]
pub(super) fn append_scope_briefing_reference(
    prompt: &mut String,
    scope_name: &str,
    track_id: &str,
    items_dir: &std::path::Path,
) -> Result<(), String> {
    cli_composition::review_v2::append_scope_briefing_reference_str(
        prompt,
        scope_name,
        track_id,
        items_dir,
        is_safe_briefing_path,
    )
}

/// Returns `true` if `path` is safe to reference as a repo-relative briefing
/// file and to inject into the markdown prompt as a backtick-quoted path bullet.
///
/// Rejects strings that contain **any of the following**:
///
/// Prompt-injection class (would break out of the `` `path` `` markdown context
/// or smuggle additional prompt lines):
/// - Any Unicode control character (`char::is_control`, Unicode category Cc) —
///   covers ASCII C0 0x00–0x1F (including `\n`, `\r`, `\t`), DEL 0x7F,
///   and C1 controls 0x80–0x9F (including NEL U+0085)
/// - Line / paragraph separators U+2028 (Zl) and U+2029 (Zp) — not in category
///   Cc and therefore not caught by `is_control`, but both act as line breaks
/// - Backtick (`` ` ``)
///
/// Path-traversal class:
/// - Absolute paths starting with `/` or `\`
/// - Windows UNC and drive-letter prefixes (e.g. `\\server\share`, `C:\...`)
/// - Any `..` component (e.g. `track/../../etc/passwd`), split on either
///   `/` or `\`
///
/// Empty paths are also rejected.
#[cfg(test)]
pub(super) fn is_safe_briefing_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    // Prompt-injection guard
    if path.chars().any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')) {
        return false;
    }
    // Absolute path (Unix root or Windows root / UNC)
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    // Windows drive-letter prefix: `C:` / `c:` etc.
    if let (Some(first), Some(second)) = (path.as_bytes().first(), path.as_bytes().get(1)) {
        if *second == b':' && first.is_ascii_alphabetic() {
            return false;
        }
    }
    // Path-traversal: reject any `..` component (check both separators).
    if path.split(['/', '\\']).any(|component| component == "..") {
        return false;
    }
    true
}
