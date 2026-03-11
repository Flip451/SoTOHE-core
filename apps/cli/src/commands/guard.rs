//! Guard subcommand for shell command policy checking.

use std::process::ExitCode;

use domain::Decision;
use domain::guard::policy;

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
        GuardCommand::Check { command } => execute_check(&command),
    }
}

fn execute_check(command: &str) -> ExitCode {
    let verdict = policy::check(command);

    let decision_str = match verdict.decision {
        Decision::Allow => "allow",
        Decision::Block => "block",
    };

    // Output JSON verdict to stdout
    let json = serde_json::json!({
        "decision": decision_str,
        "reason": verdict.reason,
    });
    println!("{json}");

    match verdict.decision {
        Decision::Allow => ExitCode::SUCCESS,
        Decision::Block => ExitCode::FAILURE,
    }
}
