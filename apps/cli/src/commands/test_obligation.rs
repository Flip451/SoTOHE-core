//! CLI subcommands for `sotp test-obligation`.

use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TestObligationCompositionRoot;
use cli_driver::test_obligation::check::TestObligationCheckInput;
use cli_driver::test_obligation::derive::TestObligationDeriveInput;
use cli_driver::test_obligation::evaluate::TestObligationEvaluateInput;
use cli_driver::test_obligation::results::TestObligationResultsInput;

use crate::CliError;
use crate::commands::driver_outcome_to_exit;

/// Arguments for `sotp test-obligation`.
#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct TestObligationArgs {
    /// Test-obligation operation.
    #[command(subcommand)]
    pub subcommand: TestObligationSubcommand,
}

impl TestObligationArgs {
    /// Builds [`TestObligationArgs`].
    #[must_use]
    pub fn new(subcommand: TestObligationSubcommand) -> Self {
        Self { subcommand }
    }
}

/// Concrete `sotp test-obligation` subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum TestObligationSubcommand {
    /// Derive obligation artifacts from the current track catalogues.
    Derive(TestObligationDeriveArgs),
    /// Check deterministic obligation bindings.
    Check(TestObligationCheckArgs),
    /// Evaluate fulfillment and waiver verdicts.
    Evaluate(TestObligationEvaluateArgs),
    /// Display cached obligation-gate results.
    Results(TestObligationResultsArgs),
}

/// Arguments for `sotp test-obligation derive`.
#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct TestObligationDeriveArgs {
    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,
}

/// Arguments for `sotp test-obligation check`.
#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct TestObligationCheckArgs {
    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,
}

/// Arguments for `sotp test-obligation evaluate`.
#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct TestObligationEvaluateArgs {
    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,
}

/// Arguments for `sotp test-obligation results`.
#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct TestObligationResultsArgs {
    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,
}

/// Execute `sotp test-obligation <subcommand>`.
pub fn execute(args: TestObligationArgs) -> ExitCode {
    match args.subcommand {
        TestObligationSubcommand::Derive(args) => execute_derive(&args),
        TestObligationSubcommand::Check(args) => execute_check(&args),
        TestObligationSubcommand::Evaluate(args) => execute_evaluate(&args),
        TestObligationSubcommand::Results(args) => execute_results(&args),
    }
}

fn command_context() -> Result<(TestObligationCompositionRoot, String), CliError> {
    let root =
        TestObligationCompositionRoot::discover().map_err(|e| CliError::Message(e.to_string()))?;
    let current_branch = root.current_branch().map_err(|e| CliError::Message(e.to_string()))?;
    Ok((root, current_branch))
}

fn execute_derive(args: &TestObligationDeriveArgs) -> ExitCode {
    let (root, current_branch) = match command_context() {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input = match TestObligationDeriveInput::try_from_raw(args.track_id.clone(), current_branch)
    {
        Ok(input) => input,
        Err(message) => return failure(CliError::Message(message)),
    };
    driver_outcome_to_exit(root.derive_handler().handle(input))
}

fn execute_check(args: &TestObligationCheckArgs) -> ExitCode {
    let (root, current_branch) = match command_context() {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input = match TestObligationCheckInput::try_from_raw(args.track_id.clone(), current_branch)
    {
        Ok(input) => input,
        Err(message) => return failure(CliError::Message(message)),
    };
    driver_outcome_to_exit(root.check_handler().handle(input))
}

fn execute_evaluate(args: &TestObligationEvaluateArgs) -> ExitCode {
    let (root, current_branch) = match command_context() {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input =
        match TestObligationEvaluateInput::try_from_raw(args.track_id.clone(), current_branch) {
            Ok(input) => input,
            Err(message) => return failure(CliError::Message(message)),
        };
    driver_outcome_to_exit(root.evaluate_handler().handle(input))
}

fn execute_results(args: &TestObligationResultsArgs) -> ExitCode {
    let (root, current_branch) = match command_context() {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input =
        match TestObligationResultsInput::try_from_raw(args.track_id.clone(), current_branch) {
            Ok(input) => input,
            Err(message) => return failure(CliError::Message(message)),
        };
    driver_outcome_to_exit(root.results_handler().handle(input))
}

fn failure(error: CliError) -> ExitCode {
    eprintln!("{error}");
    error.exit_code()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: TestObligationSubcommand,
    }

    fn parse(args: &[&str]) -> TestObligationSubcommand {
        TestCli::parse_from(args).cmd
    }

    #[test]
    fn test_parse_derive_without_track_id() {
        match parse(&["test-obligation", "derive"]) {
            TestObligationSubcommand::Derive(args) => assert!(args.track_id.is_none()),
            other => panic!("expected derive, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_check_with_track_id() {
        match parse(&["test-obligation", "check", "--track-id", "example-track"]) {
            TestObligationSubcommand::Check(args) => {
                assert_eq!(args.track_id.as_deref(), Some("example-track"));
            }
            other => panic!("expected check, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_evaluate_with_track_id() {
        match parse(&["test-obligation", "evaluate", "--track-id", "example-track"]) {
            TestObligationSubcommand::Evaluate(args) => {
                assert_eq!(args.track_id.as_deref(), Some("example-track"));
            }
            other => panic!("expected evaluate, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_results_with_track_id() {
        match parse(&["test-obligation", "results", "--track-id", "example-track"]) {
            TestObligationSubcommand::Results(args) => {
                assert_eq!(args.track_id.as_deref(), Some("example-track"));
            }
            other => panic!("expected results, got {other:?}"),
        }
    }
}
