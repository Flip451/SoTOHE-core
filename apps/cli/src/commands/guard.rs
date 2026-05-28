//! Guard subcommand for shell command policy checking.

use std::process::ExitCode;

use cli_composition::CliApp;

use crate::commands::outcome_to_exit;

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
        GuardCommand::Check { command } => outcome_to_exit(CliApp::new().guard_check(command)),
    }
}
