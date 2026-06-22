//! `sotp demo` (or default when no subcommand is given) — `DemoCompositionRoot`.
//!
//! `DemoCompositionRoot` is the per-context composition root for the `demo`
//! command.  `CliApp` keeps a shim method that delegates here for backward
//! compatibility.

use crate::{CliApp, CommandOutcome, error::CompositionError};

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `demo` command.
///
/// This family has no injectable adapter dependencies.
pub struct DemoCompositionRoot;

impl DemoCompositionRoot {
    /// Create a new `DemoCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DemoCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoCompositionRoot {
    /// Run the built-in demo / default stub (used when no subcommand is given).
    ///
    /// # Errors
    ///
    /// Returns `Err` when the demo fails to create or persist the example track.
    pub fn demo(&self) -> Result<CommandOutcome, CompositionError> {
        let msg = infrastructure::demo::run_example_demo()
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        Ok(CommandOutcome::success(Some(msg)))
    }
}

// ---------------------------------------------------------------------------
// CliApp compatibility shim
// ---------------------------------------------------------------------------

impl CliApp {
    /// Run the built-in demo / default stub (used when no subcommand is given).
    ///
    /// Delegates to [`DemoCompositionRoot::demo`].
    ///
    /// # Errors
    ///
    /// Returns `Err` when the demo fails to create or persist the example track.
    pub fn demo(&self) -> Result<CommandOutcome, CompositionError> {
        DemoCompositionRoot::new().demo()
    }
}
