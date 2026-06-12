<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookName | enum | modify | BlockDirectGitOps, BlockTestFileDeletion, GitRefUpdate, GitPrePush | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GuardVerdict | value_object | reference | — | 🔵 | 🔵 |
| HookContext | value_object | reference | — | 🔵 | 🔵 |
| HookInput | value_object | reference | — | 🔵 | 🔵 |
| HookVerdict | value_object | reference | — | 🔵 | 🔵 |
| SimpleCommand | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| HookError | error_type | reference | Input, Guard, Unsupported | 🔵 | 🔵 |
| ParseError | error_type | reference | NestingDepthExceeded, UnmatchedQuote | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ShellParser | secondary_port | reference | fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::guard::policy::block_on_parse_error | free_function | reference | fn(err: &ParseError) -> GuardVerdict | 🔵 | 🔵 |
| domain::guard::policy::check_commands | free_function | modify | fn(commands: &[SimpleCommand]) -> GuardVerdict | 🔵 | 🔵 |
| domain::guard::policy::contains_git_invocation | free_function | — | fn(commands: &[SimpleCommand]) -> bool | 🔵 | 🔵 |

