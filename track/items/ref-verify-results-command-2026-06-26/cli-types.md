<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyChainArg | enum | add | Chain1, Chain2, All | 🔵 | 🔵 |
| RefVerifyCommand | enum | modify | Run, CheckApproved, Results | 🔵 | 🔵 |
| RefVerifyVerdictFilterArg | enum | add | Pass, Fail, Pending, All | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckApprovedArgs | dto | reference | — | 🔵 | 🔵 |
| RefVerifyResultsArgs | dto | add | — | 🔵 | 🔵 |
| RunArgs | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::ref_verify::execute | free_function | modify | fn(cmd: RefVerifyCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::ref_verify::execute_results | free_function | add | fn(args: &RefVerifyResultsArgs) -> std::process::ExitCode | 🔵 | 🔵 |

