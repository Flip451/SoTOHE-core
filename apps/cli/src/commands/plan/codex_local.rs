//! Thin dispatch entry-point for the local Codex-backed planner.
//!
//! Production code converts raw clap args into a [`PlanInput`], then
//! delegates planner execution to `cli_composition::PlanCompositionRoot`
//! (which wires `CodexPlannerAdapter` → `PlannerInteractor` → `PlanDriver`).
//!
//! No subprocess management, prompt resolution, session-log I/O, or Codex
//! argv construction is performed here — all of that lives in
//! `infrastructure::codex_planner` and `usecase::planner`.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_driver::plan::PlanInput;

use super::{PLAN_RUNTIME_DIR, PlanCodexLocalArgs};
use crate::commands::driver_outcome_to_exit;

/// Execute the local Codex-backed planner.
///
/// Converts CLI args into a `PlanInput`, delegates planner execution to the
/// `PlanDriver` built by `PlanCompositionRoot`, and maps the outcome to an
/// `ExitCode`. No subprocess, prompt resolution, or session-log I/O is
/// performed in this function.
pub(super) fn execute_codex_local(args: &PlanCodexLocalArgs) -> ExitCode {
    run_execute_codex_local(args, |input| {
        let runtime_dir = PathBuf::from(PLAN_RUNTIME_DIR);
        cli_composition::PlanCompositionRoot::new(runtime_dir).plan_driver().handle(input)
    })
}

/// Test-injectable dispatcher: takes a closure for handling the `PlanInput`,
/// allowing tests to mock the driver. Production caller is `execute_codex_local`.
///
/// New in this track (thin-bin refactor, ADR 1420 D1 / ADR 1328 D5).
pub(super) fn run_execute_codex_local(
    args: &PlanCodexLocalArgs,
    handle: impl FnOnce(PlanInput) -> cli_driver::CommandOutcome,
) -> ExitCode {
    let input = plan_input_from_args(args);
    driver_outcome_to_exit(handle(input))
}

/// Validate clap args and construct `PlanInput`.
///
/// Carries raw (unresolved) args; prompt resolution and briefing-file
/// validation are delegated to `PlannerInteractor` in the usecase layer.
/// Test-callable private helper introduced by thin-bin refactor.
pub(super) fn plan_input_from_args(args: &PlanCodexLocalArgs) -> PlanInput {
    PlanInput::RunCodexLocal {
        model: args.model.clone(),
        timeout_seconds: args.timeout_seconds,
        briefing_file: args.briefing_file.clone(),
        prompt: args.prompt.clone(),
    }
}
