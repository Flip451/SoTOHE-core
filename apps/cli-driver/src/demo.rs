// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `demo` command family — primary adapter driver.
//!
//! `DemoDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror
//! `apps/cli-composition/src/demo.rs`; T021 removes the `cli_composition`
//! duplicate when the live path is flipped.

// TODO(T021): add infrastructure imports once Cargo.toml is materialized.
// use infrastructure::demo::run_example_demo;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `demo` command family.
pub enum DemoInput {
    /// Run the built-in demo / default stub (used when no subcommand is given).
    Run,
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `demo` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct DemoDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure::demo::run_example_demo
    // is called inline, same as cli_composition::DemoCompositionRoot).
}

impl DemoDriver {
    /// Create a new `DemoDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a demo command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: DemoInput) -> CommandOutcome {
        match input {
            DemoInput::Run => self.demo(),
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/demo.rs;
    // T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn demo(&self) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::demo::run_example_demo here.
        // Mirrors cli_composition/src/demo.rs DemoCompositionRoot::demo.
        CommandOutcome::success(None)
    }
}

impl Default for DemoDriver {
    fn default() -> Self {
        Self::new()
    }
}
