<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliCommand | enum | modify | Arch, AdrBaseline, Conventions, Domain, Guard, Hook, Maintenance, Track, Git, Pr, Capability, Phase, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template, CodexRuntime, BatchPlan, Demo | 🔵 | 🔵 |
| PhaseCommand | enum | add | Validate, Explain, Enter | 🔵 | 🔵 |
| RefVerifyCheckChainArg | enum | add | Chain1, Chain2 | 🔵 | 🔵 |
| ReviewCheckRoundArg | enum | add | Final | 🔵 | 🔵 |
| ReviewCommand | enum | modify | Local, FixLocal, CheckApproved, CheckZeroFindings, Results, Classify, Files | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecArgs | dto | modify | — | 🔵 | 🔵 |
| CheckApprovedArgs | dto | modify | — | 🔵 | 🔵 |
| CheckZeroFindingsArgs | dto | add | — | 🔵 | 🔵 |
| PhaseEnterArgs | dto | add | — | 🔵 | 🔵 |
| PhaseIdArgs | dto | add | — | 🔵 | 🔵 |
| PhaseValidateArgs | dto | add | — | 🔵 | 🔵 |
| ReviewCheckApprovedArgs | dto | add | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::capability::into_driver_input | free_function | modify | fn(args: CapabilityExecArgs) -> cli_driver::capability::CapabilityExecDriverInput | 🔵 | 🔵 |
| cli::commands::phase::execute | free_function | add | fn(command: PhaseCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::phase::execute_with_driver | free_function | add | fn(command: PhaseCommand, driver: &cli_driver::phase_command::PhaseCommandDriver) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::phase::input_from_command | free_function | add | fn(command: PhaseCommand) -> cli_driver::phase_command::PhaseCommandInput | 🔵 | 🔵 |
| cli::commands::ref_verify::execute_check_approved_with_driver | free_function | add | fn(args: &CheckApprovedArgs, driver: &cli_driver::ref_verify::RefVerifyDriver) -> std::process::ExitCode | 🔵 | 🔵 |

