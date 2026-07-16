<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCommand | enum | add | Snapshot, Restore, CheckReview, CheckCommit | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCheckCommitArgs | dto | add | — | 🟡 | 🔵 |
| AdrBaselineCheckReviewArgs | dto | add | — | 🟡 | 🔵 |
| AdrBaselineRestoreArgs | dto | add | — | 🟡 | 🔵 |
| AdrBaselineSnapshotArgs | dto | add | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::adr_baseline::execute | free_function | add | fn(cmd: AdrBaselineCommand) -> std::process::ExitCode | 🟡 | 🔵 |
| cli::commands::adr_baseline::execute_with_error_chain | free_function | add | fn(cmd: AdrBaselineCommand) -> (std::process::ExitCode, Option<String>) | 🟡 | 🔵 |

