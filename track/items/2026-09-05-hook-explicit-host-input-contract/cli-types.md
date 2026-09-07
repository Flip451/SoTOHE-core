<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliHookExecution | enum | add | InputError, InternalError, HookBlock, AdvisoryFired, Allow | 🔵 | 🔵 |
| CliHookHost | enum | add | Claude, Codex, Grok | 🔵 | 🔵 |
| HookExecutionDisposition | enum | add | InputError, InternalError, HookBlock, AdvisoryFired, Allow | 🔵 | 🔵 |
| cli::commands::hook::CliHookName | enum | modify | HooksPathSetup, BlockDirectGitOps, BlockTestFileDeletion, GitRefUpdate, GitPrePush, SkillCompliance | 🔵 | 🔵 |
| cli::commands::hook::HookCommand | enum | modify | Dispatch | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::hook::execute_inner | free_function | modify | fn(cmd: HookCommand) -> Result<CliHookExecution, CliError> | 🔵 | 🔵 |
| cli::is_hook_block_outcome | free_function | add | fn(disposition: HookExecutionDisposition) -> bool | 🔵 | 🔵 |

