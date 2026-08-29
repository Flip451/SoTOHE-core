<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliCommand | enum | modify | Arch, AdrBaseline, Conventions, Domain, Guard, Hook, Maintenance, Track, Git, Pr, Capability, Phase, Review, File, GateOutput, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template, CodexRuntime, BatchPlan | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateNameArg | dto | add | — | 🔵 | 🔵 |
| GateOutputArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::gate_output::execute | free_function | add | fn(args: GateOutputArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::gate_output::exit_code_from_u8 | free_function | add | fn(code: u8) -> std::process::ExitCode | 🔵 | 🔵 |

