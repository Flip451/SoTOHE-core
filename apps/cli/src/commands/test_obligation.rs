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
    dispatch_test_obligation(
        args,
        execute_derive,
        execute_check,
        execute_evaluate,
        execute_results,
        execute_bindings_skeleton,
    )
}

fn dispatch_test_obligation(
    args: TestObligationArgs,
    derive: impl FnOnce(&TestObligationDeriveArgs) -> ExitCode,
    check: impl FnOnce(&TestObligationCheckArgs) -> ExitCode,
    evaluate: impl FnOnce(&TestObligationEvaluateArgs) -> ExitCode,
    results: impl FnOnce(&TestObligationResultsArgs) -> ExitCode,
    bindings_skeleton: impl FnOnce(&TestBindingsSkeletonArgs) -> ExitCode,
) -> ExitCode {
    match args.subcommand {
        TestObligationSubcommand::Derive(args) => derive(&args),
        TestObligationSubcommand::Check(args) => check(&args),
        TestObligationSubcommand::Evaluate(args) => evaluate(&args),
        TestObligationSubcommand::Results(args) => results(&args),
        TestObligationSubcommand::BindingsSkeleton(args) => bindings_skeleton(&args),
    }
}

fn command_context() -> Result<(TestObligationCompositionRoot, String), CliError> {
    command_context_with(command_root, current_branch)
}

fn command_context_with(
    discover_root: impl FnOnce() -> Result<TestObligationCompositionRoot, CliError>,
    read_current_branch: impl FnOnce(&TestObligationCompositionRoot) -> Result<String, CliError>,
) -> Result<(TestObligationCompositionRoot, String), CliError> {
    let root = discover_root()?;
    let current_branch = read_current_branch(&root)?;
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
    execute_derive_with(args, command_context, |root, input| {
        driver_outcome_to_exit(root.derive_handler().handle(input))
    })
}

fn execute_derive_with(
    args: &TestObligationDeriveArgs,
    build_context: impl FnOnce() -> Result<(TestObligationCompositionRoot, String), CliError>,
    handle: impl FnOnce(&TestObligationCompositionRoot, TestObligationDeriveInput) -> ExitCode,
) -> ExitCode {
    let (root, current_branch) = match build_context() {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input = match TestObligationDeriveInput::try_from_raw(args.track_id.clone(), current_branch)
    {
        Ok(input) => input,
        Err(message) => return failure(CliError::Message(message)),
    };
    handle(&root, input)
}

fn execute_check(args: &TestObligationCheckArgs) -> ExitCode {
    execute_check_with(args, read_only_command_context, |root, input| {
        driver_outcome_to_exit(root.check_handler().handle(input))
    })
}

fn execute_check_with(
    args: &TestObligationCheckArgs,
    build_context: impl FnOnce(
        Option<&str>,
    ) -> Result<(TestObligationCompositionRoot, String), CliError>,
    handle: impl FnOnce(&TestObligationCompositionRoot, TestObligationCheckInput) -> ExitCode,
) -> ExitCode {
    let (root, current_branch) = match build_context(args.track_id.as_deref()) {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input = match TestObligationCheckInput::try_from_raw(args.track_id.clone(), current_branch)
    {
        Ok(input) => input,
        Err(message) => return failure(CliError::Message(message)),
    };
    handle(&root, input)
}

fn execute_evaluate(args: &TestObligationEvaluateArgs) -> ExitCode {
    execute_evaluate_with(args, command_context, |root, input| {
        driver_outcome_to_exit(root.evaluate_handler().handle(input))
    })
}

fn execute_evaluate_with(
    args: &TestObligationEvaluateArgs,
    build_context: impl FnOnce() -> Result<(TestObligationCompositionRoot, String), CliError>,
    handle: impl FnOnce(&TestObligationCompositionRoot, TestObligationEvaluateInput) -> ExitCode,
) -> ExitCode {
    let (root, current_branch) = match build_context() {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input =
        match TestObligationEvaluateInput::try_from_raw(args.track_id.clone(), current_branch) {
            Ok(input) => input,
            Err(message) => return failure(CliError::Message(message)),
        };
    handle(&root, input)
}

fn execute_results(args: &TestObligationResultsArgs) -> ExitCode {
    execute_results_with(args, read_only_command_context, |root, input| {
        driver_outcome_to_exit(root.results_handler().handle(input))
    })
}

fn execute_results_with(
    args: &TestObligationResultsArgs,
    build_context: impl FnOnce(
        Option<&str>,
    ) -> Result<(TestObligationCompositionRoot, String), CliError>,
    handle: impl FnOnce(&TestObligationCompositionRoot, TestObligationResultsInput) -> ExitCode,
) -> ExitCode {
    let (root, current_branch) = match build_context(args.track_id.as_deref()) {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input =
        match TestObligationResultsInput::try_from_raw(args.track_id.clone(), current_branch) {
            Ok(input) => input,
            Err(message) => return failure(CliError::Message(message)),
        };
    handle(&root, input)
}

fn execute_bindings_skeleton(args: &TestBindingsSkeletonArgs) -> ExitCode {
    execute_bindings_skeleton_with(args, read_only_command_context, |root, input| {
        driver_outcome_to_exit(root.bindings_skeleton_handler().handle(input))
    })
}

fn execute_bindings_skeleton_with(
    args: &TestBindingsSkeletonArgs,
    build_context: impl FnOnce(
        Option<&str>,
    ) -> Result<(TestObligationCompositionRoot, String), CliError>,
    handle: impl FnOnce(&TestObligationCompositionRoot, TestBindingsSkeletonInput) -> ExitCode,
) -> ExitCode {
    let (root, current_branch) = match build_context(args.track_id.as_deref()) {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    let input = match TestBindingsSkeletonInput::try_from_raw(args.track_id.clone(), current_branch)
    {
        Ok(input) => input,
        Err(message) => return failure(CliError::Message(message)),
    };
    handle(&root, input)
}

fn failure(error: CliError) -> ExitCode {
    eprintln!("{error}");
    error.exit_code()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

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

    const OBLIGATION_TRACK_ID: &str = "test-obligation-fulfillment-gate-2026-07-07";

    fn source_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn source_track_dir() -> PathBuf {
        source_root().join("track/items").join(OBLIGATION_TRACK_ID)
    }

    fn copy_track_fixture_file(workspace_root: &Path, name: &str) {
        let target = workspace_root.join("track/items").join(OBLIGATION_TRACK_ID).join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(source_track_dir().join(name), target).unwrap();
    }

    fn fixture_workspace(track_files: &[&str]) -> tempfile::TempDir {
        let temp_root = source_root().canonicalize().unwrap().join("tmp");
        let temp = tempfile::Builder::new()
            .prefix("sotp-cli-test-obligation-")
            .tempdir_in(temp_root)
            .unwrap();
        let workspace_root = temp.path().join("workspace");
        let config_path = workspace_root.join(".harness/config/test-obligation-rules.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::copy(source_root().join(".harness/config/test-obligation-rules.json"), config_path)
            .unwrap();
        for name in track_files {
            copy_track_fixture_file(&workspace_root, name);
        }
        temp
    }

    fn fixture_root(workspace_root: &Path) -> TestObligationCompositionRoot {
        TestObligationCompositionRoot::new(
            workspace_root.to_path_buf(),
            workspace_root.join(".harness/config/test-obligation-rules.json"),
        )
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
    fn test_track_id_selects_identity_without_narrowing_gate_scope() {
        for subcommand in ["check", "evaluate", "results", "bindings-skeleton"] {
            let parsed = TestCli::try_parse_from([
                "test-obligation",
                subcommand,
                "--track-id",
                "detached-track",
            ])
            .unwrap()
            .cmd;
            let track_id = match parsed {
                TestObligationSubcommand::Check(args) => args.track_id,
                TestObligationSubcommand::Evaluate(args) => args.track_id,
                TestObligationSubcommand::Results(args) => args.track_id,
                TestObligationSubcommand::BindingsSkeleton(args) => args.track_id,
                TestObligationSubcommand::Derive(_) => None,
            };
            assert_eq!(track_id.as_deref(), Some("detached-track"));
        }

        for subcommand in ["derive", "check", "evaluate", "results"] {
            for prohibited in ["--lenient", "--force"] {
                assert!(
                    TestCli::try_parse_from(["test-obligation", subcommand, prohibited]).is_err()
                );
            }
        }

        let branch = read_command_branch(Some("detached-track"), || {
            panic!("track identity must avoid branch-dependent resolution")
        })
        .unwrap();
        assert_eq!(branch, BRANCH_NOT_READ);
    }

    #[test]
    fn test_parser_rejects_scope_selection_flags_for_every_gate_command() {
        for subcommand in ["derive", "check", "evaluate", "results", "bindings-skeleton"] {
            for prohibited in ["--context", "--layer", "--scope"] {
                assert!(
                    TestCli::try_parse_from(["test-obligation", subcommand, prohibited]).is_err()
                );
            }
        }
    }

    #[test]
    fn test_gate_args_accept_track_identity_but_reject_scope_and_override_flags() {
        for subcommand in ["derive", "check", "evaluate", "results", "bindings-skeleton"] {
            assert!(
                TestCli::try_parse_from([
                    "test-obligation",
                    subcommand,
                    "--track-id",
                    "detached-track",
                ])
                .is_ok()
            );

            for prohibited in ["--context", "--layer", "--scope", "--lenient", "--force"] {
                assert!(
                    TestCli::try_parse_from(["test-obligation", subcommand, prohibited]).is_err(),
                    "{subcommand} must not accept {prohibited}"
                );
            }
        }

        let branch = read_command_branch(Some("detached-track"), || {
            panic!("an explicit track identity must not need branch discovery")
        })
        .unwrap();
        assert_eq!(branch, BRANCH_NOT_READ);
    }

    #[test]
    fn test_execute_dispatches_each_subcommand_to_its_matching_handler() {
        let cases = [
            (
                TestObligationSubcommand::Derive(TestObligationDeriveArgs { track_id: None }),
                ExitCode::from(11),
            ),
            (
                TestObligationSubcommand::Check(TestObligationCheckArgs { track_id: None }),
                ExitCode::from(12),
            ),
            (
                TestObligationSubcommand::Evaluate(TestObligationEvaluateArgs { track_id: None }),
                ExitCode::from(13),
            ),
            (
                TestObligationSubcommand::Results(TestObligationResultsArgs { track_id: None }),
                ExitCode::from(14),
            ),
            (
                TestObligationSubcommand::BindingsSkeleton(TestBindingsSkeletonArgs {
                    track_id: None,
                }),
                ExitCode::from(15),
            ),
        ];

        for (subcommand, expected) in cases {
            let exit = dispatch_test_obligation(
                TestObligationArgs::new(subcommand),
                |_| ExitCode::from(11),
                |_| ExitCode::from(12),
                |_| ExitCode::from(13),
                |_| ExitCode::from(14),
                |_| ExitCode::from(15),
            );
            assert_eq!(exit, expected);
        }
    }

    #[test]
    fn test_command_root_discovers_root_and_read_only_context_uses_explicit_identity() {
        let root = command_root().unwrap();
        assert!(root.workspace_root.is_absolute());

        let (_root, explicit_branch) = read_only_command_context(Some("detached-track")).unwrap();
        assert_eq!(explicit_branch, BRANCH_NOT_READ);
    }

    #[test]
    fn test_command_root_workspace_discovery_returns_configured_composition_root() {
        let root = command_root().unwrap();

        assert_eq!(
            root.config_path,
            root.workspace_root.join(".harness/config/test-obligation-rules.json")
        );
        assert!(root.config_path.is_absolute());
    }

    #[test]
    fn test_command_context_with_root_and_active_branch_returns_pair() {
        let expected_root = TestObligationCompositionRoot::new(
            PathBuf::from("/workspace"),
            PathBuf::from("/workspace/.harness/config/test-obligation-rules.json"),
        );
        let (root, branch) =
            command_context_with(|| Ok(expected_root), |_| Ok("track/example-track".to_owned()))
                .unwrap();

        assert_eq!(root.workspace_root, PathBuf::from("/workspace"));
        assert_eq!(branch, "track/example-track");
    }

    #[test]
    fn test_command_context_discovery_failure_propagates_error() {
        let result = command_context_with(
            || Err(CliError::Message("workspace discovery failed".to_owned())),
            |_| -> Result<String, CliError> {
                panic!("branch must not be read after discovery fails")
            },
        );

        assert!(matches!(
            result,
            Err(CliError::Message(message)) if message == "workspace discovery failed"
        ));
    }

    #[test]
    fn test_command_context_uses_active_branch_for_existence_based_check_scope() {
        let temp = fixture_workspace(&[]);
        let workspace_root = temp.path().join("workspace");

        let exit = execute_check_with(
            &TestObligationCheckArgs { track_id: None },
            |explicit_track_id| {
                assert!(explicit_track_id.is_none());
                command_context_with(
                    || Ok(fixture_root(&workspace_root)),
                    |_| Ok(format!("track/{OBLIGATION_TRACK_ID}")),
                )
            },
            |root, input| {
                assert!(input.track_id().is_none());
                assert_eq!(input.current_branch().as_str(), format!("track/{OBLIGATION_TRACK_ID}"));
                driver_outcome_to_exit(root.check_handler().handle(input))
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_execute_gate_commands_delegate_to_composition_root() {
        let missing_track = Some("test-obligation-missing-track".to_owned());

        assert_eq!(
            execute_derive(&TestObligationDeriveArgs { track_id: missing_track.clone() }),
            ExitCode::FAILURE
        );
        assert_eq!(
            execute_check(&TestObligationCheckArgs { track_id: missing_track.clone() }),
            ExitCode::SUCCESS
        );
        assert_eq!(
            execute_evaluate(&TestObligationEvaluateArgs { track_id: missing_track.clone() }),
            ExitCode::FAILURE
        );
        assert_eq!(
            execute_bindings_skeleton(&TestBindingsSkeletonArgs {
                track_id: missing_track.clone()
            }),
            ExitCode::FAILURE
        );
        assert_eq!(
            execute_results(&TestObligationResultsArgs { track_id: missing_track }),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn test_execute_derive_valid_track_materializes_obligations() {
        let temp = fixture_workspace(&[
            "spec.json",
            "domain-types.json",
            "usecase-types.json",
            "infrastructure-types.json",
            "cli_composition-types.json",
            "cli_driver-types.json",
            "cli-types.json",
        ]);
        let workspace_root = temp.path().join("workspace");
        let expected_artifact =
            workspace_root.join("track/items").join(OBLIGATION_TRACK_ID).join("obligations.json");

        let exit = execute_derive_with(
            &TestObligationDeriveArgs { track_id: Some(OBLIGATION_TRACK_ID.to_owned()) },
            || Ok((fixture_root(&workspace_root), format!("track/{OBLIGATION_TRACK_ID}"))),
            |root, input| driver_outcome_to_exit(root.derive_handler().handle(input)),
        );

        assert_eq!(exit, ExitCode::SUCCESS);
        assert!(expected_artifact.is_file());
    }

    #[test]
    fn test_derive_then_check_real_catalogues_does_not_report_stale_obligations() {
        let temp = fixture_workspace(&[
            "spec.json",
            "domain-types.json",
            "usecase-types.json",
            "infrastructure-types.json",
            "cli_composition-types.json",
            "cli_driver-types.json",
            "cli-types.json",
            "test-bindings.json",
        ]);
        let workspace_root = temp.path().join("workspace");

        let derive_exit = execute_derive_with(
            &TestObligationDeriveArgs { track_id: Some(OBLIGATION_TRACK_ID.to_owned()) },
            || Ok((fixture_root(&workspace_root), format!("track/{OBLIGATION_TRACK_ID}"))),
            |root, input| driver_outcome_to_exit(root.derive_handler().handle(input)),
        );
        assert_eq!(derive_exit, ExitCode::SUCCESS);

        let check_input = TestObligationCheckInput::try_from_raw(
            Some(OBLIGATION_TRACK_ID.to_owned()),
            BRANCH_NOT_READ.to_owned(),
        )
        .unwrap();
        let outcome = fixture_root(&workspace_root).check_handler().handle(check_input);
        let diagnostic = outcome.stderr.unwrap_or_default();

        assert!(
            !diagnostic.contains("stale obligations artifact"),
            "derive → check must share the persisted-obligation construction: {diagnostic}"
        );
    }

    #[test]
    fn test_execute_check_partial_artifact_scope_returns_failure() {
        let temp = fixture_workspace(&["obligations.json"]);
        let workspace_root = temp.path().join("workspace");

        let exit = execute_check_with(
            &TestObligationCheckArgs { track_id: Some(OBLIGATION_TRACK_ID.to_owned()) },
            |explicit_track_id| {
                assert_eq!(explicit_track_id, Some(OBLIGATION_TRACK_ID));
                Ok((fixture_root(&workspace_root), BRANCH_NOT_READ.to_owned()))
            },
            |root, input| driver_outcome_to_exit(root.check_handler().handle(input)),
        );

        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_execute_evaluate_valid_input_dispatches_to_evaluation_handler() {
        let root = TestObligationCompositionRoot::new(
            PathBuf::from("/workspace"),
            PathBuf::from("/workspace/.harness/config/test-obligation-rules.json"),
        );
        let exit = execute_evaluate_with(
            &TestObligationEvaluateArgs { track_id: Some(OBLIGATION_TRACK_ID.to_owned()) },
            || Ok((root, format!("track/{OBLIGATION_TRACK_ID}"))),
            |root, input| {
                assert_eq!(root.workspace_root, PathBuf::from("/workspace"));
                assert_eq!(
                    input.track_id().map(|track_id| track_id.as_ref()),
                    Some(OBLIGATION_TRACK_ID)
                );
                assert_eq!(input.current_branch().as_str(), format!("track/{OBLIGATION_TRACK_ID}"));
                ExitCode::SUCCESS
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_execute_bindings_skeleton_valid_track_returns_schema_pure_draft() {
        let temp = fixture_workspace(&["obligations.json"]);
        let workspace_root = temp.path().join("workspace");
        let exit = execute_bindings_skeleton_with(
            &TestBindingsSkeletonArgs { track_id: Some(OBLIGATION_TRACK_ID.to_owned()) },
            |explicit_track_id| {
                assert_eq!(explicit_track_id, Some(OBLIGATION_TRACK_ID));
                Ok((fixture_root(&workspace_root), BRANCH_NOT_READ.to_owned()))
            },
            |root, input| {
                let outcome = root.bindings_skeleton_handler().handle(input);
                let draft: serde_json::Value =
                    serde_json::from_str(outcome.stdout.as_deref().unwrap()).unwrap();
                assert_eq!(draft["track_id"], OBLIGATION_TRACK_ID);
                assert_eq!(draft["records"][0]["kind"], "fulfillment");
                assert_eq!(draft["records"][0]["tests"][0]["layer"], "TODO_LAYER");
                driver_outcome_to_exit(outcome)
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_execute_results_explicit_track_returns_lane_summary_without_branch_read() {
        let temp = fixture_workspace(&[
            "obligations.json",
            "test-bindings.json",
            "obligation-fulfillment-cache.json",
            "waiver-cache.json",
            "spec.json",
            "domain-types.json",
            "usecase-types.json",
            "infrastructure-types.json",
            "cli_composition-types.json",
            "cli_driver-types.json",
            "cli-types.json",
            "task-contract.json",
            "impl-plan.json",
        ]);
        let workspace_root = temp.path().join("workspace");
        let exit = execute_results_with(
            &TestObligationResultsArgs { track_id: Some(OBLIGATION_TRACK_ID.to_owned()) },
            |explicit_track_id| {
                assert_eq!(explicit_track_id, Some(OBLIGATION_TRACK_ID));
                let branch = read_command_branch(explicit_track_id, || {
                    panic!("explicit results track must not read the current branch")
                })?;
                assert_eq!(branch, BRANCH_NOT_READ);
                Ok((fixture_root(&workspace_root), branch))
            },
            |root, input| {
                let outcome = root.results_handler().handle(input);
                let stdout = outcome.stdout.as_deref().unwrap();
                assert!(stdout.contains("Fulfillment:"), "expected lane summary: {stdout}");
                assert!(stdout.contains("records="));
                driver_outcome_to_exit(outcome)
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_execute_results_without_track_id_resolves_current_branch() {
        let temp = fixture_workspace(&[
            "obligations.json",
            "test-bindings.json",
            "obligation-fulfillment-cache.json",
            "waiver-cache.json",
        ]);
        let workspace_root = temp.path().join("workspace");

        let exit = execute_results_with(
            &TestObligationResultsArgs { track_id: None },
            |explicit_track_id| {
                assert!(explicit_track_id.is_none());
                Ok((fixture_root(&workspace_root), format!("track/{OBLIGATION_TRACK_ID}")))
            },
            |root, input| {
                assert_eq!(
                    input.track_id().map(|track_id| track_id.as_ref()),
                    Some(OBLIGATION_TRACK_ID)
                );
                let outcome = root.results_handler().handle(input);
                assert_eq!(outcome.exit_code, 0);
                driver_outcome_to_exit(outcome)
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_execute_routes_bindings_skeleton_through_real_command_path() {
        let exit = execute(TestObligationArgs::new(TestObligationSubcommand::BindingsSkeleton(
            TestBindingsSkeletonArgs { track_id: Some("test-obligation-missing-track".to_owned()) },
        )));

        assert_eq!(exit, ExitCode::FAILURE);
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
