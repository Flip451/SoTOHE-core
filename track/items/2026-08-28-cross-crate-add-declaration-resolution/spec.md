<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 12, yellow: 0, red: 0 }
---

# 参照先 crate の add 宣言を解決集合に加える

## Goal

- [GO-01] 同じ track の層をまたぐ宣言先行の型・trait 参照を、完全修飾 identity を保った fail-closed な解決集合で実現する。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1, knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2]

## Scope

### In Scope
- [IN-01] ある層の解決集合に、同じ track の他の TDDD 有効層の catalogue が add 宣言した型と trait を、宣言層 crate の外部項目として加えること。参照側 catalogue に重複する記述は要求しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T001, T002, T004, T005]
- [IN-02] 他層の add 宣言から合成する項目の identity と配置を、宣言層 catalogue の crate 名、既存の bin-target root 正準化、および宣言層自身の module 解決に従わせること。参照側 rustdoc paths に同一 identity がある場合は rustdoc 項目を優先する。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2] [tasks: T001, T002, T003]

### Out of Scope
- [OS-01] 参照側 catalogue に cross-crate 宣言を追加して、宣言を二重化する方式。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T002]
- [OS-02] cross-crate 参照に限って短名 fallback を復活させる方式。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T003]
- [OS-03] cross-crate 参照を実装まで未解決のまま残す方式。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T003]

## Constraints
- [CO-01] 解決集合は一箇所で構築し、既存の自層 add 宣言の入力に他層 add 宣言を加える。経路ごとの add 型特例を新設しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T001, T002, T003, T004, T005]
- [CO-02] 対象となる他層の集合は architecture-rules.json が定める TDDD 有効層に委ね、catalogue ファイルがない層は宣言なしとして扱う。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T001, T002, T005]
- [CO-03] 合成項目は宣言層の crate 名を identity root とし、bin-target alias は既存の正準化を通す。module_path は明示時はその配置を用い、省略時は既存の配置未確定規則に従う。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] ある TDDD 有効層が add 宣言した未実装の型又は trait を、同じ track の別の TDDD 有効層が参照できる。参照側 catalogue には、その外部項目を重複記述しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T001, T002, T004, T005]
- [ ] [AC-02] 合成された cross-crate 項目は、宣言層の crate 名を root とする fully-qualified identity で解決される。bin target の crate 名と rustdoc root 名が異なる場合も既存の正準化により解決され、明示 module_path 又は省略時の配置未確定の扱いは宣言層自身の規則と一致する。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3] [tasks: T001, T002]
- [ ] [AC-03] 参照側の rustdoc paths が cross-crate add 宣言と同じ identity を持つ場合、rustdoc 項目が解決に用いられ、同一項目は合成されない。未根拠の cross-crate 参照は短名 fallback 又は実装までの未解決許容によって通過しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T001, T003, T005]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 12  🟡 0  🔴 0

