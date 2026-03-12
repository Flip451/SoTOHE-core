<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Hook Fail-Closed: Rust hook dispatcher + Python advisory warn-and-log

ERR-06: Migrate security-critical hooks from Python fail-open to Rust fail-closed.
Implement `sotp hook dispatch <hook-name>` as single Rust entry point for security hooks.
Apply SOLID: shared Decision (SRP), HookHandler trait (OCP), domain ports (DIP), no clap in domain (DIP).
Advisory hooks remain Python with warn-and-log pattern (log error, emit warning, exit 0).

## Domain Types (SRP + DIP)

Extract Decision to domain root for reuse by guard and hook subdomains.
Define hook-specific types in domain::hook module.
No framework dependencies (clap, serde_json::Value) in domain — use re-export or infra-side parsing.

- [x] domain::decision — Extract shared Decision enum to domain root (used by guard and hook)
- [x] domain::hook — HookName, HookContext, HookEnvelope, HookToolInput, HookVerdict, HookError type definitions
- [x] domain::guard::verdict — Refactor GuardVerdict to use shared Decision enum

## UseCase Layer (OCP)

HookHandler trait enables adding new hooks without modifying dispatch logic.
GuardHookHandler delegates to existing domain::guard::policy::check.
LockAcquire/ReleaseHookHandler delegates to existing domain::lock::FileLockManager.

- [x] usecase::hook — HookHandler trait (OCP: each hook implements independently)
- [x] usecase::hook — GuardHookHandler implementation (delegates to domain::guard::policy::check)
- [x] usecase::hook — LockAcquireHookHandler and LockReleaseHookHandler implementations

## CLI Integration

HookCommand with clap::ValueEnum for hook name (CLI layer only).
Reads JSON from stdin, dispatches to HookHandler, emits JSON to stdout, sets exit code.

- [x] apps/cli/commands/hook.rs — HookCommand with CliHookName (clap::ValueEnum), stdin JSON parsing, exit code mapping

## Python Hook Migration

Security hooks become thin launchers: exec sotp hook dispatch, propagate exit code.
Advisory hooks get warn-and-log pattern in _shared.py.

- [x] Python hooks — block-direct-git-ops.py becomes thin sotp hook dispatch launcher; advisory hooks get warn-and-log pattern via _shared.py

## Tests

Rust: hook dispatch unit tests, HookHandler mock tests.
Python: launcher integration test, warn-and-log behavior test.

- [x] Tests — Rust unit tests for hook dispatch + Python selftest for launcher and warn-and-log behavior
