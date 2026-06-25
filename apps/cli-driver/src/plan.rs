//! `plan` command family — primary adapter driver.
//!
//! `PlanDriver` holds an injected `PlannerPort` and exposes
//! `handle(PlanInput) -> CommandOutcome`. All planner execution is
//! delegated to the port; no I/O work is performed here.

use std::sync::Arc;

use usecase::planner::PlannerPort;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `plan` command family.
pub enum PlanInput {
    /// Run the local Codex-backed planner.
    RunCodexLocal {
        /// Model name for the planner.
        model: String,
        /// Timeout for the planner execution in seconds.
        timeout_seconds: u64,
        /// Full prompt string (already resolved from briefing file or inline).
        prompt: String,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `plan` command family.
///
/// Holds a single injected `PlannerPort`; exposes
/// `handle(PlanInput) -> CommandOutcome`. No I/O is performed here —
/// planner execution is delegated to the port (D5 thin-bin / ADR 1328).
pub struct PlanDriver {
    /// Injected planner port — the only way to perform planner execution.
    pub planner: Arc<dyn PlannerPort>,
}

impl PlanDriver {
    /// Handle a plan command.
    pub fn handle(&self, input: PlanInput) -> CommandOutcome {
        match input {
            PlanInput::RunCodexLocal { model, timeout_seconds, prompt } => {
                self.run_codex_local(model, timeout_seconds, prompt)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Operation implementations
    // -----------------------------------------------------------------------

    fn run_codex_local(
        &self,
        model: String,
        timeout_seconds: u64,
        prompt: String,
    ) -> CommandOutcome {
        match self.planner.run(&model, &prompt, timeout_seconds) {
            Ok(output) => {
                CommandOutcome { stdout: None, stderr: None, exit_code: output.exit_code }
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}
