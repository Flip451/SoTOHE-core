# Spec: PR Task Completion Guard

## Goal

`sotp pr push` 実行時に、track の全タスクが完了していなければブロックする。マージ後に main 上でタスク状態を直接更新する必要をなくす。

## Scope

- `TrackMetadata::all_tasks_resolved()` メソッド追加 [source: libs/domain/src/track.rs]
- `apps/cli/src/commands/pr.rs` の `push()` 関数にガード追加 [source: pr.rs L167-174]
- テスト追加

## Constraints

- `push()` のみにガードを追加。`ensure-pr`, `pr-review`, `wait-and-merge` はブロックしない [source: ユーザー判断]
- `plan/` ブランチからの push はタスクチェックをスキップ（計画 artifacts のみ） [source: inference — planning-only は実装タスクなし]
- 既存テスト（1068 件）を壊さないこと

## Acceptance Criteria

1. 未完了タスクがある track ブランチから `sotp pr push` を実行すると `[BLOCKED]` エラーで失敗する
2. 全タスクが `done`/`skipped` の track ブランチから `sotp pr push` を実行すると正常に push される
3. `plan/` ブランチからの push はガードをスキップする
4. `cargo make ci` が通過する

## Related Conventions (Required Reading)

- `project-docs/conventions/prefer-type-safe-abstractions.md`
