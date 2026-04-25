<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# type-designer の reconnaissance フェーズ追加 + orchestrator 向け出力整理

## Goal

- [GO-01] type-designer の internal pipeline 先頭に reconnaissance step (baseline-capture + type-graph 実行 + Read) を挿入し、catalogue draft 段階で既存型インベントリ (種別 / partition / 命名規則) を把握した状態で kind・action 判定ができるようにすることで、低レベル review 指摘 (「既存にあった」「kind 違い」) の発生を構造的に抑制する [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]
- [GO-02] type-designer の orchestrator 向け final message を Signal evaluation + Open Questions の 2 セクションに絞り込み、Entries written / Action rationale / Cross-partition migrations の 3 セクションを削除することで、type-designer 1 回の invocation あたりの戻り値を小さくし、親 ADR が定める「orchestrator 向け責務は信号機評価のみ完結」の実装と定義を一致させる [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D2, knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#D1]

## Scope

### In Scope
- [IN-01] .claude/agents/type-designer.md の Internal pipeline セクションを改訂し、新 7-step 順序 (baseline-capture → type-graph → Read → catalogue draft → Write → contract-map → type-signals) を反映する。step 3 の Read 対象は --cluster-depth 値に応じた 2 ケース (<layer>-graph.md / <layer>-graph/index.md + per-cluster ファイル群) を両方明記する [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]
- [IN-02] .claude/agents/type-designer.md の Output セクション (lines 96 onward) を改訂し、Entries written / Action rationale / Cross-partition migrations の 3 セクションを削除する。残すセクションは per-layer の Signal evaluation と末尾の Open Questions のみとする [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D2]
- [IN-03] Mission セクションの「Reconnaissance first」段落を、Internal pipeline の新 7-step 設計と矛盾なく整合させる (既に部分的に追加済みの段落を確認し、pipeline セクションとの一貫性を保つ) [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]

### Out of Scope
- [OS-01] type-graph の --cluster-depth レンダリングオプション最適値の確定: 別途調査用トラックで評価し、得られた判断基準を後続 ADR / convention として固定する。本 track では「depth 値ごとに Read 対象が異なる」という事実を記述するのみ [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]
- [OS-02] Rust ソースコードへの変更: 本 track の成果物は .claude/agents/type-designer.md の agent 定義ファイル編集のみ。CLI 追加 / domain 純粋関数 / usecase interactor / infrastructure adapter への変更は含まない [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]
- [OS-03] /track:type-design command 本体 (.claude/commands/ 配下) の挙動変更: command 本体は subagent invocation + signal 受け取りのみであり、reconnaissance step の追加は subagent 内部 pipeline に閉じる。command 定義ファイルは変更対象外 [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1, knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#D1]
- [OS-04] reconnaissance step で取得した既存型インベントリ情報を orchestrator に echo する設計: インベントリは <layer>-graph/ / baseline / catalogue 自体に残っているため、orchestrator が必要なら直接 Read できる。subagent の final message に含める必要はない [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D2]
- [OS-05] 他 subagent 定義ファイル (spec-designer.md / impl-planner.md / adr-editor.md) への同様の整理適用: 「ADR 範囲を超えた余計な output sections」問題が他 subagent で表面化した場合は別 ADR / 別 track で対応する [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]

## Constraints
- [CN-01] .claude/agents/type-designer.md 以外のファイルは変更しない。特に knowledge/adr/ 配下、knowledge/conventions/ 配下、Rust ソース、および他 subagent 定義ファイルへの変更は禁止 [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]
- [CN-02] reconnaissance step (baseline-capture / type-graph / Read) で得た既存型インベントリは subagent 内部の探索情報として扱い、orchestrator への final message には含めない [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D2]
- [CN-03] reconnaissance を orchestrator (command 本体) 側で実行して briefing に埋め込む設計は採用しない。subagent 内部 pipeline として完結させ、command 本体は subagent invocation + 結果受け取りのみに留める (親 ADR D1 / Rejected Alternative B の確認) [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1, knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#D1]
- [CN-04] catalogue draft 後に baseline / type-graph 出力との整合性を後付け検証する post-hoc validation 方式は採用しない。reconnaissance を pipeline 先頭に置き draft 前に既存インベントリを把握する設計を維持する (Rejected Alternative A の確認) [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]

## Acceptance Criteria
- [ ] [AC-01] .claude/agents/type-designer.md の Internal pipeline セクションが 7-step 構造 (baseline-capture → type-graph → Read → catalogue draft → Write → contract-map → type-signals) を明示しており、step 3 の Read 対象として --cluster-depth 0 (単一ファイル) と --cluster-depth >= 1 (cluster directory) の 2 ケースが両方記述されている [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]
- [ ] [AC-02] .claude/agents/type-designer.md の Output セクションが Signal evaluation (per-layer) と Open Questions の 2 セクションのみを定義しており、Entries written / Action rationale / Cross-partition migrations の 3 セクションが存在しない [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D2]
- [ ] [AC-03] .claude/agents/type-designer.md の Mission セクションに「reconnaissance は内部探索のみ — orchestrator への final message には含めない」という趣旨の記述があり、Internal pipeline の新 7-step 設計と矛盾しない [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1, knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D2]
- [ ] [AC-04] reconnaissance step の記述が「catalogue draft の前に既存コードベースの型インベントリを把握する」という目的を明示しており、skip してはならない旨が pipeline 仕様として記述されている [adr: knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md#D1]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

