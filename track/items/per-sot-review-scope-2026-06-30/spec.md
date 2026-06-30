<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 28, yellow: 0, red: 0 }
---

# 内容レビューの SoT 別スコープ化

## Goal

- [GO-01] `review-scope.json` の `plan-artifacts` 単一スコープを廃止し、SoT chain の phase 境界に対応した `adr` / `spec` / `types` / `impl-plan` の 4 スコープへ分割することで、内容レビュー軸を signal / ref-verify / task-contract / rollback-diagnoser が共有する SoT 境界認識に揃える。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D1]
- [GO-02] 各 SoT スコープに専用の briefing ファイルを持たせることで、振る舞い契約（spec）・型設計（types）・タスク分解（impl-plan）・意思決定（adr）という固有の評価観点を有効にする。特に `types` スコープでは SOLID / CQRS / DRY などの一般コーディング原則に照らした型設計レビューを可能にする。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D5]
- [GO-03] `plan-artifacts` をスコープ名として参照する既存箇所（`review-scope.json`・full-cycle lifecycle tail commit・rollback-diagnoser・ソーステストフィクスチャ）を新スコープ構成へ同時に移行し、`plan-artifacts` 参照を本 track で根絶する。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6]

## Scope

### In Scope
- [IN-01] `review-scope.json` の `plan-artifacts` グループを削除し、`adr` / `spec` / `types` / `impl-plan` の 4 グループを新設する。各グループは当該 SoT の評価観点に特化した dedicated briefing ファイル（`.harness/custom/review-prompts/<scope>.md`）を参照する。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D1, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D5] [tasks: T001]
- [IN-02] D2 のファイル割当表に従い、4 スコープのパターンと `review_operational` 追加項目を設定する。`adr` スコープ: `knowledge/adr/**`・`knowledge/research/**`。`spec` スコープ: `track/items/<track-id>/spec.json`・`track/items/<track-id>/spec.md`。`types` スコープ: `track/items/<track-id>/*-types.json`（全 layer 一括）・`track/items/<track-id>/contract-map.md`。`impl-plan` スコープ: `track/items/<track-id>/impl-plan.json`・`task-coverage.json`・`task-contract.json`・`plan.md`・`observations.md`。`metadata.json` と `<layer>-types.md` は `review_operational` へ追加してレビュー対象外とする。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D3] [tasks: T001]
- [IN-03] `types` スコープはすべての layer の `*-types.json` ファイルを layer 別分割なく 1 スコープに束ねる（layer-unified）。`contract-map.md` も `types` スコープに同梱する。`<layer>-types.md` は SSoT の機械的 Markdown 化ビューで付加情報が乏しいため `review_operational` に退避し、`types` スコープには含めない。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D4, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2] [tasks: T001]
- [IN-04] `.harness/custom/review-prompts/` に `adr.md` / `spec.md` / `types.md` / `impl-plan.md` の 4 briefing ファイルを新設する。各ファイルは当該 SoT 固有の評価観点を持つ severity policy を記述する。`types.md` には SOLID / CQRS / DRY などの一般コーディング原則に照らした型設計レビュー指針を明示的に含める。既存 `plan-artifacts.md` の severity policy を各 briefing へ分解・移行する。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D5] [tasks: T002]
- [IN-05] `plan-artifacts` スコープ名を参照する既存箇所をすべて新スコープ名へ移行する。対象: (a) `review-scope.json` の `plan-artifacts` グループ定義、(b) `.harness/workflows/track/full-cycle.md` の lifecycle tail commit ステップ（`--scope plan-artifacts` → `--scope impl-plan`）、(c) rollback-diagnoser / track-diagnose 関連ドキュメントのトリガー記述（旧 `plan-artifacts findings` を `adr` / `spec` / `types` / `impl-plan` の各 SoT review finding を診断対象にする記述へ更新）、(d) `knowledge/conventions/enforce-by-mechanism.md` の ADR review scope / semantic review 記述（`plan-artifacts` → `adr`）、(e) ソーステストフィクスチャ（`fixpoint_resolve.rs`・`track_phase.rs`・`review_v2` 関連の domain / usecase / infrastructure / cli-composition / cli 各 crate）。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T001, T003, T004]

### Out of Scope
- [OS-01] `types` スコープの layer 別分割（`domain-types` / `usecase-types` / … を個別スコープにする案）は本 track の対象外とする。finding 傾向が layer 別評価観点の必要性を実証するまで判断を保留する（D4 Reassess）。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D4] [tasks: T001]
- [OS-02] `plan-artifacts` スコープ名の後方互換エイリアスや移行シムの提供は行わない。D1 の廃止決定は互換レイヤーなしの全面削除であり、参照箇所は D6 の同時移行で全て更新する。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D1, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T001]
- [OS-03] `bin/sotp` サブコマンドや CI architecture check への「4 スコープ構成の網羅性検証」機能の一般機構としての追加は行わない。スコープ構成の enforcement は `review-scope.json` の定義と reviewer briefing に限定し、CI 一般機構としての追加は本 ADR の決定範囲外とする。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T005]

## Constraints
- [CN-01] `types.md` briefing には型設計を SOLID / CQRS / DRY などの一般コーディング原則に照らしてレビューする旨を明示的に含めなければならない。D5 に記録されたユーザーの明示的な要求であり、省略または曖昧な言及は不可。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D5] [tasks: T002]
- [CN-02] `metadata.json`（identity-only）と `<layer>-types.md`（型カタログの Markdown 化ビュー）は `review_operational` に列挙し、4 つの新スコープのいずれのパターンにも含めない。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2] [tasks: T001]
- [CN-03] `plan-artifacts` スコープは廃止であり、4 つの新スコープとの並存は認めない。`plan-artifacts.md` は severity policy を 4 briefing へ分解移行後に削除または空にし、`review-scope.json` の `groups` から `plan-artifacts` エントリを除去する。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D1, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D5] [tasks: T001, T002]
- [CN-04] D6 移行は網羅的であること。IN-05 で列挙したすべての参照箇所を本 track で更新し、`plan-artifacts` をスコープ名として使用する参照が残存しない状態を実装完了の必要条件とする。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T003, T004, T005]

## Acceptance Criteria
- [ ] [AC-01] `.harness/config/review-scope.json` の `groups` に `adr`・`spec`・`types`・`impl-plan` の 4 エントリが存在し、`plan-artifacts` エントリが存在しない。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D1] [tasks: T001]
- [ ] [AC-02] `review-scope.json` の `adr` スコープ patterns が `knowledge/adr/**` と `knowledge/research/**` を含む。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2] [tasks: T001]
- [ ] [AC-03] `review-scope.json` の `spec` スコープ patterns が `track/items/<track-id>/spec.json` と `track/items/<track-id>/spec.md` を含む。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D3] [tasks: T001]
- [ ] [AC-04] `review-scope.json` の `types` スコープ patterns が `track/items/<track-id>/*-types.json`（全 layer 一括）と `track/items/<track-id>/contract-map.md` を含み、layer 別分割エントリが存在しない。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D3, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D4] [tasks: T001]
- [ ] [AC-05] `review-scope.json` の `impl-plan` スコープ patterns が `track/items/<track-id>/impl-plan.json`・`task-coverage.json`・`task-contract.json`・`plan.md`・`observations.md` を含む。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D3] [tasks: T001]
- [ ] [AC-06] `review-scope.json` の `review_operational` に `track/items/<track-id>/metadata.json` および `track/items/<track-id>/*-types.md` に対応するパターンが含まれる。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2] [tasks: T001]
- [ ] [AC-07] `.harness/custom/review-prompts/adr.md`・`spec.md`・`types.md`・`impl-plan.md` の 4 ファイルが存在し、各ファイルが当該 SoT 固有の評価観点を含む severity policy を持つ。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D5] [tasks: T002]
- [ ] [AC-08] `types.md` briefing に SOLID・CQRS・DRY の各原則への明示的な言及が含まれる。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D5] [tasks: T002]
- [ ] [AC-09] `.harness/workflows/track/full-cycle.md` の lifecycle tail commit ステップが `plan-artifacts` スコープを参照せず、`impl-plan` スコープを参照する（`bin/sotp review results --scope impl-plan` 等として使用）。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T003]
- [ ] [AC-10] rollback-diagnoser / track-diagnose 関連ドキュメント（`.harness/capabilities/rollback-diagnoser.md`・`.claude/commands/track/diagnose.md`・`.claude/skills/diagnose/SKILL.md`・`.claude/agents/rollback-diagnoser.md`・`.agents/skills/rollback-diagnoser/SKILL.md`・`.codex/agents/rollback-diagnoser.toml`・`.codex/instructions.md`）のトリガー記述が `plan-artifacts` スコープ名を使用せず、`adr` / `spec` / `types` / `impl-plan` の各 SoT review finding を診断対象として明示する。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T003]
- [ ] [AC-11] `fixpoint_resolve.rs`（usecase / cli-composition crate）および `track_phase.rs`（domain crate）のテストフィクスチャ内で `plan-artifacts` 文字列がスコープ名として使用されない。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T004]
- [ ] [AC-12] `review_v2` 関連テスト（domain・infrastructure・cli 各 crate の `review_v2/tests.rs`・`scope_config_loader.rs`・`review/tests.rs` 等）が `plan-artifacts` 文字列をスコープ名として使用しない。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T004]
- [ ] [AC-13] 移行後に `track/items/<track-id>/` 配下のファイルを分類した結果、4 つの新スコープまたは `review_operational` に帰属しないファイルが存在しないことが確認されている（暗黙の `other` スコープへの意図せぬ落下がない）。 [adr: knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D2, knowledge/adr/2026-06-30-1549-per-sot-review-scope.md#D6] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/review-protocol.md#コミット前レビュー必須
- knowledge/conventions/pre-track-adr-authoring.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 28  🟡 0  🔴 0

