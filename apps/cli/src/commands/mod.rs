//! CLI subcommand definitions.

use std::process::ExitCode;
use std::time::Duration;

use cli_composition::{CommandOutcome, CompositionError};

/// Polling interval for subprocess completion checks in CLI command adapters.
///
/// Shared by all CLI subcommand adapters (`plan`, `review`) that poll Codex or Claude
/// subprocesses for completion (ADR D4 / AC-05). Intentionally separate from the
/// infrastructure-layer constant — coincidental cross-layer equality, not a shared dependency.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(50);

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
