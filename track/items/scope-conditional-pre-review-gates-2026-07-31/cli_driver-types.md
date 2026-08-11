<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseCommandInput | enum | add | Validate, Explain, Enter | 🔵 | 🔵 |
| RefVerifyChainSelect | enum | reference | Chain1, Chain2, All | 🔵 | 🔵 |
| ReviewCheckRoundSelect | enum | add | Final | 🔵 | 🔵 |
| ReviewInput | enum | modify | RunCodex, RunClaude, RunLocal, CheckApproved, CheckZeroFindings, Results, Classify, Files, ValidateScope, GetBriefing, PersistCommitHash | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecDriverInput | dto | modify | — | 🔵 | 🔵 |
| CommandOutcome | dto | reference | — | 🔵 | 🔵 |
| PhaseIdArg | dto | add | — | 🔵 | 🔵 |
| RefVerifyCheckApprovedInput | dto | modify | — | 🔵 | 🔵 |
| ReviewCheckZeroFindingsInput | dto | add | — | 🔵 | 🔵 |
| ReviewFixInput | dto | add | — | 🔵 | 🔵 |
| ReviewResultsInput | dto | add | — | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| PhaseCommandDriver | primary_adapter | add | — | 🔵 | 🔵 |
| RefVerifyDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| ReviewDriver | primary_adapter | modify | — | 🔵 | 🔵 |
| ReviewFixDriver | primary_adapter | modify | — | 🔵 | 🔵 |

