# Spec: Track State Machine — DMMF Domain Model

## Goal

Python リファレンス実装（`scripts/track_schema.py`, `scripts/track_state_machine.py`）と同等の track ステートマシンを Rust ドメインモデルとして実装する。DMMF（Domain Modeling Made Functional）パターンで「不正な状態を表現不可能」にする。

## Scope

### In scope

- **Domain layer** (`libs/domain`):
  - Newtype パターンの ID 型（`TrackId`, `TaskId`, `CommitHash`）とバリデーション
  - thiserror による階層的エラー型（`DomainError` > `ValidationError` / `TransitionError` / `RepositoryError`）
  - タスク状態マシン: `TaskStatus` enum（`Done { commit_hash }` でデータ紐付け）、`TaskTransition` コマンド
  - `TrackTask` エンティティ: `transition()` メソッドで許可エッジのみ受付
  - `TrackMetadata` アグリゲート: タスク状態からの `TrackStatus` 自動導出、`StatusOverride`（Blocked/Cancelled）
  - `PlanView` / `PlanSection`: plan-task 参照整合性のバリデーション
  - `TrackRepository` trait（port）
- **Usecase layer** (`libs/usecase`):
  - `SaveTrackUseCase`, `LoadTrackUseCase`, `TransitionTaskUseCase`
- **Infrastructure layer** (`libs/infrastructure`):
  - `InMemoryTrackRepository`（`Mutex<HashMap>` 実装）
- **CLI** (`apps/cli`):
  - 新ドメインモデルを使ったデモ動作

### Out of scope

- JSON ファイルベースの永続化（後続 track で対応）
- serde によるシリアライゼーション（後続 track で対応）
- clap による CLI パーサー（後続 track で対応）

## Constraints

- Rust edition 2024, MSRV 1.85
- 外部依存は `thiserror` のみ（domain 層）
- Python リファレンスの状態遷移ルールと完全に一致すること
- レイヤー依存ルール（`deny.toml`, `check_layers.py`）を違反しないこと

## Acceptance Criteria

1. `cargo make ci` が全チェック通過
2. タスク状態遷移が Python リファレンスと同一のエッジセット
3. TrackStatus がタスク状態から正しく導出される
4. StatusOverride が全タスク解決時に自動クリアされる
5. Plan-Task 参照整合性（全タスクが plan に正確に1回参照）が検証される
6. 10+ テストが domain/usecase/infrastructure/cli にまたがる
