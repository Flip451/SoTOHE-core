<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexDryChecker | secondary_adapter | reference | — | 🔵 | 🔵 |
| CodexReviewer | secondary_adapter | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::codex_common::tee_stderr_to_file | free_function | — | fn(pipe: std::process::ChildStderr, log_file: std::fs::File) -> () | 🔵 | 🔵 |
| infrastructure::dry_check::corpus::sha256_hex | free_function | reference | fn(data: &[u8]) -> String | 🔵 | 🔵 |

