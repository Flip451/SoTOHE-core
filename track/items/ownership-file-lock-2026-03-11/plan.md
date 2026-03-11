<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Ownership-based file lock manager for agent concurrent processing

Map Rust ownership semantics (&/&mut) to file-level access control for Agent Teams.
PreToolUse hooks acquire shared/exclusive locks; PostToolUse hooks release them.
File-based lock registry (.locks/registry.json) protected by flock for cross-process coordination.
Domain layer defines ports (FileLockManager trait); infrastructure implements with fd-lock.
CLI exposes lock subcommands; Python hooks bridge via subprocess invocation.

## Domain layer: lock types and validation

FilePath (canonicalized path), AgentId, LockMode (Shared/Exclusive), LockEntry.
LockError hierarchy: InvalidPath, ExclusivelyHeld, SharedLockConflict, NotFound, Timeout, RegistryIo.

- [ ] Implement domain lock types: FilePath, AgentId, LockMode, LockEntry
- [ ] Implement domain LockError hierarchy with thiserror

## Domain layer: RAII guard and port trait

FileGuard with boxed release callback (domain stays I/O-free).
FileLockManager trait: acquire, release, query, cleanup, extend.

- [ ] Implement FileGuard with RAII drop and boxed release callback
- [ ] Define FileLockManager port trait (acquire/release/query/cleanup/extend)

## Infrastructure layer: FsFileLockManager

flock on .locks/LOCK for atomic read-modify-write of registry.json.
PID-based stale lock detection and TTL expiry (5min default).
Deadlock prevention: lexicographic path ordering, try-lock with timeout, no upgrading.

- [ ] Implement FsFileLockManager: flock + registry.json with PID/TTL stale recovery

## CLI layer: lock subcommands

clap Subcommand: acquire, release, status, cleanup, extend.
JSON output for hook integration.

- [ ] Add CLI lock subcommands: acquire, release, status, cleanup, extend

## Hook integration: PreToolUse / PostToolUse

Python hooks extract file path from tool_input JSON.
PreToolUse: invoke CLI lock acquire; block on conflict (exit 2).
PostToolUse: invoke CLI lock release.

- [ ] Implement PreToolUse/PostToolUse Python hooks for lock acquire/release

## Integration tests and CI verification

Cross-process lock contention tests.
Stale lock recovery tests.
cargo make ci passes all checks.

- [ ] Integration tests: cross-process contention, stale recovery, CI verification
