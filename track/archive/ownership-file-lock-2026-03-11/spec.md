# Spec: Ownership-based File Lock Manager

## Goal

Rust の所有権セマンティクス（`&` 共有参照 / `&mut` 排他参照）をファイルアクセス制御にマッピングし、Agent Teams 並行実行時のファイル競合を防止する。PreToolUse / PostToolUse フックがインターセプトポイントとなり、Rust CLI バイナリを呼び出してロック管理を行う。

## Scope

### In scope

- **Domain layer** (`libs/domain/src/lock/`):
  - `FilePath`（正規化パス）、`AgentId`、`LockMode`（Shared/Exclusive）、`LockEntry` 型
  - `LockError` 階層（`InvalidPath` / `ExclusivelyHeld` / `SharedLockConflict` / `NotFound` / `Timeout` / `RegistryIo`）
  - `FileGuard`（RAII ガード、boxed release callback で domain 層を I/O フリーに保つ）
  - `FileLockManager` trait（port）: `acquire` / `release` / `query` / `cleanup` / `extend`
- **Infrastructure layer** (`libs/infrastructure/src/lock/`):
  - `FsFileLockManager`: `.locks/registry.json` + `flock` on `.locks/LOCK` で cross-process 協調
  - PID 生存確認 + TTL(5分) による stale lock 自動回収
  - デッドロック防止: パス辞書順取得、タイムアウト、アップグレード禁止
- **CLI layer** (`apps/cli/src/commands/lock.rs`):
  - `lock` サブコマンド: `acquire` / `release` / `status` / `cleanup` / `extend`
  - JSON 出力（hook 連携用）
- **Hook integration** (`.claude/hooks/`):
  - PreToolUse: Edit/Write ツール検知 → CLI `lock acquire --mode exclusive` 呼び出し
  - PreToolUse: Read ツール検知 → CLI `lock acquire --mode shared` 呼び出し
  - PostToolUse: CLI `lock release` 呼び出し

### Out of scope

- ディレクトリ単位のロック（ファイル単位のみ）
- ネットワーク越しのロック協調（ローカルファイルシステムのみ）
- GUI / Web ダッシュボードでのロック可視化
- standalone crate としての公開（将来の抽出は設計で考慮するが、本 track では CLI 内に組み込む）

## Constraints

- Rust edition 2024, MSRV 1.85
- 同期のみ（async runtime なし）
- domain 層の外部依存は `thiserror` のみ
- infrastructure 層で `fd-lock` クレートを使用
- レイヤー依存ルール（`deny.toml`, `check_layers.py`）を違反しないこと
- `track/tech-stack.md` に `fd-lock` を追加してから実装開始
- Hook エラー処理は fail-closed（ロック取得失敗時はツール実行をブロック、fail-open しない）
- 同一エージェントの再取得（`AlreadyHeld`）はタイムアウト設定に関わらず即時エラー返却

## Acceptance Criteria

1. `cargo make ci` が全チェック通過
2. Agent A が exclusive lock を保持中に Agent B が同じファイルに acquire → `ExclusivelyHeld` エラー
3. Agent A が exclusive lock を保持中に Agent B が shared lock を acquire → `ExclusivelyHeld` エラー
4. 複数エージェントが同時に shared lock を取得可能
5. FileGuard の Drop で自動的にロック解放
6. PID が存在しない stale entry が自動回収される
7. TTL 超過の stale entry が自動回収される
8. PreToolUse hook が Edit/Write ツールで exclusive lock を取得
9. PostToolUse hook がロック解放
10. lock status コマンドで現在のロック状態を JSON 表示
