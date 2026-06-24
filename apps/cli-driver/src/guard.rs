// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `guard` command family — primary adapter driver.
//!
//! `GuardDriver` holds an injected [`usecase::guard::GuardCheckService`] and exposes
//! `handle(input) -> CommandOutcome`.  JSON formatting is performed here at the
//! driver boundary; the usecase layer never sees JSON.

use std::sync::Arc;

use usecase::guard::{GuardCheckService, GuardDecision};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `guard` command family.
pub enum GuardInput {
    /// Check a shell command against the guard policy.
    Check {
        /// The shell command string to check.
        command: String,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `guard` command family.
///
/// Holds an injected [`GuardCheckService`]; exposes `handle(input) -> CommandOutcome`.
pub struct GuardDriver {
    service: Arc<dyn GuardCheckService>,
}

impl GuardDriver {
    /// Create a new `GuardDriver` with the given service.
    pub fn new(service: Arc<dyn GuardCheckService>) -> Self {
        Self { service }
    }

    /// Handle a guard command.
    ///
    /// Returns a JSON verdict (`{"decision":"allow"|"block","reason":"..."}`) in stdout.
    /// Exit code 0 = allow, 1 = block.
    pub fn handle(&self, input: GuardInput) -> CommandOutcome {
        match input {
            GuardInput::Check { command } => self.guard_check(command),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn guard_check(&self, command: String) -> CommandOutcome {
        let output = self.service.check(command);

        let (decision_str, is_blocked) = match output.decision {
            GuardDecision::Allow => ("allow", false),
            GuardDecision::Block => ("block", true),
        };

        let json = serde_json::json!({
            "decision": decision_str,
            "reason": output.reason,
        });

        let exit_code: u8 = if is_blocked { 1 } else { 0 };
        CommandOutcome { stdout: Some(json.to_string()), stderr: None, exit_code }
    }
}
