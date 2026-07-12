<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityCommand | enum | add | Exec | 🔵 | 🔵 |
| CliCommand | enum | modify | Arch, Conventions, Domain, Guard, Hook, Track, Git, Pr, Capability, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::capability::execute | free_function | add | fn(command: CapabilityCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::capability::execute_exec | free_function | add | fn(args: CapabilityExecArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::capability::execute_with | free_function | add | fn(command: CapabilityCommand, execute_exec: impl FnOnce(CapabilityExecArgs) -> std::process::ExitCode) -> std::process::ExitCode | 🔵 | 🔵 |

