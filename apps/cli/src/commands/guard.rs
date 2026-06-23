//! Guard subcommand for shell command policy checking.

use std::process::ExitCode;

use cli_composition::GuardCompositionRoot;
use cli_driver::guard::GuardInput;

use crate::commands::driver_outcome_to_exit;

/// Guard subcommands for shell command checking.
#[derive(Debug, clap::Subcommand)]
pub enum GuardCommand {
    /// Check a shell command against the guard policy.
    Check {
        /// The shell command string to check.
        #[arg(long)]
        command: String,
    },
}

/// Executes a guard subcommand.
pub fn execute(cmd: GuardCommand) -> ExitCode {
    match cmd {
        GuardCommand::Check { command } => driver_outcome_to_exit(
            GuardCompositionRoot::new().guard_driver().handle(GuardInput::Check { command }),
        ),
    }
}
