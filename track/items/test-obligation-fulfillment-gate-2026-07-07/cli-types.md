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
| cli::commands::test_obligation::command_root | free_function | add | fn() -> Result<cli_composition::TestObligationCompositionRoot, crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::current_branch | free_function | add | fn(root: &cli_composition::TestObligationCompositionRoot) -> Result<String, crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::execute | free_function | add | fn(args: TestObligationArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_bindings_skeleton | free_function | add | fn(args: &TestBindingsSkeletonArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_check | free_function | add | fn(args: &TestObligationCheckArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_derive | free_function | add | fn(args: &TestObligationDeriveArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_evaluate | free_function | add | fn(args: &TestObligationEvaluateArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::execute_results | free_function | add | fn(args: &TestObligationResultsArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::failure | free_function | add | fn(error: crate::CliError) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::test_obligation::read_command_branch | free_function | add | fn(explicit_track_id: Option<&str>, read_current_branch: impl FnOnce() -> Result<String, crate::CliError>) -> Result<String, crate::CliError> | 🔵 | 🔵 |
| cli::commands::test_obligation::read_only_command_context | free_function | add | fn(explicit_track_id: Option<&str>) -> Result<(cli_composition::TestObligationCompositionRoot, String), crate::CliError> | 🔵 | 🔵 |

