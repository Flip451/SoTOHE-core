<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryInput | enum | modify | Report, CompleteCommand, EmitCompletedCommand, EmitArchivedTrackSubcommand | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryCompletion | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli_driver::telemetry::completion_eligible | free_function | add | fn(subcommand: &str) -> bool | 🔵 | 🔵 |
| cli_driver::telemetry::duration_millis | free_function | add | fn(start: std::time::Instant) -> u64 | 🔵 | 🔵 |
| cli_driver::telemetry::exit_code_value | free_function | add | fn(exit_code: std::process::ExitCode) -> i32 | 🔵 | 🔵 |
| cli_driver::telemetry::items_dir_from_args | free_function | add | fn(args: &[std::ffi::OsString]) -> std::path::PathBuf | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryDriver | primary_adapter | modify | — | 🔵 | 🔵 |

