# STRAT-03 Phase 2: Track State Machine の Rust 化

## Feature Goal

`scripts/track_state_machine.py`, `scripts/track_schema.py`, `scripts/track_markdown.py` の中核責務を `sotp track` サブコマンドに移行し、Python フォールバックを除去する。

## Scope

### In Scope

- Domain 層: `TrackMetadata::add_task()` + `next_task_id()` メソッド
- Usecase 層: `AddTaskUseCase`, `SetOverrideUseCase`
- CLI 層: `sotp track add-task`, `set-override`, `clear-override`, `next-task`, `task-counts` サブコマンド
- Makefile.toml: 新 wrapper タスク追加
- Python 削減: `track_state_machine.py` のフォールバック削除、sotp 必須化

### Out of Scope

- verify script 群の Rust 化（Phase 5 で対応）
- `track_registry.py` / `track_resolution.py` の完全移行（依存する verify script と同時に対応）
- `track_markdown.py` の legacy parser (`summarize_plan()`) の移行（使用箇所が限定的）
- `external_guides.py` の `track_resolution` 依存解消

## Constraints

- `track/tech-stack.md` に未解決の残課題がないこと（現状クリア）
- TDD: テストを先に書く（Red → Green → Refactor）
- 既存の `sotp track transition` / `sotp track views sync` のインターフェースを壊さない
- `TrackWriter::update()` の atomic RMW パターンを踏襲する
- 新サブコマンドの出力フォーマットは既存のパターン（`[OK]` プレフィックス / JSON）に従う

## Behavioral Contracts

### `sotp track add-task`

- `--section <id>` で対象セクションを指定。省略時は最初のセクションに追加。指定された section id が存在しない場合はエラーを返す
- `--after <task-id>` でセクション内の挿入位置を指定。省略時はセクション末尾に追加。指定された task-id が対象セクション内に存在しない場合はセクション末尾に追加（Python の既存挙動に準拠）
- tasks 配列に新タスク（Todo 状態）を追加し、指定セクションの task_ids を更新
- 追加後に `sync_rendered_views()` で plan.md / registry.md を自動再生成
- **ブランチガード**: 現在の git ブランチが metadata.json の branch フィールドと一致しない場合はエラー

### `sotp track set-override` / `clear-override`

- `set-override`: `blocked` or `cancelled` + `--reason <text>` でオーバーライドを設定
- `clear-override`: オーバーライドを解除
- 全タスクが resolved (done/skipped) の場合は set-override を拒否
- **ブランチガード**: set-override / clear-override の両方に適用。不一致時はエラー

### `sotp track next-task`

- plan.sections[*].task_ids の順序に従い、in_progress タスクを優先、次に todo を返す
- JSON 出力スキーマ: `{"task_id":"T001","description":"...","status":"todo"}`
- 該当タスクなし: `{"task_id":null,"description":null,"status":null}`

### `sotp track task-counts`

- JSON 出力スキーマ: `{"total":5,"todo":2,"in_progress":1,"done":2,"skipped":0}`
- skipped を含む全状態を返す

## Acceptance Criteria

- [ ] `sotp track add-task` が `--section` / `--after` オプションでタスク配置を制御できる
- [ ] `sotp track add-task` / `set-override` / `clear-override` がブランチガードを適用し、不一致時にエラーを返す
- [ ] `sotp track set-override` / `clear-override` がオーバーライドを設定/解除する
- [ ] `sotp track next-task` が plan section 順で in_progress 優先、JSON スキーマ準拠で出力する
- [ ] `sotp track task-counts` が全状態（todo/in_progress/done/skipped）を含む JSON を出力する
- [ ] `cargo make track-add-task` 等の wrapper タスクが動作する
- [ ] `track_state_machine.py` の Python フォールバックが除去され、sotp が必須になる
- [ ] 既存の 517+ テストが全て通る
- [ ] `cargo make ci` が通る
