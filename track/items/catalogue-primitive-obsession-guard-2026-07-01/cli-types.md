<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLintCommand | enum | add | CheckActiveTrack | 🔵 | 🔵 |
| CliCommand | enum | modify | Arch, Conventions, Domain, Guard, Hook, Track, Git, Pr, Plan, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, Signal, TaskContract, CatalogueLint, Demo | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLintCheckActiveTrackArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::catalogue_lint::execute | free_function | add | fn(cmd: CatalogueLintCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalogue_lint::execute_check_active_track | free_function | add | fn(args: CatalogueLintCheckActiveTrackArgs) -> std::process::ExitCode | 🔵 | 🔵 |

