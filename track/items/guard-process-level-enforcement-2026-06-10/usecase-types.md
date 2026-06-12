<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookVerdictDecision | enum | reference | Allow, Block | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookDispatchError | error_type | reference | UnknownHookName, HandlerFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookHandler | secondary_port | reference | fn handle(&self, ctx: &domain::hook::HookContext, input: &domain::hook::HookInput) -> Result<domain::hook::HookVerdict, domain::hook::HookError> | 🔵 | 🔵 |
| HookShellParserPort | secondary_port | reference | fn split_shell(&self, input: &str) -> Result<Vec<domain::guard::SimpleCommand>, domain::guard::ParseError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookDispatchService | application_service | reference | fn dispatch(&self, hook_name: String, command: HookDispatchCommand) -> Result<HookVerdictOutput, HookDispatchError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitPrePushHandler | interactor | — | — | 🔵 | 🔵 |
| GitRefUpdateHandler | interactor | — | — | 🔵 | 🔵 |
| GuardHookHandler | interactor | reference | — | 🔵 | 🔵 |
| HookDispatchInteractor | interactor | modify | — | 🔵 | 🔵 |
| HooksPathSetupHandler | interactor | — | — | 🔵 | 🔵 |
| TestFileDeletionGuardHandler | interactor | reference | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookVerdictOutput | dto | reference | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookDispatchCommand | command | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::hook::dispatch | free_function | reference | fn(_name: domain::hook::HookName, handler: &dyn HookHandler, ctx: &domain::hook::HookContext, input: &domain::hook::HookInput) -> Result<domain::hook::HookVerdict, domain::hook::HookError> | 🔵 | 🔵 |

