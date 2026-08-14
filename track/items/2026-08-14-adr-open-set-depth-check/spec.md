<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 12, yellow: 0, red: 0 }
---

# ADR 起草とレビューに開集合検査を追加する

## Goal

- [GO-01] 開集合の意味論を含む決定を、ADR 起草時と実装計画時の二段階で確認できる文書上の検査として定義する。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1, knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D2]

## Scope

### In Scope
- [IN-01] ADR 起草ヒアリングの Full モードと ADR レビュー prompt に、開集合の決定文面を確認する三択検査を追加する。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1] [tasks: T001, T002]
- [IN-02] impl-plan と types の reviewer briefing に、開集合をヒューリスティックで覆う実装方針を検出する観点を追加する。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D2] [tasks: T003, T004]

### Out of Scope
- [OUT-01] 開集合を示す句のパターン検出を CI lint として機械化することは対象外とする。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1] [tasks: T001, T002]
- [OUT-02] Rust のプロダクションコード、型、または実行時の検査を実装することは対象外とする。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1, knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D2] [tasks: T001, T002, T003, T004]
- [OUT-03] ADR 起草時の確認を省略し、実装後のレビューだけで開集合を検出する運用は対象外とする。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1] [tasks: T001]

## Constraints
- [CN-01] D1 の検査は ADR 起草ヒアリングの Full モードおよび ADR レビュー prompt に限定して配置する。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1] [tasks: T001, T002]
- [CN-02] D2 の検査は impl-plan と types の reviewer briefing に限定して配置する。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D2] [tasks: T003, T004]
- [CN-03] 開集合検査は既存の権威への委譲、保守的な過大近似、または厳密実装の深さ見積りを比較する意味論的な確認であり、単純な文言一致で代替しない。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] Full モードの ADR 起草ヒアリングと ADR 意味論レビュー prompt は、正確な列挙、完全な追跡、または「すべての X を Y する」型の決定文面について、既存の権威への委譲、保守的な過大近似、厳密実装の深さ見積りの三択確認を求める。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1] [tasks: T001, T002]
- [ ] [AC-02] impl-plan と types の reviewer briefing は、実装方針が開集合をヒューリスティックで覆おうとしている場合に、手作りのパーサ、リソース管理、またはビルドシステム模倣を指摘する観点を含む。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D2] [tasks: T003, T004]
- [ ] [AC-03] 検査は自然言語の意味を人が判断する文書上の確認として提供され、句パターンを検出する決定論的な CI lint を導入しない。 [adr: knowledge/adr/2026-08-13-1756-adr-open-set-depth-check.md#D1] [tasks: T001, T002]

## Signal Summary

### Stage 1: Spec Signals
🔵 12  🟡 0  🔴 0

