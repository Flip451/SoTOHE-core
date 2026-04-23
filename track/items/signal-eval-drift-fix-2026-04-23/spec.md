<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 21, yellow: 0, red: 0 }
---

# Signal evaluation drift fix: align evaluate_requirement_signal and validate_track_snapshots with ADR §D3.1 / §D3.2 / §D0.0 / §D1.4

## Goal

- [GO-01] evaluate_requirement_signal (および type catalogue 側の同等関数) の signal 評価 logic を ADR §D3.1 / §D3.2 が規定する informal-priority rule に準拠させ、adr_refs 非空 + informal_grounds 非空 のケースが Yellow を返すよう修正する [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.2]
- [GO-02] validate_track_snapshots / verify-track-metadata の責務を ADR §D0.0 / §D1.4 の Phase 責務分離に整合させ、Phase 0 直後 (metadata.json のみ、plan.md 未 render) の状態で verify-track-metadata が pass するよう修正する [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D0.0, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D1.4]
- [GO-03] ADR §D3.1 および §D3.2 内の max() 表現行を削除し、ADR テキストを修正済みの informal-priority rule に整合させる [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.2]

## Scope

### In Scope
- [IN-01] libs/domain/src/spec.rs の evaluate_requirement_signal logic を informal-priority に修正する: Blue の必要条件を adr_refs 非空 かつ informal_grounds 空 とし、adr_refs 非空 + informal_grounds 非空 は Yellow を返す。関数シグネチャは変更しない [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1] [tasks: T001]
- [IN-02] libs/domain/src/tddd/signals.rs 内に evaluate_requirement_signal と同等の drift (adr_refs 非空時に informal_grounds を無視して Blue を返す) が存在するか調査し、存在すれば同一 informal-priority rule で修正する。調査の結果 drift がない場合はスキップする [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.2] [tasks: T002]
- [IN-03] ADR §D3.1 の 総合 signal = max(adr_refs, informal_grounds) 表現行を削除する。他の field-level signal 定義行 (adr_refs / informal_grounds / convention_refs の個別定義) および anchor D3.1 セクション見出しは維持する。ADR §D3.2 の 総合 signal = max(spec_refs, informal_grounds) 表現行も同様に削除し、他の行と anchor は維持する [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.2] [tasks: T004]
- [IN-04] libs/infrastructure/src/track/render.rs の validate_track_snapshots を Phase 責務分離に整合させる: plan.md が存在しない snapshot では plan.md content の整合チェックを skip し、I/O error を返さない。実装方針として (a) plan.md missing 時は content check をスキップ (registry.md の if registry_path.is_file() パターンと同様) または (b) validate を identity-only と view-freshness の 2 関数に分離し verify-track-metadata は identity-only を呼ぶ、の選択は実装者に委ねる [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D0.0, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D1.4] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T003]
- [IN-05] 関連 unit test を更新および追加する: (a) spec.rs の test_requirement_signal_adr_refs_take_priority_over_informal を論理反転または補完し、adr_refs 非空 + informal_grounds 非空 → Yellow を検証するテストとして整合させる。(b) adr_refs 非空 + informal_grounds 非空 → Yellow を独立した新 case として追加する。(c) IN-04 の修正に対する unit test (plan.md missing 時に verify-track-metadata が pass すること) を追加する [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1] [conv: .claude/rules/05-testing.md#Test Structure] [tasks: T001, T003]
- [IN-06] .claude/agents/spec-designer.md に以下 3 点の判断基準を追記し、spec-designer agent が autonomous に判断できるようにする: (1) universal coding principle (`.claude/rules/04-coding-principles.md` §No Panics in Library Code、hexagonal boundary rules 等) は spec の top-level `related_conventions[]` に属し、per-element `constraints[]` / `acceptance_criteria[]` / `in_scope[]` の convention_refs に置くのは不適切であること。(2) convention_refs[] は ADR §D3.1 により signal 評価対象外であり、adr_refs[] および informal_grounds[] が両方空の element は 🔴 Red と評価されること。(3) informal_grounds[] が非空 (🟡 Yellow) の element はマージ前に (a) adr_refs[] へのプロモート、(b) related_conventions[] への移動 (universal rule の場合)、(c) element の削除、のいずれかで解消すること [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1] [tasks: T005]

### Out of Scope
- [OS-01] evaluate_requirement_signal の関数シグネチャ変更 (2 引数構造を維持) [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1]
- [OS-02] convention_refs[] の signal 評価対象化 (convention_refs は引き続き signal 評価対象外。ADR §D3.1 の convention_refs: signal 評価対象外 記述は維持) [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1]
- [OS-03] verify-latest-track-local および Phase 1/2/3 の他の verify ゲートの統廃合 (本 track は verify-track-metadata の Phase 責務整合のみ) [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D6.1]
- [OS-04] catalogue-signal (型契約 → 仕様書 signal) の全面実装 (IN-02 で drift を修正する場合も signal 全面実装は別 ADR 範囲) [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.2]
- [OS-05] ADR §D3.1 / §D3.2 の field-level signal 定義行 (adr_refs: 🔵/🔴、informal_grounds: 空→🔵/非空→🟡、convention_refs: 対象外) の変更 (max() 表現行のみ削除し、定義行は維持) [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.2]

## Constraints
- [CN-01] cargo make ci が全項目 pass すること (fmt-check + clippy + nextest + test-doc + deny + check-layers + verify-*) [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D6.2] [conv: .claude/rules/07-dev-environment.md#Pre-commit Checklist] [tasks: T006]
- [CN-02] evaluate_requirement_signal および相当する type catalogue 側関数のシグネチャを変更しない。呼び出し元の修正は不要 [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1] [tasks: T001, T002]
- [CN-03] ADR §D3.1 の convention_refs: signal 評価対象外 記述は変更しない [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1] [tasks: T004]

## Acceptance Criteria
- [ ] [AC-01] evaluate_requirement_signal(adr_refs=[x], informal_grounds=[y]) が Yellow を返すことを独立した新 unit test で検証する。adr_refs 非空 + informal_grounds 空 → Blue、両方空 → Red の既存ケースも引き続き pass する [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1] [tasks: T001]
- [ ] [AC-02] ADR §D3.1 の max(adr_refs, informal_grounds) 記述行および §D3.2 の max(spec_refs, informal_grounds) 記述行が ADR ファイルから削除されている。anchor D3.1 / D3.2 のセクション見出しおよび field-level 定義行 (adr_refs / informal_grounds / convention_refs の個別定義) は維持されている [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D3.2] [tasks: T004]
- [ ] [AC-03] Phase 0 直後の状態 (track/items/<id>/ に metadata.json のみ存在、plan.md 未 render) で cargo make verify-track-metadata が pass する。plan.md が欠如した snapshot で I/O error が返らない [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D0.0, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D1.4] [tasks: T003]
- [ ] [AC-04] cargo make ci の全項目が pass する [adr: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md#D6.2] [conv: .claude/rules/07-dev-environment.md#Pre-commit Checklist] [tasks: T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 21  🟡 0  🔴 0

