//! `hook` command family — CliApp impl methods.

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Dispatch a security-critical hook via Rust logic.
    ///
    /// Reads Claude Code hook JSON from stdin.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn hook_dispatch(&self, hook_name: String) -> Result<CommandOutcome, String> {
        let _ = hook_name;
        Err(String::from("not implemented"))
    }
}
