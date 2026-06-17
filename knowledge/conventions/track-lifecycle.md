# Track Lifecycle Convention

## Purpose

`metadata.json` を識別情報の SSoT、`impl-plan.json` を実装計画・タスク進捗の SSoT とし、`plan.md` / `track/registry.md` を `bin/sotp track views sync` から自動生成される読み取り専用ビューに限定する。タスク状態遷移は `bin/sotp track` サブコマンド経由の atomic write に限定して手編集を禁止し、機械検証不能な手動観測は `observations.md` だけに閉じ込めることで、生成ビューと SSoT の乖離を構造的に排除する。

## Scope

- 適用対象: `track/items/<id>/metadata.json`（schema_version 5、identity-only）、`track/items/<id>/impl-plan.json`（実装計画・タスク進捗 SSoT）、`track/items/<id>/plan.md`（生成ビュー）、`track/registry.md`（生成ビュー、`.gitignore` 済み）、`track/items/<id>/observations.md`（任意手動観測ログ）、`bin/sotp track` 系状態遷移サブコマンド、`/track:plan` / `/track:commit` / `/track:archive` の registry 更新タイミング、タイムスタンプ取り扱い。
- 適用外: ブランチ作成・PR ワークフロー（`knowledge/conventions/branch-strategy.md`）、git note 付与（`knowledge/conventions/git-notes.md`）、`spec.json` / `<layer>-types.json` / `impl-plan.json` の生成パイプライン（各 phase コマンドと spec-designer / type-designer / impl-planner の責務）。

## Rules

### plan.md と metadata.json SSoT

- `metadata.json`（`schema_version: 5`）が identity 情報の唯一の SSoT。`plan.md` は `bin/sotp track views sync` で `metadata.json` + `impl-plan.json` から生成される **読み取り専用ビュー**。直接編集してはならない（`<!-- Generated from ... — DO NOT EDIT DIRECTLY -->` マーカー付き）。
- 二段階ライフサイクル:
  1. **初回作成**（Phase 0 の `/track:init` または `/track:plan` 承認後）: `metadata.json` を作成し、`plan.md` は `bin/sotp track views sync` で生成する。初回から SSoT モデルに従う。
  2. **以降の更新**: タスク状態の変更は `bin/sotp track` サブコマンド経由で `impl-plan.json` の対応 task を更新し、`plan.md` は自動再生成される。直接編集は禁止。
- CI（`verify-plan-progress`）は `plan.md` と `metadata.json` + `impl-plan.json` からのレンダリング結果が一致することを検証する。

### 状態遷移 API

状態遷移は `bin/sotp track` サブコマンド（Rust CLI）を経由する:

- `bin/sotp track transition`: タスクの状態遷移（todo → in_progress → done / skipped）。done に遷移する際は `--commit-hash <hash>` で記録する。
- `bin/sotp track add-task`: 新タスクの追加。
- `bin/sotp track set-override` / `clear-override`: トラック全体のブロック/キャンセル。
- `bin/sotp track next-task`: 次の作業対象タスクの取得（JSON 出力）。
- `bin/sotp track task-counts`: タスク集計の取得（JSON 出力）。

これらのコマンドは `bin/sotp` ネイティブサブコマンドとして直接呼び出す（`--items-dir` のデフォルトは `track/items`）。対応する `cargo make` ラッパータスクは廃止済み。

### observations.md（optional）

各トラックは `observations.md` を **必要に応じて** 作成する。AC 充足判定は `spec.json` の signals + `review.json` の zero_findings + `impl-plan.json` の task done / commit_hash で機械的に行うため、`observations.md` は「機械検証不能な手動観測ログ」専用とする。

作成条件（いずれかに該当する場合のみ）:

- (a) 実装中に implementer が「機械検証不能な観測値が出た」と判断した場合（裁量）。
- (b) `spec.json` の `acceptance_criteria` に「〜を実測して `observations.md` に記録する」と明示された項目がある場合。

フォーマットは自由（scaffold / 必須フィールド / 必須セクションなし）。観測対象・手順・値・日時などを作成者の裁量で含める。ファイルが存在しない場合は「観測なし」として扱い、CI / `verify-latest-track` は不在を error にしない（「file 存在 = phase 状態」原則）。

新規トラックで `verification.md` は作成しない。過去トラックの `verification.md` は歴史資料として原型保存される。

### Generated Views

以下のファイルは `metadata.json`（+ `impl-plan.json`）から自動生成される **読み取り専用ビュー** であり、git stage / commit してはならない:

- `track/registry.md` — `.gitignore` 済み。`bin/sotp track views sync` で再生成される。

タイムスタンプ（`created_at`, `updated_at` 等）は必ず `date -u +%Y-%m-%dT%H:%M:%SZ` コマンドの出力を使用する。手入力や推測は禁止（`created_at > updated_at` 等の不整合を防止するため）。

### track/registry.md 更新ルール

`track/registry.md` は以下のタイミングで `metadata.json` から自動再生成される（生成ロジックは `bin/sotp track views sync`）。

| Trigger | 更新内容 |
| ------- | ------- |
| `/track:plan <feature>` 承認時 | アクティブトラック行を追加・更新、`Current Focus` を設定、`Next recommended command` を `/track:full-cycle <task>` または `/track:implement` に設定（`planned` 状態は自動レンダラーの既定）、`Last updated` 更新 |
| `/track:commit <message>` | 現在のトラックの status/result を更新、完了したものは適切なタイミングで `Completed Tracks` へ移動、`Last updated` 更新 |
| `/track:archive <id>` | トラック status を `archived` に設定、`Completed` から `Archived Tracks` へ移動、`Last updated` 更新。解決済みトラック（全タスクが `done` または `skipped`）のみアーカイブ可能 |

## Examples

- Good: タスクを終えたら `bin/sotp track transition T003 done --commit-hash <hash>` を実行する。`plan.md` と `track/registry.md` は自動再生成され、手動で行を書き換えない。
- Good: 機械検証不能な dogfood 結果が出た task で `observations.md` に観測対象・手順・実測値・日時を自由フォーマットで追記し、機械検証で AC を満たすタスクでは `observations.md` を作成しない。
- Bad: `plan.md` の `[ ]` を `[x]` に手編集する（生成ビューを直接編集してはならない。`bin/sotp track transition` 経由で SSoT を変えて再生成する）。
- Bad: 任意 dogfood ログを `metadata.json` の説明文に詰め込む（identity フィールドに観測値を載せると schema validation や生成ビューに影響する。`observations.md` を使う）。

## Exceptions

- 過去トラックの `verification.md` は歴史資料として原型保存し、リネームしない。ただし新規トラックでは作らない（必要なら `observations.md` を使う）。
- 一時的な checkpoint コミット時に `plan.md` の追加状況を読み返したい場合は再生成 (`bin/sotp track views sync`) だけで対応する。手編集は許容しない。

## Review Checklist

- [ ] `plan.md` / `track/registry.md` を手編集していないか（差分は SSoT 変更 + 自動再生成の結果のみ）
- [ ] タスク状態遷移が `bin/sotp track transition` 経由になっているか
- [ ] `observations.md` の追記が AC の「機械検証不能観測」に該当するか
- [ ] タイムスタンプが `date -u +%Y-%m-%dT%H:%M:%SZ` 由来か（手入力 / 推測がないか）
- [ ] `track/registry.md` の更新タイミングが上表 3 行のいずれかに対応しているか

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本 convention の原典となる ADR はこの索引から辿る
- [knowledge/conventions/branch-strategy.md](./branch-strategy.md) — `track/<id>` ブランチの作成・切替・PR 操作
- [knowledge/conventions/git-notes.md](./git-notes.md) — コミットへの構造化メモ
- [knowledge/conventions/workflow-ceremony-minimization.md](./workflow-ceremony-minimization.md) — 人工状態フィールド・事前承認の最小化
