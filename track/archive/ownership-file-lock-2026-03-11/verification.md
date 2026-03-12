# Verification: Ownership-based File Lock Manager

## Scope Verified

- [x] Domain lock types (FilePath, AgentId, LockMode, LockEntry)
- [x] LockError hierarchy (including AlreadyHeld)
- [x] FileGuard RAII with boxed release callback
- [x] FileLockManager port trait
- [x] FsFileLockManager infrastructure implementation
- [x] CLI lock subcommands (acquire/release/status/cleanup/extend)
- [x] PreToolUse/PostToolUse Python hooks
- [x] Cross-process lock coordination (registry-based)

## Manual Verification Steps

- [x] `cargo make ci` passes all checks (26 Rust tests, 444 script tests, 341 hook tests)
- [x] Exclusive lock blocks concurrent exclusive acquire
- [x] Exclusive lock blocks concurrent shared acquire
- [x] Multiple shared locks coexist
- [x] Same-agent reacquire is rejected (AlreadyHeld error)
- [x] Same-agent mode change is rejected (no lock upgrading)
- [x] Stale lock with expired TTL is reaped on next acquire/cleanup
- [x] Lock on nonexistent file uses parent directory canonicalization
- [x] Invalid registry mode is rejected (typed enum deserialization)
- [x] Atomic registry write (temp + sync + rename)
- [x] `lock status` shows current lock state as JSON
- [x] Layer dependency rules respected (`deny.toml`, `check_layers.py`)
- [x] Timeout returns error on conflict

## Known Limitations (deferred to future track)

The following issues were identified during Codex review and are accepted as known
limitations for this initial implementation. A future track should introduce a
`LockOwner`/`LockLease`/`FileLockRegistry` redesign to address them:

1. **PID lifecycle mismatch in CLI flow**: The CLI `lock acquire` subprocess is
   short-lived. Its PID is stored in the registry but dies immediately after exit.
   PID-based stale reaping can incorrectly reap live locks. Mitigation: hooks
   should pass the long-lived parent PID via `--pid` flag, or TTL provides the
   primary safety net.

2. **FileGuard RAII not meaningful in CLI flow**: The CLI acquire uses
   `mem::forget(guard)` since the process exits immediately. The RAII guard is
   designed for in-process use. Future work should separate the in-process guard
   API from the CLI registry API.

3. **release/extend keyed on (path, agent) without PID**: One process can
   release another process's lock if they share the same AgentId. Future work
   should use `(path, LockOwner{agent, pid, start_time})` or a lease ID.

## Result / Open Issues

All acceptance criteria met within the defined scope. Three architectural
limitations documented above are deferred to a follow-up lease API redesign track.

## verified_at

2026-03-11
