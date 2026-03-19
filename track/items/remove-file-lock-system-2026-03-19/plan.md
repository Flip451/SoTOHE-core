<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# ファイルロックシステム一式削除

ファイルロックシステム一式（~2,100行）を consumer-first (bottom-up) で削除する。まず消費者（CLI lock コマンド → usecase/CLI hook ハンドラ + settings.json → FsTrackStore + CLI track/ + make.rs + track_state_machine.py）を順に除去し、最後にプロバイダー（infrastructure::lock → domain::lock）を削除する。各タスクは単独でワークスペース全体がコンパイル可能かつランタイム安全。

## Consumer removal — CLI lock command

CLI lock サブコマンド全体を除去。

- [ ] Remove CLI lock.rs entirely + remove LockCommand variant from main.rs + remove pub mod lock from commands/mod.rs — consumer removal only, domain::lock and infrastructure::lock still exist so workspace compiles

## Consumer removal — usecase + CLI hook + settings.json

usecase lock handlers + CLI hook lock 分岐 + settings.json lock hook entries を同時除去。ハンドラーと呼び出し元を同時に削除して fail-closed を回避。

- [ ] Remove usecase lock handlers (LockAcquire/ReleaseHookHandler, resolve_lock_mode, DEFAULT_LOCK_TIMEOUT, StubLockManager, all lock tests) from hook.rs + remove CLI hook.rs FileLockAcquire/Release branches, CliHookName lock variants, AgentId import, locks_dir/agent/pid clap args, resolve_locks_dir helper, and lock-related HookContext construction + remove file-lock hook entries from .claude/settings.json (PreToolUse file-lock-acquire + PostToolUse file-lock-release) to prevent fail-closed hooks calling deleted handlers — atomic commit

## Consumer removal — FsTrackStore + CLI track + make.rs + Python wrapper

FsTrackStore ジェネリクス除去 + CLI track/ + make.rs + track_state_machine.py の locks_dir 参照を同時更新。

- [ ] Simplify FsTrackStore: remove FileLockManager generic, lock_manager, lock_timeout, lock imports, lock calls in save/update/with_locked_document, simplify constructor to new(root), delete NoOpLockManager + delete concurrency.rs + update ALL CLI track/ callers (state_ops.rs, activate.rs, transition.rs, mod.rs, resolve.rs, review.rs) removing locks_dir args + update make.rs to remove --locks-dir from track transition/add-task/set-override/clear-override dispatch + update scripts/track_state_machine.py to remove --locks-dir — atomic commit

## Provider removal — infrastructure::lock + domain::lock

全消費者が除去された後、infrastructure::lock → domain::lock の順で削除。

- [ ] Delete infrastructure::lock module entirely (lock/mod.rs, fs_lock_manager.rs) + remove pub mod lock from infrastructure lib.rs + remove fd-lock from Cargo.toml — no consumers remain after T001-T003 so workspace compiles
- [ ] Delete domain::lock module entirely (lock/mod.rs, types.rs, error.rs, port.rs, guard.rs) + remove pub mod lock from domain lib.rs + remove HookError::Lock variant from hook/error.rs + remove HookName::FileLockAcquire/FileLockRelease from hook/types.rs + remove HookContext locks_dir/agent/pid fields + update HookInput doc comments — no consumers remain after T001-T004 so workspace compiles

## Configuration, documentation, and verification

orchestra.rs 検証コード + .gitignore 更新、ドキュメント更新、最終 CI パス確認。

- [ ] Remove file-lock expectations from verify/orchestra.rs + remove .locks/ entry from .gitignore
- [ ] Update DESIGN.md and tech-stack.md to remove lock-related references (fd-lock, FsFileLockManager, infrastructure::lock module tree, lock design rationale)
- [ ] Run cargo make ci and verify full pass — all lock references eliminated
