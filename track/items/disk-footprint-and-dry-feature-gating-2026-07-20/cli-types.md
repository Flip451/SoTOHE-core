<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliCommand | enum | modify | Arch, AdrBaseline, Conventions, Domain, Guard, Hook, Maintenance, Track, Git, Pr, Capability, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template, CodexRuntime | 🔵 | 🔵 |
| MaintenanceCommand | enum | add | ConfigureSccache, Cleanup | 🔵 | 🔵 |
| SemanticDupCommandFamily | enum | add | Dry, SemanticDuplicate | 🔵 | 🔵 |
| VerdictFilterArg | enum | modify | All, NotAViolation, Accepted, Violation | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CleanupArgs | dto | add | — | 🔵 | 🔵 |
| ProjectRootArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::maintenance::execute | free_function | add | fn(command: MaintenanceCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::semantic_dup_feature_gate::semantic_dup_feature_disabled_exit | free_function | add | fn(command_family: SemanticDupCommandFamily) -> std::process::ExitCode | 🔵 | 🔵 |

