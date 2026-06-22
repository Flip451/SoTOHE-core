// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `guard` command family — primary adapter driver.
//!
//! `GuardDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The JSON formatting helpers here
//! mirror those in `apps/cli-composition/src/guard.rs` (lines ~56-63);
//! T021 removes the `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::sync::Arc;
// use usecase::hook_dispatch::{
//     HookDispatchCommand, HookDispatchInteractor, HookDispatchService, HookVerdictDecision,
// };
// use infrastructure::shell::ConchShellParser;

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
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct GuardDriver {
    // TODO(T021): inject use-case interactors here.
    // hook_dispatch_service: Arc<dyn usecase::hook_dispatch::HookDispatchService>,
}

impl GuardDriver {
    /// Create a new `GuardDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a guard command.
    ///
    /// Returns a JSON verdict (`{"decision":"allow"|"block","reason":"..."}`) in stdout.
    /// Exit code 0 = allow, 1 = block.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: GuardInput) -> CommandOutcome {
        match input {
            GuardInput::Check { command } => self.guard_check(command),
        }
    }

    // -----------------------------------------------------------------------
    // JSON formatting helpers (duplicated from cli_composition/src/guard.rs
    // lines ~56-63; T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn guard_check(&self, _command: String) -> CommandOutcome {
        // Stub: cli_driver Driver::handle is not yet wired (deferred from T021).
        // Returning failure is the safe choice here — silently returning
        // `{"decision":"allow"}` would bypass the git-operation guard for any
        // caller that switches to this primary-adapter path before the
        // HookDispatchInteractor wiring lands.
        CommandOutcome::failure(Some(
            "cli_driver Driver::handle is not yet wired — apps/cli still routes through \
             cli_composition CompositionRoot dispatch (deferred from T021); call the matching \
             CompositionRoot method instead"
                .to_owned(),
        ))
    }
}

impl Default for GuardDriver {
    fn default() -> Self {
        Self::new()
    }
}
