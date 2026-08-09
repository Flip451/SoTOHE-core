<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseCommandInput | enum | add | Validate, Explain, Enter | 🔵 | 🔵 |
| RefVerifyChainSelect | enum | reference | Chain1, Chain2, All | 🔵 | 🔵 |
| ReviewInput | enum | modify | RunCodex, RunClaude, RunLocal, CheckApproved, CheckZeroFindings, Results, Classify, Files, ValidateScope, GetBriefing, PersistCommitHash | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecDriverInput | dto | modify | — | 🔵 | 🔵 |
| CommandOutcome | dto | reference | — | 🔵 | 🔵 |
| PhaseIdArg | dto | add | — | 🔵 | 🔵 |
| RefVerifyCheckApprovedInput | dto | modify | — | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| PhaseCommandDriver | primary_adapter | add | — | 🔵 | 🔵 |
| RefVerifyDriver | primary_adapter | reference | — | 🔵 | 🔵 |

