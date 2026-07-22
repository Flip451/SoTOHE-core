<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 20, yellow: 0, red: 0 }
---

# SoT 再入の順次処理規律 — ルーティング後のフェーズ収束 Prerequisite

## Goal

- [GO-01] rollback-diagnoser により上流 SoT へ回帰した後、各下流 writer phase が直上流の再収束を待ってからのみ再開する順次処理規律を、track 運用文書に一貫して定着させる。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D1, knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3]

## Scope

### In Scope
- [IN-01] `knowledge/conventions/sot-reentry-sequencing.md` を追加し、phase 収束を参照 signal、該当 ref-verify scope、該当 SoT scope review の `zero_findings` の三要素として、既存 SSoT を参照して規定する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2] [tasks: T1]
- [IN-02] `knowledge/conventions/sot-reentry-sequencing.md` は rollback-diagnoser、orchestrator、writer capability の責務分離を規定し、`.harness/workflows/track/diagnose.md` は routing 後かつ各再入 writer dispatch 前に、orchestrator が参照 signal・該当 ref-verify scope・該当 SoT scope review の `zero_findings` と直上流 1 層を確認し、その収束証跡を writer briefing に渡すことを規定する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D1, knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2, knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3] [tasks: T1, T2, T3, T4, T5]
- [IN-03] 新 convention は、spec-design、type-design、impl-plan、implementation の再開時に、それぞれ ADR、spec、catalogue、または catalogue と impl-plan review という直上流の収束だけを prerequisite として確認する規則を定める。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3] [tasks: T1, T2, T3]
- [IN-04] 新 convention は、下流作業中に上流 SoT の編集必要性が判明した場合に下流を即時中断して上流へ戻し、上流の再収束まで再開を禁止する規則を定める。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D4] [tasks: T1, T2, T3]
- [IN-05] `spec-designer`、`type-designer`、`impl-planner`、`implementer` の capability contracts に、満たされない prerequisite を含む briefing を作業せず orchestrator へ返す規定と新 convention への pointer を加える。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D1] [tasks: T2, T3]
- [IN-06] 前記 writer capability contracts は、再開 prerequisite を各 phase の直上流 1 層だけの収束として明示する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3] [tasks: T2, T3]
- [IN-07] `rollback-diagnoser` capability contract に、回帰先の診断・勧告と、diagnosis 後に orchestrator が順次再入規律を適用する責務との境界を示す cross-reference pointer を加える。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D1] [tasks: T4]
- [IN-08] `knowledge/conventions/README.md` の convention index を、追加した SoT 再入規律 document が参照可能な状態へ再生成する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D6] [tasks: T1]

### Out of Scope
- [OUT-01] `.harness/config/signal-gates.json`、signal gate、CI、または `adr_user` 評価の実装・許容値を変更しない。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D6] [tasks: T1, T2, T3, T4]
- [OUT-02] impl-plan task status transition 以外の上流 SoT 編集を収束失効の例外として追加・一般化しない。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D5] [tasks: T1]

## Constraints
- [CN-01] 新 convention は signal の許容値や ref-verify scope の対応を複製せず、`.harness/config/signal-gates.json` と既存 ref-verify 定義をそれぞれの SSoT として参照する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2] [tasks: T1]
- [CN-02] 上流 SoT の編集が必要と判明した時点で下流作業を中断し、回帰先が自明でない場合は rollback-diagnoser を経由する。上流の再収束前に下流を継続または再開する裁量経路を設けない。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D4] [tasks: T1, T2, T3, T4]
- [CN-03] この track は prompt-level discipline と documentation + review による運用に留め、人工的な phase-state field、追加承認 ceremony、または新しい機械的 enforcement を導入しない。将来の mechanism 化は ADR の Reassess When に従って別途検討する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D6] [conv: knowledge/conventions/workflow-ceremony-minimization.md#Rules, knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T1, T2, T3, T4]

## Acceptance Criteria
- [ ] [AC-01] 新 convention は、phase 収束を参照 signal、該当 `bin/sotp ref-verify` scope、該当 SoT scope review の `zero_findings` の三要素として扱い、各要素の具体的な許容値・対応表は既存 SSoT を参照する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2] [tasks: T1]
- [ ] [AC-02] writer capability documents は spec-design、type-design、impl-plan、implementation の各再開時に、ADR、spec、catalogue、または catalogue と impl-plan review という ADR で定められた直上流 prerequisite だけを確認し、満たせない briefing を orchestrator へ返す。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3] [tasks: T2, T3]
- [ ] [AC-03] 下流作業中に上流 SoT の編集必要性を発見した場合、capability document は即時中断と上流への return を指示し、上流の再収束まで下流を続行・再開させない。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D4] [tasks: T1, T2, T3, T4]
- [ ] [AC-04] impl-plan の `bin/sotp track transition` による task status transition だけは review 収束を失効させない明示例外として記述され、他の SoT へ同種の例外を拡張しない。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D5] [tasks: T1]
- [ ] [AC-05] 実装差分は新 convention、指定された五つの capability documents、`.harness/workflows/track/diagnose.md`、および再生成された conventions index に限定され、gate、CI、signal-gates configuration、`adr_user` evaluator に変更を含まない。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D6] [tasks: T1, T2, T3, T4, T5]
- [ ] [AC-06] `.harness/workflows/track/diagnose.md` は routing 後かつ各再入 writer dispatch 前に、orchestrator が参照 signal、該当 ref-verify scope、該当 SoT scope review の `zero_findings`、および直上流 1 層を確認し、その収束証跡を writer briefing に渡す手順を明記する。 [adr: knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D1, knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2, knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3] [tasks: T5]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/adr.md#ADR vs Convention

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 0  🔴 0

