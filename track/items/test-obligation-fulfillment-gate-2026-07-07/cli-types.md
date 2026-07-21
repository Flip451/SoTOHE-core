<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliCommand | enum | modify | Arch, Conventions, Domain, Guard, Hook, Track, Git, Pr, Plan, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template | 🔵 | 🔵 |
| TestObligationSubcommand | enum | add | Derive, Check, Evaluate, Results, BindingsSkeleton | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TestBindingsSkeletonArgs | dto | add | — | 🔵 | 🔵 |
| TestObligationArgs | dto | add | — | 🔵 | 🔵 |
| TestObligationCheckArgs | dto | add | — | 🔵 | 🔵 |
| TestObligationDeriveArgs | dto | add | — | 🔵 | 🔵 |
| TestObligationEvaluateArgs | dto | add | — | 🔵 | 🔵 |
| TestObligationResultsArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::test_obligation::command_context | free_function | add | fn() -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::command_context_with | free_function | add | fn(discover_root: impl FnOnce() -> Result<cli_composition::TestObligationCompositionRoot, crate::CliError>, read_current_branch: impl FnOnce(&cli_composition::TestObligationCompositionRoot) -> Result<String, crate::CliError>) -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::command_root | free_function | add | fn() -> Result<cli_composition::TestObligationCompositionRoot, crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::current_branch | free_function | add | fn(root: &cli_composition::TestObligationCompositionRoot) -> Result<String, crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::dispatch_test_obligation | free_function | add | fn(args: TestObligationArgs, derive: impl FnOnce(&TestObligationDeriveArgs) -> std::process::ExitCode, check: impl FnOnce(&TestObligationCheckArgs) -> std::process::ExitCode, evaluate: impl FnOnce(&TestObligationEvaluateArgs) -> std::process::ExitCode, results: impl FnOnce(&TestObligationResultsArgs) -> std::process::ExitCode, bindings_skeleton: impl FnOnce(&TestBindingsSkeletonArgs) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute | free_function | add | fn(args: TestObligationArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_bindings_skeleton | free_function | add | fn(args: &TestBindingsSkeletonArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_bindings_skeleton_with | free_function | add | fn(args: &TestBindingsSkeletonArgs, build_context: impl FnOnce(Option<&str>) -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError>, handle: impl FnOnce(&cli_composition::TestObligationCompositionRoot, cli_driver::test_obligation::bindings_skeleton::TestBindingsSkeletonInput) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_check | free_function | add | fn(args: &TestObligationCheckArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_check_with | free_function | add | fn(args: &TestObligationCheckArgs, build_context: impl FnOnce(Option<&str>) -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError>, handle: impl FnOnce(&cli_composition::TestObligationCompositionRoot, cli_driver::test_obligation::check::TestObligationCheckInput) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_derive | free_function | add | fn(args: &TestObligationDeriveArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_derive_with | free_function | add | fn(args: &TestObligationDeriveArgs, build_context: impl FnOnce() -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError>, handle: impl FnOnce(&cli_composition::TestObligationCompositionRoot, cli_driver::test_obligation::derive::TestObligationDeriveInput) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_evaluate | free_function | add | fn(args: &TestObligationEvaluateArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_evaluate_with | free_function | add | fn(args: &TestObligationEvaluateArgs, build_context: impl FnOnce() -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError>, handle: impl FnOnce(&cli_composition::TestObligationCompositionRoot, cli_driver::test_obligation::evaluate::TestObligationEvaluateInput) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_results | free_function | add | fn(args: &TestObligationResultsArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_results_with | free_function | add | fn(args: &TestObligationResultsArgs, build_context: impl FnOnce(Option<&str>) -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError>, handle: impl FnOnce(&cli_composition::TestObligationCompositionRoot, cli_driver::test_obligation::results::TestObligationResultsInput) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::failure | free_function | add | fn(error: crate::CliError) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::read_command_branch | free_function | add | fn(explicit_track_id: Option<&str>, read_current_branch: impl FnOnce() -> Result<String, crate::CliError>) -> Result<String, crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::read_only_command_context | free_function | add | fn(explicit_track_id: Option<&str>) -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError> | 🔵 | 🔵 |

