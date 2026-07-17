<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCommand | enum | add | Snapshot, Restore, CheckReview, CheckCommit | 🔵 | 🔵 |
| CliCommand | enum | modify | Arch, AdrBaseline, Conventions, Domain, Guard, Hook, Track, Git, Pr, Capability, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, TestObligation, Signal, TaskContract, Catalog, CatalogueLint, Template | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCheckCommitArgs | dto | add | — | 🔵 | 🔵 |
| AdrBaselineCheckReviewArgs | dto | add | — | 🔵 | 🔵 |
| AdrBaselineRestoreArgs | dto | add | — | 🔵 | 🔵 |
| AdrBaselineSnapshotArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::adr_baseline::dispatch | free_function | add | fn(cmd: AdrBaselineCommand) -> Result<cli_driver::CommandOutcome, CliError> | 🔵 | 🔵 |
| cli::commands::adr_baseline::execute | free_function | add | fn(cmd: AdrBaselineCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::adr_baseline::execute_with_error_chain | free_function | add | fn(cmd: AdrBaselineCommand) -> (std::process::ExitCode, Option<String>) | 🔵 | 🔵 |

