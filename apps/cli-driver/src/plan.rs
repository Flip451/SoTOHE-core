//! `plan` command family — primary adapter driver.
//!
//! `PlanDriver` holds an injected [`usecase::planner::PlannerService`] and exposes
//! `handle(PlanInput) -> CommandOutcome`. All planner execution is
//! delegated to the service; no I/O work is performed here.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::planner::PlannerService;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `plan` command family.
///
/// Carries raw (unresolved) CLI args; prompt resolution and briefing-file
/// validation are performed by `PlannerInteractor`.
pub enum PlanInput {
    /// Run the local Codex-backed planner.
    RunCodexLocal {
        /// Model name for the planner.
        model: String,
        /// Timeout for the planner execution in seconds.
        timeout_seconds: u64,
        /// Optional path to a briefing file (mutually exclusive with `prompt`).
        briefing_file: Option<PathBuf>,
        /// Optional inline prompt string (mutually exclusive with `briefing_file`).
        prompt: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `plan` command family.
///
/// Holds a single injected [`PlannerService`]; exposes
/// `handle(PlanInput) -> CommandOutcome`. No I/O is performed here —
/// planner execution is delegated to the service (D5 thin-bin / ADR 1328).
pub struct PlanDriver {
    service: Arc<dyn PlannerService>,
}

impl PlanDriver {
    /// Create a new `PlanDriver` with the given `PlannerService`.
    pub fn new(service: Arc<dyn PlannerService>) -> Self {
        Self { service }
    }

    /// Handle a plan command.
    pub fn handle(&self, input: PlanInput) -> CommandOutcome {
        match input {
            PlanInput::RunCodexLocal { model, timeout_seconds, briefing_file, prompt } => {
                self.run_codex_local(model, timeout_seconds, briefing_file, prompt)
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
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
    ) -> CommandOutcome {
        match self.service.run_codex_local(model, briefing_file, prompt, timeout_seconds) {
            Ok(output) => {
                CommandOutcome { stdout: None, stderr: None, exit_code: output.exit_code }
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}
