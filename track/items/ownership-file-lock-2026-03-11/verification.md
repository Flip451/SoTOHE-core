# Verification: Ownership-based File Lock Manager

## Scope Verified

- [ ] Domain lock types (FilePath, AgentId, LockMode, LockEntry)
- [ ] LockError hierarchy
- [ ] FileGuard RAII with boxed release callback
- [ ] FileLockManager port trait
- [ ] FsFileLockManager infrastructure implementation
- [ ] CLI lock subcommands (acquire/release/status/cleanup/extend)
- [ ] PreToolUse/PostToolUse Python hooks
- [ ] Cross-process lock coordination

## Manual Verification Steps

- [ ] `cargo make ci` passes all checks
- [ ] Exclusive lock blocks concurrent exclusive acquire
- [ ] Exclusive lock blocks concurrent shared acquire
- [ ] Multiple shared locks coexist
- [ ] FileGuard Drop releases lock automatically
- [ ] Stale lock with dead PID is reaped on next acquire
- [ ] Stale lock with expired TTL is reaped on next acquire
- [ ] PreToolUse hook blocks Edit/Write when file is exclusively locked
- [ ] PostToolUse hook releases lock after tool execution
- [ ] `lock status` shows current lock state as JSON
- [ ] Layer dependency rules respected (`deny.toml`, `check_layers.py`)

## Result / Open Issues

_Not yet verified._

## verified_at

_Not yet verified._
