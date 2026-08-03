<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogCommand | enum | reference | Init, Add, Import, Cite, Check | 🔵 | 🔵 |
| CatalogueLintCommand | enum | reference | CheckActiveTrack | 🔵 | 🔵 |
| TrackCommand | enum | modify | Archive, Transition, Branch, Resolve, Views, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, TypeGraph, BaselineGraph, ContractMap, SpecElementHash, BaselineCapture, FixpointResolve, SetCommitHash, Lint, CatalogueImplSignals, SwitchBase | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::command_identity_from_args | free_function | add | fn(args: &[std::ffi::OsString]) -> String | 🔵 | 🔵 |
| cli::run_cli | free_function | modify | fn(cli: Cli, dry_execute: fn(commands::dry::DryCommand) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::run_cli_with | free_function | modify | fn(cli: Cli, dry_execute: fn(commands::dry::DryCommand) -> std::process::ExitCode, ref_verify_execute: fn(commands::ref_verify::RefVerifyCommand) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::telemetry_completion_eligible | free_function | add | fn(subcommand: &str) -> bool | 🔵 | 🔵 |
| cli::telemetry_track_display_only | free_function | add | fn(subcommand: &str) -> bool | 🔵 | 🔵 |
