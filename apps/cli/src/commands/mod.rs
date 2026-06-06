//! CLI subcommand definitions.

use std::process::ExitCode;

use cli_composition::CommandOutcome;

pub mod domain;
pub mod dry;
pub mod file;
pub mod git;
pub mod guard;
pub mod hook;
pub mod plan;
pub mod pr;
pub mod review;
pub mod semantic_dup;
pub mod track;
pub mod verify;
#[cfg(test)]
pub mod verify_catalogue_spec_refs;

/// Convert a `CliApp` result into an `ExitCode`, printing stdout/stderr.
///
/// On `Ok(outcome)`: prints stdout (no trailing newline added — the content
/// already includes one when expected) and stderr, then returns the exit code.
/// On `Err(msg)`: prints `msg` to stderr and returns `ExitCode::FAILURE`.
pub(crate) fn outcome_to_exit(result: Result<CommandOutcome, String>) -> ExitCode {
    match result {
        Ok(outcome) => {
            if let Some(stdout) = outcome.stdout {
                println!("{stdout}");
            }
            if let Some(stderr) = outcome.stderr {
                eprintln!("{stderr}");
            }
            ExitCode::from(outcome.exit_code)
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}
