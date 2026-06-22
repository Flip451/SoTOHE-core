//! `sotp demo` (or default when no subcommand is given) — `DemoCompositionRoot`.

use crate::{CommandOutcome, error::CompositionError};

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
