//! Thin dispatch entry-point for the local Codex-backed planner.
//!
//! Production code validates CLI-only prompt inputs, then delegates planner
//! execution to `cli_composition::PlanCompositionRoot` (which wires
//! `infrastructure::CodexPlannerAdapter` into `cli_driver::PlanDriver`).
//!
//! No subprocess management, session-log I/O, or Codex argv construction
//! is performed here — all of that lives in `infrastructure::codex_planner`.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_driver::plan::PlanInput;

use super::{PLAN_RUNTIME_DIR, PlanCodexLocalArgs};
use crate::commands::driver_outcome_to_exit;

/// Execute the local Codex-backed planner.
///
/// Resolves the prompt from CLI args, delegates planner execution to the
/// `PlanDriver` built by `PlanCompositionRoot`, and maps the outcome to an
/// `ExitCode`. No subprocess or session-log I/O is performed in this function.
pub(super) fn execute_codex_local(args: &PlanCodexLocalArgs) -> ExitCode {
    run_execute_codex_local(args, |input| {
        let runtime_dir = PathBuf::from(PLAN_RUNTIME_DIR);
        cli_composition::PlanCompositionRoot::new(runtime_dir).plan_driver().handle(input)
    })
}

pub(super) fn run_execute_codex_local(
    args: &PlanCodexLocalArgs,
    handle: impl FnOnce(PlanInput) -> cli_driver::CommandOutcome,
) -> ExitCode {
    let input = match plan_input_from_args(args) {
        Ok(input) => input,
        Err(err) => {
            eprintln!("{err}");
            return err.exit_code();
        }
    };

    driver_outcome_to_exit(handle(input))
}

pub(super) fn plan_input_from_args(
    args: &PlanCodexLocalArgs,
) -> Result<PlanInput, crate::CliError> {
    let prompt = if let Some(path) = &args.briefing_file {
        if !path.is_file() {
            return Err(crate::CliError::Message(format!(
                "briefing file not found: {}",
                path.display()
            )));
        }
        format!("Read {} and perform the task described there.", path.display())
    } else if let Some(inline) = args.prompt.clone() {
        inline
    } else {
        return Err(crate::CliError::Message(
            "either --briefing-file or --prompt is required".to_owned(),
        ));
    };

    Ok(PlanInput::RunCodexLocal {
        model: args.model.clone(),
        timeout_seconds: args.timeout_seconds,
        prompt,
    })
}
