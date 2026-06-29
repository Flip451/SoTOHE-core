//! CLI subcommand definitions.

use std::process::ExitCode;

use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::CommandOutcome as DriverOutcome;

pub mod arch;
pub mod conventions;
pub mod domain;
pub mod dry;
pub mod file;
pub mod git;
pub mod guard;
pub mod hook;
pub mod plan;
pub mod pr;
pub mod ref_verify;
pub mod review;
pub mod semantic_dup;
pub mod signal;
pub mod task_contract;
pub mod telemetry;
pub mod track;
pub mod verify;
#[cfg(test)]
pub mod verify_catalogue_spec_refs;

/// Convert a `CliApp` result into an `ExitCode`, printing stdout/stderr.
///
/// On `Ok(outcome)`: prints stdout (no trailing newline added — the content
/// already includes one when expected) and stderr, then returns the exit code.
/// On `Err(e)`: prints the error message to stderr and returns `ExitCode::FAILURE`.
pub(crate) fn outcome_to_exit(result: Result<CommandOutcome, CompositionError>) -> ExitCode {
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
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Convert a driver `CommandOutcome` directly into an `ExitCode`, printing stdout/stderr.
///
/// Used by call sites that use the new `<Name>CompositionRoot::new().<name>_driver().handle(...)`
/// pattern, which returns `cli_driver::CommandOutcome` directly (not wrapped in `Result`).
pub(crate) fn driver_outcome_to_exit(outcome: DriverOutcome) -> ExitCode {
    if let Some(stdout) = outcome.stdout {
        println!("{stdout}");
    }
    if let Some(stderr) = outcome.stderr {
        eprintln!("{stderr}");
    }
    ExitCode::from(outcome.exit_code)
}
