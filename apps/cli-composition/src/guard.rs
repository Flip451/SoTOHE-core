//! `guard` command family — CliApp impl methods.

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Check a shell command against the guard policy.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn guard_check(&self, command: String) -> Result<CommandOutcome, String> {
        let _ = command;
        Err(String::from("not implemented"))
    }
}
