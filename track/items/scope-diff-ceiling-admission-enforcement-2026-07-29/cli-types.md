<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanCommand | enum | add | Check | 🟡 | 🔵 |
| CliCommand | enum | modify | Arch, AdrBaseline, Conventions, Domain, Guard, Hook, Maintenance, Track, Git, Pr, Capability, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template, CodexRuntime, BatchPlan | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanCheckArgs | dto | add | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::batch_plan::execute | free_function | add | fn(cmd: BatchPlanCommand) -> std::process::ExitCode | 🟡 | 🔵 |

