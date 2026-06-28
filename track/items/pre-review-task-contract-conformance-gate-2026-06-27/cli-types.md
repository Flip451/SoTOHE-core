<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliCommand | enum | modify | Arch, Conventions, Domain, Guard, Hook, Track, Git, Pr, Plan, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, Signal, TaskContract, Demo | 🔵 | 🔵 |
| TaskContractCommand | enum | add | Check, Coverage | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TaskContractCheckArgs | dto | add | — | 🔵 | 🔵 |
| TaskContractCoverageArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::task_contract::detect_active_track_from_branch_cwd | free_function | add | fn() -> Option<String> | 🔵 | 🔵 |
| cli::commands::task_contract::execute | free_function | add | fn(cmd: TaskContractCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::task_contract::execute_task_contract_check | free_function | add | fn(args: TaskContractCheckArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::task_contract::execute_task_contract_coverage | free_function | add | fn(args: TaskContractCoverageArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::task_contract::task_contract_check_core | free_function | add | fn(track_id_opt: Option<String>, layer: Option<String>, items_dir: std::path::PathBuf) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::task_contract::task_contract_coverage_core | free_function | add | fn(track_id_opt: Option<String>, items_dir: std::path::PathBuf) -> std::process::ExitCode | 🔵 | 🔵 |

