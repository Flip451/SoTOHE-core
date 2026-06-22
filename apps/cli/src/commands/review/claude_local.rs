//! Subprocess management for the local Claude-backed reviewer.
//!
//! Production code never imports `domain::` types directly (CN-01 / AC-03).
//! All domain conversions happen inside `cli_composition::CliApp`.

use std::io::{self, Write};
use std::process::ExitCode;

use cli_composition::ReviewRunClaudeInput;

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
    // Step 1: Validate record args before delegating to CliApp (fail fast).
    let validated = validate_claude_auto_record_args(args)?;

    // Step 2: Build DTO and delegate to CliApp.review_run_claude.
    let input = ReviewRunClaudeInput {
        model: args.model.clone(),
        timeout_seconds: args.timeout_seconds,
        briefing_file: args.briefing_file.clone(),
        prompt: args.prompt.clone(),
        track_id: Some(validated.track_id),
        round_type: validated.round_type_str,
        group: validated.group_name,
        items_dir: validated.items_dir,
    };

    let outcome = cli_composition::ReviewCompositionRoot::new()
        .review_run_claude(input)
        .map_err(|e| e.to_string())?;

    if let Some(line) = &outcome.stdout {
        writeln!(io::stdout(), "{line}").map_err(|e| format!("failed to write stdout: {e}"))?;
    }
    Ok(outcome.exit_code)
}
