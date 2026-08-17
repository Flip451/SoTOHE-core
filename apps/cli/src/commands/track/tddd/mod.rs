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

/// Routes driver failures through the shared CLI error path and reports through
/// the shared outcome emitter. The macro keeps this distinction out of each
/// command handler without adding an un-catalogued production function.
macro_rules! emit_driver_outcome {
    ($outcome:expr, $stdout:expr, $stderr:expr) => {{
        let outcome = $outcome;
        if outcome.stderr.is_some() {
            $crate::commands::track::state_ops::track_driver_outcome_to_result(outcome)
        } else {
            $crate::commands::track::tddd::emit_command_outcome(&outcome, $stdout, $stderr)
        }
    }};
}

pub(super) use emit_driver_outcome;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_command_outcome_success_writes_stdout_and_returns_zero() {
        let outcome = cli_composition::CommandOutcome {
            stdout: Some("ok report".to_owned()),
            stderr: None,
            exit_code: 0,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = emit_command_outcome(&outcome, &mut stdout, &mut stderr).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(String::from_utf8(stdout).unwrap(), "ok report\n");
        assert!(stderr.is_empty());
    }

    #[test]
    fn test_emit_command_outcome_red_report_writes_stdout_and_returns_failure() {
        let outcome = cli_composition::CommandOutcome {
            stdout: Some("## Layer: `usecase`\n🔴 Red".to_owned()),
            stderr: None,
            exit_code: 1,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = emit_command_outcome(&outcome, &mut stdout, &mut stderr).unwrap();
        assert_eq!(code, ExitCode::FAILURE);
        assert!(String::from_utf8(stdout).unwrap().contains("🔴 Red"));
        assert!(stderr.is_empty());
    }
}
