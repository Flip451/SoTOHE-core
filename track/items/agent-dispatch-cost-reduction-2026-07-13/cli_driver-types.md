<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityResumeArg | enum | add | Fresh, ResumeWithoutTarget, Resume | 🔵 | 🔵 |
| ReviewInput | enum | modify | RunCodex, RunClaude, RunLocal, CheckApproved, Results, Classify, Files, ValidateScope, GetBriefing, PersistCommitHash | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecDriverInput | dto | modify | — | 🔵 | 🔵 |
| TargetArtifactPathArg | dto | add | — | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| ReviewFixDriver | primary_adapter | add | — | 🔵 | 🔵 |

