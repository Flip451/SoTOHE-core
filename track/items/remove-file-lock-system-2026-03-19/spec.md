---
status: planned
version: 1
signals: 0/0/0
---

# Spec: ファイルロックシステム一式削除

## Goal

ファイルロックシステム一式（~2,100行）を完全に削除し、コードベースを簡素化する。

## Background

ファイルロックシステムは並行エージェントアクセス時の metadata.json 排他制御として設計された。
二つの経路がある: (1) Claude hook 経由（`SOTP_LOCK_ENABLED=1` オプトインゲート、デフォルト無効）、
(2) FsTrackStore 内の直接ロック（常時有効だが単一プロセス実行のため実質不要）。
Phase 4 SPEC-04（worktree 分離）により並行アクセスは物理的隔離で解決される方針のため、
両経路とも削除する。詳細な並行安全性の根拠は下記セクション参照。

## Scope

### In scope

- `libs/domain/src/lock/` モジュール全体（types, error, port, guard, mod）
- `libs/infrastructure/src/lock/` モジュール全体（fs_lock_manager, mod）
- `libs/infrastructure/tests/concurrency.rs`（ロックベース並行テスト）
- `libs/usecase/src/hook.rs` の `LockAcquire/ReleaseHookHandler` と `resolve_lock_mode`
- `apps/cli/src/commands/lock.rs` 全体
- `apps/cli/src/commands/hook.rs` の `FileLockAcquire/Release` 分岐
- `apps/cli/src/main.rs` の `LockCommand` enum variant
- `apps/cli/src/commands/mod.rs` の `pub mod lock;`
- `.claude/settings.json` の file-lock hook entries（PreToolUse + PostToolUse）
- `libs/infrastructure/Cargo.toml` の `fd-lock` 依存
- `libs/infrastructure/src/track/fs_store.rs` の `FsTrackStore<L: FileLockManager>` ジェネリクス除去
- CLI track/ (state_ops, activate, transition, mod, resolve, review) の `FsFileLockManager` 構築・参照除去
- `domain::hook::error::HookError::Lock` variant 除去
- `domain::hook::types` のドキュメントコメント内 `FileLockManager` 参照除去
- `libs/infrastructure/src/verify/orchestra.rs` の file-lock hook 検証コード除去
- `.claude/docs/DESIGN.md` と `track/tech-stack.md` のロック関連記述更新
- `.gitignore` の `.locks/` エントリ除去

### Out of scope

- `FsTrackStore` の並行安全性の再設計（Phase 4 SPEC-04 で worktree 分離として対応）
- `fd-lock` crate の他用途への転用

## Concurrency Safety Justification

FsTrackStore は FileLockManager で metadata.json 書き込みを排他制御しているが、
ロック除去後の並行安全性は以下の根拠で担保される：

1. **単一プロセス実行モデル**: 現在の sotp CLI は単一プロセスで実行される。
   `cargo make track-transition` 等の wrapper は逐次実行であり、同一 track への
   同時書き込みは発生しない。
2. **Agent Teams の分離**: `/track:implement` の並列ワーカーは `WORKER_ID` で
   `CARGO_TARGET_DIR` を分離するが、metadata.json への書き込みは orchestrator が
   逐次的に行う（ワーカーは実装のみ）。
3. **Phase 4 SPEC-04 による物理隔離**: 将来の並行エージェントアクセスは worktree 分離
   で対応する方針が確定している。ファイルレベルのロックは不要。
4. **実績**: SOTP_LOCK_ENABLED はデフォルト無効のまま、27 トラック 188 タスクを
   データ破損なく運用した実績がある。

### review.rs with_locked_document の TOCTOU 対応

`review.rs:871-882` の `with_locked_document` は PENDING 書き込みと code_hash 計算の
間に別プロセスが割り込むことを防ぐために使用されている。ロック除去後:

- `with_locked_document` は `update` と同じ read-modify-write に簡素化する
  （ロック取得/解放が no-op になるだけで、closure API は維持）
- コメントを更新: 「排他制御は単一プロセス実行モデルに依存。並行アクセスは
  Phase 4 SPEC-04 worktree 分離で対応」
- `/track:implement` のワーカーは実装のみ行い、metadata.json 書き込みは
  orchestrator が逐次実行する。ワーカー同士は `WORKER_ID` で
  `CARGO_TARGET_DIR` を分離するが、metadata.json には触れない。

## Constraints

- 全変更後に `cargo make ci` が完全パスすること
- `FsTrackStore` のジェネリクス除去後、既存の単体テスト・統合テストが全てパスすること
- `.gitignore` の `.locks/` エントリを除去すること

## Acceptance Criteria

1. `libs/domain/src/lock/` ディレクトリが存在しないこと
2. `libs/infrastructure/src/lock/` ディレクトリが存在しないこと
3. `apps/cli/src/commands/lock.rs` が存在しないこと
4. `libs/infrastructure/tests/concurrency.rs` が存在しないこと
5. `grep -r "FileLockManager\|FsFileLockManager\|LockMode\|LockError\|FileGuard\|LockEntry\|AgentId\|FileLockAcquire\|FileLockRelease\|locks_dir" libs/ apps/` が 0 件であること（lock:: namespace のもの）
6. `grep -r "fd.lock\|fd_lock" libs/ apps/ Cargo.toml` が 0 件であること
7. `.claude/settings.json` に `file-lock-acquire` / `file-lock-release` の記述がないこと
8. `cargo make ci` が全パスすること
9. `FsTrackStore` がジェネリクスなしの具体型として定義されていること
10. `HookContext` に `locks_dir`, `agent` (AgentId型), `pid` フィールドが存在しないこと
11. `HookName` に `FileLockAcquire` / `FileLockRelease` variant が存在しないこと
12. `.gitignore` に `.locks/` エントリが存在しないこと
13. `.claude/docs/DESIGN.md` にロック関連記述（fd-lock, FsFileLockManager, infrastructure::lock）が存在しないこと
14. `track/tech-stack.md` にロック関連記述（fd-lock, ファイルロック）が存在しないこと

## Related Conventions (Required Reading)

- `project-docs/conventions/layered-architecture.md` (存在する場合)
