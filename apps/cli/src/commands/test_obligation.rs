//! CLI subcommands for `sotp test-obligation`.

use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TestObligationCompositionRoot;
use cli_driver::test_obligation::bindings_skeleton::TestBindingsSkeletonInput;
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
    /// Print a schema-shaped test-bindings draft (TODO placeholder tests) to stdout.
    BindingsSkeleton(TestBindingsSkeletonArgs),
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

/// Arguments for `sotp test-obligation bindings-skeleton`.
#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct TestBindingsSkeletonArgs {
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
        TestObligationSubcommand::BindingsSkeleton(args) => execute_bindings_skeleton(&args),
    }
}

fn command_context() -> Result<(TestObligationCompositionRoot, String), CliError> {
    let root = command_root()?;
    let current_branch = current_branch(&root)?;
    Ok((root, current_branch))
}

fn read_only_command_context(
    explicit_track_id: Option<&str>,
) -> Result<(TestObligationCompositionRoot, String), CliError> {
    let root = command_root()?;
    let current_branch = read_command_branch(explicit_track_id, || current_branch(&root))?;
    Ok((root, current_branch))
}

fn command_root() -> Result<TestObligationCompositionRoot, CliError> {
    let root =
        TestObligationCompositionRoot::discover().map_err(|e| CliError::Message(e.to_string()))?;
    Ok(root)
}

fn current_branch(root: &TestObligationCompositionRoot) -> Result<String, CliError> {
    root.current_branch().map_err(|e| CliError::Message(e.to_string()))
}

/// Diagnostic placeholder recorded instead of the git branch when an explicit
/// `--track-id` fully determines the target track (read-only commands stay
/// usable on detached HEAD this way). Deliberately not `track/<id>`-shaped so
/// branch-based guards can never mistake it for the active track branch.
const BRANCH_NOT_READ: &str = "(branch not read: explicit --track-id)";

fn read_command_branch(
    explicit_track_id: Option<&str>,
    read_current_branch: impl FnOnce() -> Result<String, CliError>,
) -> Result<String, CliError> {
    match explicit_track_id {
        Some(_) => Ok(BRANCH_NOT_READ.to_owned()),
        None => read_current_branch(),
    }
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
    let (root, current_branch) = match read_only_command_context(args.track_id.as_deref()) {
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
    let (root, current_branch) = match read_only_command_context(args.track_id.as_deref()) {
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

fn execute_bindings_skeleton(args: &TestBindingsSkeletonArgs) -> ExitCode {
    let (root, current_branch) = match read_only_command_context(args.track_id.as_deref()) {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input = match TestBindingsSkeletonInput::try_from_raw(args.track_id.clone(), current_branch)
    {
        Ok(input) => input,
        Err(message) => return failure(CliError::Message(message)),
    };
    driver_outcome_to_exit(root.bindings_skeleton_handler().handle(input))
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

    #[test]
    fn test_parse_bindings_skeleton_with_track_id() {
        match parse(&["test-obligation", "bindings-skeleton", "--track-id", "example-track"]) {
            TestObligationSubcommand::BindingsSkeleton(args) => {
                assert_eq!(args.track_id.as_deref(), Some("example-track"));
            }
            other => panic!("expected bindings-skeleton, got {other:?}"),
        }
    }

    #[test]
    fn test_read_command_branch_with_track_id_does_not_read_current_branch() {
        let branch = read_command_branch(Some("example-track"), || -> Result<String, CliError> {
            panic!("current branch should not be read for explicit track id");
        })
        .unwrap();

        // The placeholder must not look like a real `track/<id>` branch, so
        // branch-based guards fail closed instead of trusting a forged value.
        assert_eq!(branch, BRANCH_NOT_READ);
        assert!(!branch.starts_with("track/"));
    }

    #[test]
    fn test_read_command_branch_without_track_id_reads_current_branch() {
        let branch = read_command_branch(None, || Ok("track/current-track".to_owned())).unwrap();

        assert_eq!(branch, "track/current-track");
    }

    #[test]
    fn test_failure_maps_cli_error_to_failure_exit_code() {
        let exit_code = failure(CliError::Message("test failure".to_owned()));

        assert_eq!(exit_code, ExitCode::FAILURE);
    }
}
