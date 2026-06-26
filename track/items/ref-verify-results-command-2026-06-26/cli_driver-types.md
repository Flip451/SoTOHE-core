<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyChainSelect | enum | add | Chain1, Chain2, All | 🔵 | 🔵 |
| RefVerifyInput | enum | modify | Run, CheckApproved, Results | 🔵 | 🔵 |
| RefVerifyVerdictSelect | enum | add | FailPending, Pass, Fail, Pending, All | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyCheckApprovedInput | dto | reference | — | 🔵 | 🔵 |
| RefVerifyResultsInput | dto | add | — | 🔵 | 🔵 |
| RefVerifyRunInput | dto | reference | — | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyDriver | primary_adapter | modify | — | 🔵 | 🔵 |

