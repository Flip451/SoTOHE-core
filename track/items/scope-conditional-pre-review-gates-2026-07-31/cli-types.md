<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliCommand | enum | modify | Arch, AdrBaseline, Conventions, Domain, Guard, Hook, Maintenance, Track, Git, Pr, Capability, Phase, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template, CodexRuntime, BatchPlan, Demo | 🔵 | 🔵 |
| PhaseCommand | enum | add | Validate, Explain, Enter | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecArgs | dto | modify | — | 🔵 | 🔵 |
| PhaseEnterArgs | dto | add | — | 🔵 | 🔵 |
| PhaseIdArgs | dto | add | — | 🔵 | 🔵 |
| PhaseValidateArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::capability::into_driver_input | free_function | modify | fn(args: CapabilityExecArgs) -> cli_driver::capability::CapabilityExecDriverInput | 🔵 | 🔵 |
| cli::commands::phase::execute | free_function | add | fn(command: PhaseCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::phase::execute_with_driver | free_function | add | fn(command: PhaseCommand, driver: &cli_driver::phase_command::PhaseCommandDriver) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::phase::input_from_command | free_function | add | fn(command: PhaseCommand) -> cli_driver::phase_command::PhaseCommandInput | 🔵 | 🔵 |

