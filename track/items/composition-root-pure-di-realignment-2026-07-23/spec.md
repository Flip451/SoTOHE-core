<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 16, yellow: 0, red: 0 }
---

# Composition Root Pure DI Realignment

## Goal

- [GO-01] 出荷される composition-root の正例、CLI 呼出しフロー、およびその enforcement surface を純 DI の composition-root 境界へ整合させ、composition root が完全に配線した primary adapter を渡すだけの状態にする。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D2]

## Scope

### In Scope
- [IN-01] 出荷 placeholder と CLI call flow を、composition root が secondary adapter・interactor・driver を構築・配線して primary adapter を返し、CLI が parse → adapter 取得 → handle → emit の順で実行する正例へ整合させる。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D2] [tasks: T003]
- [IN-02] composition review policy が、composition 上の request-to-result 実行メソッドと composition の公開面への domain・usecase・internal role 型の露出を逸脱として検出し、純 DI の成果物として返される PrimaryAdapter は許可する状態にする。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D3] [tasks: T002]
- [IN-03] composition root の公開面規律を既存 catalogue-lint mechanism で強制し、実行メソッドまたは禁止された公開型露出の逸脱を検出できる状態にする。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D5] [tasks: T001]
- [IN-04] 既存の internal composition facade を既知乖離として登記し、各 bounded context を実質的に改修する将来 track で opportunistic に純 DI へ移行する方針を可視化する。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D4] [tasks: T003]

### Out of Scope
- [OUT-01] sotp 本体にある全 composition root の big-bang な純 DI 移行は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D4] [tasks: T003]
- [OUT-02] 新規 source-level verify の導入、および catalogue-lint の具体的な schema・設定・型形状の決定は本 track の behavioral contract の対象外とする。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D5] [tasks: T001]

## Constraints
- [CN-01] composition root は object graph の構築・配線と fully wired PrimaryAdapter の引渡しだけを担い、request を受けて result を返す実行メソッドを公開してはならない。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D3] [tasks: T002, T003]
- [CN-02] 出荷 placeholder の composition 公開面は domain 型・usecase error 型・internal role 型を露出せず、入力検証を primary adapter 側の振る舞いに置く。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D2, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D3] [tasks: T002, T003]
- [CN-03] 公開面規律の enforcement は既存 catalogue lint を使用し、source-level 検査を追加しない。具体的な lint schema と type-shape の選択は後続 phase に委ねる。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D5] [tasks: T001]
- [CN-04] 既知の internal composition facade は、本 track で一括移行を要求せず、対象 context の実質的な改修 track が発生したときに当該 track に純 DI 化を含める。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D4] [tasks: T003]

## Acceptance Criteria
- [ ] [AC-01] 出荷 placeholder の composition root は fully wired PrimaryAdapter を返し、公開面に request-to-result 実行メソッドを持たない。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D2] [tasks: T003]
- [ ] [AC-02] CLI は入力を parse した後に composition root から PrimaryAdapter を取得し、その adapter の handle を呼び、結果を emit する。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D2] [tasks: T003]
- [ ] [AC-03] composition review policy は、interactor 呼出しと driver 呼出しのいずれを行う execution method も逸脱として検出し、domain・usecase・internal role 型の composition 公開面への露出も逸脱として検出する一方、返却される PrimaryAdapter は許可する。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D3] [tasks: T002]
- [ ] [AC-04] 既存 catalogue lint は composition root の execution-method deviation と prohibited public-surface exposure deviation を検出し、適合する public surface を通過させる。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D5] [tasks: T001]
- [ ] [AC-05] sotp 本体の既存 internal composition facade は既知乖離として記録され、全件の即時移行を要求せず、各 context の実質的な改修時に opportunistic 移行する方針が維持される。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D4] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/type-designer-kind-selection.md#R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/track-lifecycle.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 16  🟡 0  🔴 0

