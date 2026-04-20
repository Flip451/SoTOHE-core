# Pre-Track ADR Authoring Convention

## Purpose

ADR は track 内成果物ではなく **track 前段階** で author が作成する track 横断資産とする。track のライフサイクル (作成 → 進行 → 完了 → archive) と ADR のライフサイクル (書き換え可だが原則安定、全 track で参照される) を独立に保つため、ADR 生成を `/track:plan` の内部ステップから切り離し、事前確認のみを行う。

## Scope

- 適用対象: `knowledge/adr/*.md` の新規作成 / 更新、`/track:plan` の起動条件、adr-editor サブエージェントの invocation。
- 適用外: track 内 research note (`track/items/<id>/research/` 配下)、spec.json や型カタログなど track 内 SSoT 成果物、既に `archive/` に移動した track の ADR 修正。

## Rules

- **配置**: ADR は `knowledge/adr/YYYY-MM-DD-HHMM-slug.md` に配置する。`track/items/` 配下には置かない。
- **作成タイミング**: 初期作成は `/track:plan` **起動前** に user + main 対話 (または手動) で完了させる。`/track:plan` は ADR を自動生成しない。
- **起動時の事前確認**: `/track:plan` は起動直後に、参照予定 ADR が `knowledge/adr/` に存在するかを確認する。未整備なら停止し、ADR 整備を促す (厳密モード)。
- **状態フィールドなし**: ADR に `Status` 見出しや `approved` のような状態フィールドは作らない。ファイルが存在して内容が読める状態が運用上の「承認」。
- **書き換え可 (例外なし)**: 既存 ADR を直接書き換えてよい。大きな決定変更では新規 ADR を作り旧 ADR に `## Follow-up` で参照してもよい (任意)。
- **track 内成果物からの参照**: spec.json は ADR を構造化参照 (`AdrRef { file, anchor }`) で cite できる (SoT Chain ① spec → ADR)。型カタログ (Phase 2) は spec を `spec_refs[]` で参照し、ADR を直接 cite することは SoT Chain のレイヤースキップになるので禁止 (`type catalogue → ADR` は逆流/スキップ)。impl-plan (Phase 3) も同様に spec / 型カタログ経由で参照し、ADR を直接 cite しない。逆方向 (ADR から track 内成果物への参照) は SoT Chain 逆流なので禁止。
- **back-and-forth 自動修正 (adr-editor)**: `/track:plan` の探索的精緻化ループで下流 signal が 🔴 になって ADR 側の修正が必要になった場合、adr-editor サブエージェントが ADR を working tree レベルで編集する。
  - ADR ファイルに commit 履歴あり → auto-edit (working tree のみ、loop 中は commit しない)
  - ADR ファイルに commit 履歴なし → user pause (ADR を先に commit してから再開)
- **終端処理**: `/track:plan` 終了時に ADR working tree に HEAD からの diff があれば、user に diff を提示して判断 (accept / revert / 手動修正 / 中止) を仰ぐ。
- **main による直接編集の禁止**: back-and-forth での ADR 修正も含めて、main orchestrator が `knowledge/adr/*.md` を直接 Edit してはならない。adr-editor サブエージェントを経由する (1 ファイル = 1 writer 原則)。

## Examples

- Good: user が `knowledge/adr/<date>-<slug>.md` を作成 → `/track:plan "feature X"` を invoke → `/track:plan` が ADR 存在を確認して Phase 0 (init) に進む。
- Good: Phase 1 (spec) で signal 🔴 発生 → `/track:plan` が adr-editor を自動 invoke (ADR に commit 履歴あり) → working tree 編集 → loop 再開 → 終端で user に diff 提示。（Phase 2 の 🔴 は spec-designer が再 invoke される。adr-editor が呼ばれるのは Phase 1 信号が ADR 側の修正を要求する場合のみ。）
- Bad: `/track:plan "feature X"` を実行、内部で ADR を自動生成 (spec に合わせて decision を後付けする rationalization の温床になる)。
- Bad: main が `Edit` tool で `knowledge/adr/xxx.md` を直接書き換える (1 ファイル 1 writer 原則違反)。

## Exceptions

- **既存 ADR 流用**: 本機能の設計判断が既存 ADR でカバーされているなら新規作成は不要。
- **緩和モードなし**: 「ADR 未整備でも stub で進行を許す」モードは意図的にサポートしない。必要性が実証された時点で別 ADR で検討する。

## Review Checklist

- [ ] track 内成果物 (`track/items/<id>/` 配下) に ADR を配置していないか
- [ ] ADR ファイルに `Status` / `approved` 等の状態フィールドを追加していないか
- [ ] `/track:plan` 起動前に ADR が `knowledge/adr/` に存在するか (厳密モード条件を満たすか)
- [ ] ADR の back-and-forth 修正で main が直接編集せず adr-editor を経由しているか
- [ ] ADR からの参照が track 内成果物を指していないか (SoT Chain 逆流禁止)

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本 convention の原典となる ADR はこの索引から辿る
- [knowledge/conventions/adr.md](./adr.md) — ADR 運用の基本ルール
- [knowledge/conventions/workflow-ceremony-minimization.md](./workflow-ceremony-minimization.md) — 事後レビュー方式 / 事前承認限定の原則
