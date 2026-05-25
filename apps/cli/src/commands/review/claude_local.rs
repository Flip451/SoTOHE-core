//! Subprocess management for the local Claude-backed reviewer.
//!
//! Production code never imports `domain::` types directly (CN-01 / AC-03).
//! All domain conversions happen inside `infrastructure::review_v2`.

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use infrastructure::review_v2::{ClaudeReviewer, CodexReviewOutcome};

use super::{ClaudeLocalArgs, validate_claude_auto_record_args};

pub(super) fn execute_claude_local(args: &ClaudeLocalArgs) -> ExitCode {
    match run_execute_claude_local(args) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

fn run_execute_claude_local(args: &ClaudeLocalArgs) -> Result<u8, String> {
    // Step 1: Validate record args before building composition (fail fast).
    let validated = validate_claude_auto_record_args(args)?;

    // Step 2: Check whether the scope has a configured briefing file.
    // Log a warning if the path is unsafe (prompt injection guard) but do not
    // fail — `append_scope_briefing_reference_str` skips injection on unsafe paths.
    let maybe_briefing = infrastructure::review_v2::get_briefing_for_scope_str(
        &validated.group_name,
        &validated.track_id,
        &validated.items_dir,
    )?;
    if let Some(path) = &maybe_briefing {
        if !is_safe_briefing_path(path) {
            eprintln!(
                "[WARN] briefing_file for scope '{}' contains unsafe characters — \
                 scope-specific severity policy injection skipped",
                validated.group_name
            );
        }
    }

    // Step 3: Build base prompt and append the scope-specific severity policy reference.
    let mut base_prompt = build_base_prompt(args)?;
    infrastructure::review_v2::append_scope_briefing_reference_str(
        &mut base_prompt,
        &validated.group_name,
        &validated.track_id,
        &validated.items_dir,
        is_safe_briefing_path,
    )?;

    // Step 4: Build v2 composition with real ClaudeReviewer.
    let timeout = Duration::from_secs(args.timeout_seconds);
    let reviewer = ClaudeReviewer::new(&args.model, timeout, base_prompt)
        .with_scope_label(&validated.group_name);

    // Step 5: Run the review cycle via infrastructure (handles all domain types internally).
    // fail-closed: write failure → error returned → verdict not displayed (CN-02 / AC-03).
    let outcome = infrastructure::review_v2::run_claude_review_str(
        &validated.track_id,
        &validated.items_dir,
        &validated.group_name,
        &validated.round_type_str,
        reviewer,
    )?;

    match outcome {
        CodexReviewOutcome::Skipped { scope_label } => {
            emit_skip_output(&scope_label)?;
            Ok(0)
        }
        CodexReviewOutcome::FinalCompleted { verdict_json, exit_code } => {
            emit_stdout_line(&verdict_json)?;
            Ok(exit_code)
        }
        CodexReviewOutcome::FastCompleted { verdict_json, exit_code } => {
            emit_stdout_line(&verdict_json)?;
            Ok(exit_code)
        }
    }
}

/// Builds the base prompt from CLI args (briefing file or inline prompt).
///
/// The scope file list is NOT appended here — `ClaudeReviewer::build_full_prompt`
/// appends it when it receives the `ReviewTarget` from `ReviewCycle`.
///
/// # Errors
/// Returns an error if the briefing file does not exist or neither arg is provided.
pub(super) fn build_base_prompt(args: &ClaudeLocalArgs) -> Result<String, String> {
    if let Some(path) = &args.briefing_file {
        if !path.is_file() {
            return Err(format!("briefing file not found: {}", path.display()));
        }
        Ok(format!("Read {} and perform the task described there.", path.display()))
    } else {
        args.prompt
            .clone()
            .ok_or_else(|| "either --briefing-file or --prompt is required".to_owned())
    }
}

/// Prints the skip message and zero_findings JSON for an empty scope.
fn emit_skip_output(scope: &str) -> Result<(), String> {
    eprintln!("[auto-record] Scope '{scope}' is empty, skipping");
    emit_stdout_line(r#"{"verdict":"zero_findings","findings":[]}"#)
}

fn emit_stdout_line(line: &str) -> Result<(), String> {
    writeln!(io::stdout(), "{line}").map_err(|e| format!("failed to write stdout: {e}"))
}

/// Returns `true` if `path` is safe to reference as a repo-relative briefing
/// file and to inject into the markdown prompt as a backtick-quoted path bullet.
///
/// Mirrors the same validation logic in `codex_local::is_safe_briefing_path`.
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
