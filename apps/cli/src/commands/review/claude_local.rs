//! Subprocess management for the local Claude-backed reviewer.
//!
//! Production code never imports `domain::` types directly (CN-01 / AC-03).
//! All domain conversions happen inside `cli_composition::CliApp`.

use std::io::{self, Write};
use std::process::ExitCode;

use cli_driver::review::ReviewInput;

use super::{ClaudeLocalArgs, validate_claude_auto_record_args};

pub(super) fn execute_claude_local(args: &ClaudeLocalArgs) -> ExitCode {
    match run_execute_claude_local(args) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn run_execute_claude_local(args: &ClaudeLocalArgs) -> Result<u8, crate::CliError> {
    // Step 1: Validate record args before delegating to CliApp (fail fast).
    let validated = validate_claude_auto_record_args(args)?;

    let input = ReviewInput::RunClaude {
        model: args.model.clone(),
        timeout_seconds: args.timeout_seconds,
        briefing_file: args.briefing_file.clone(),
        prompt: args.prompt.clone(),
        track_id: Some(validated.track_id),
        round_type: validated.round_type_str,
        group: validated.group_name,
        items_dir: validated.items_dir,
    };

    let outcome = cli_composition::ReviewCompositionRoot::new().review_driver().handle(input);

    if let Some(line) = &outcome.stdout {
        writeln!(io::stdout(), "{line}").map_err(crate::CliError::Io)?;
    }
    if let Some(line) = &outcome.stderr {
        eprintln!("{line}");
    }
    Ok(outcome.exit_code)
}
