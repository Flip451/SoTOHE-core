//! TDDD CLI commands — domain type signal evaluation and baseline capture.

use std::io::Write;
use std::process::ExitCode;

pub(crate) mod baseline;
pub(crate) mod baseline_graph;
pub(crate) mod catalogue_impl_signals;
pub(crate) mod contract_map;
pub(crate) mod graph;
pub(crate) mod lint;
pub(crate) mod spec_element_hash;

/// Emits a composition outcome to the process streams and preserves its exit code.
pub(super) fn emit_command_outcome<W: Write, E: Write>(
    outcome: &cli_composition::CommandOutcome,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<ExitCode, crate::CliError> {
    if let Some(message) = &outcome.stdout {
        writeln!(stdout, "{message}").map_err(crate::CliError::Io)?;
    }
    if let Some(message) = &outcome.stderr {
        writeln!(stderr, "{message}").map_err(crate::CliError::Io)?;
    }
    Ok(ExitCode::from(outcome.exit_code))
}
