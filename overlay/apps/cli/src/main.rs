//! CLI entry point (delivery adapter binary).
//!
//! The outermost layer. It reads process input, delegates to the composition
//! root ([`cli_composition::run_greeting`]), and presents the resulting
//! [`CommandOutcome`] on stdout/stderr. It holds no business logic of its own.

use std::process::ExitCode;

use cli_composition::run_greeting;
use cli_driver::CommandOutcome;

fn main() -> ExitCode {
    let name = std::env::args().nth(1).unwrap_or_else(|| "world".to_owned());
    present(run_greeting(&name))
}

/// Prints the command result and maps it to a process exit code.
fn present(result: Result<CommandOutcome, impl std::fmt::Display>) -> ExitCode {
    match result {
        Ok(outcome) => {
            println!("{}", outcome.message);
            if outcome.success { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
