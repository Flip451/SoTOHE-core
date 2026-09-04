<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 11, yellow: 0, red: 0 }
---

# rustdoc の child item 除外を番兵 id ではなく kind で判定する

## Goal

- [GO-01] 型シグナル評価器の構造照合が、通常の rustdoc child item を id の数値にかかわらず完全に参照し、`Id(0)` を持つ struct field を含む型を誤って 🟡 と判定しないようにする。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1]

## Scope

### In Scope
- [IN-01] 構造照合の child-item traversal では、crate root を含む rustdoc の Module kind の item だけを除外し、id の特定値を除外条件にしない。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]
- [IN-02] 構造照合が作る実装側の参照集合に、`Id(0)` が割り当てられた struct field を含むすべての通常 field を含める。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]

### Out of Scope
- [OUT-01] rustdoc JSON の format_version ごとに child-item 除外規則を分岐させない。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]
- [OUT-02] 実装側の参照集合の欠落を隠すために、catalogue から該当 field の宣言を削除しない。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]
- [OUT-03] type-path placeholder として既存テストで使われる `Id(0)` の意味や、その child-item traversal と無関係な使用箇所を変更しない。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]

## Constraints
- [CN-01] child-item traversal の除外可否は rustdoc が明示する item kind から決め、id の数値へ特別な意味を与えない。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]
- [CN-02] 修正対象は型シグナル評価器の構造照合における child-item traversal に限定し、通常の field を失わせる fail-open な回避策を導入しない。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `Id(0)` を struct field に割り当てた固定 rustdoc fixture で、構造照合の実装側参照集合がその field を含む全 field を保持することを検証する。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]
- [ ] [AC-02] child-item traversal が crate root を含む Module kind の item を除外しつつ、Module ではない `Id(0)` の item を除外しないことを検証する。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]
- [ ] [AC-03] catalogue で field を省略せず、`Id(0)` field を持つ型の structural signal が実装側参照集合との完全な照合により 🔵 と評価され、誤って 🟡 にならないことを検証する。 [adr: knowledge/adr/2026-09-04-0049-rustdoc-child-exclusion-by-kind.md#D1] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 11  🟡 0  🔴 0

