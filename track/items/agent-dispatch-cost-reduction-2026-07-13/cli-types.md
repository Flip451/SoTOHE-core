<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityCommand | enum | reference | Exec | 🔵 | 🔵 |
| ReviewCommand | enum | reference | CodexLocal, ClaudeLocal, Local, FixLocal, CheckApproved, Results, Classify, Files | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecArgs | dto | modify | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::capability::execute | free_function | reference | fn(command: CapabilityCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::capability::into_driver_input | free_function | add | fn(args: CapabilityExecArgs) -> cli_driver::capability::CapabilityExecDriverInput | 🔵 | 🔵 |
| cli::commands::review::execute | free_function | reference | fn(command: ReviewCommand) -> std::process::ExitCode | 🔵 | 🔵 |

