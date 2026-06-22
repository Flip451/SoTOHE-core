//! `sotp demo` (or default when no subcommand is given) — run the built-in example.

use crate::{CliApp, CommandOutcome, error::CompositionError};

impl CliApp {
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
