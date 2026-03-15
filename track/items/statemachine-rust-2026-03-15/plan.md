<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# STRAT-03 Phase 2: Track state machine の Rust 化

STRAT-03 Phase 2: track_state_machine.py / track_schema.py / track_markdown.py の中核責務を sotp track サブコマンドに移行する。
transition と sync-views は既に Rust 実装済み。残りの add-task, set-override, next-task, task-counts を CLI サブコマンドとして追加し、Python フォールバックを除去する。

## Domain 層 — タスク追加メソッド

TrackMetadata にタスク追加のドメインロジックを実装する。
next_task_id() で連番 TaskId 生成、add_task() で tasks + section.task_ids を更新し、validate_plan_invariants() で整合性を保証。

- [ ] Domain 層: TrackMetadata::add_task() + next_task_id() メソッド追加

## Usecase 層 — 新ユースケース

TrackWriter::update() 経由の atomic RMW でタスク追加とオーバーライド操作を実装。
既存の TransitionTaskUseCase と同じパターンに従う。

- [ ] Usecase 層: AddTaskUseCase（atomic RMW でタスク追加）
- [ ] Usecase 層: SetOverrideUseCase（atomic RMW でオーバーライド設定/解除）

## CLI 層 — 新サブコマンド

sotp track に add-task, set-override, clear-override, next-task, task-counts サブコマンドを追加。
next-task と task-counts は JSON 出力でスクリプトから解析可能にする。

- [ ] CLI 層: sotp track add-task サブコマンド（--section/--after オプション + ブランチガード）
- [ ] CLI 層: sotp track set-override / clear-override サブコマンド（ブランチガード付き）
- [ ] CLI 層: sotp track next-task サブコマンド（plan section 順 + in_progress 優先 + JSON スキーマ準拠）
- [ ] CLI 層: sotp track task-counts サブコマンド（total/todo/in_progress/done/skipped の JSON スキーマ準拠）

## Makefile 統合 + bin/sotp 更新

Makefile.toml に wrapper タスクを追加し、bin/sotp をリビルド。

- [ ] Makefile 統合: wrapper タスク追加 + bin/sotp リビルド

## Python 削減

track_state_machine.py の Python フォールバックを削除し sotp を必須化。
ドキュメント参照を更新し、deprecated マーキングを追加。

- [ ] Python 削減: track_state_machine.py の Python フォールバック削除 + sotp 必須化
- [ ] ドキュメント更新: deprecated マーキング + /track:plan コマンド参照更新
