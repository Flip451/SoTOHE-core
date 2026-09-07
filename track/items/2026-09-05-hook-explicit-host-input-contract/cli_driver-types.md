<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookExecution | enum | add | InputError, HookBlock, AdvisoryFired, Allow | 🔵 | 🔵 |
| HookHost | enum | add | Claude, Codex, Grok | 🔵 | 🔵 |
| cli_driver::hook::HookName | enum | reference | HooksPathSetup, BlockDirectGitOps, BlockTestFileDeletion, GitRefUpdate, GitPrePush, SkillCompliance | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli_driver::hook::HookInput | dto | modify | — | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli_driver::hook::HookDriver | primary_adapter | modify | — | 🔵 | 🔵 |

