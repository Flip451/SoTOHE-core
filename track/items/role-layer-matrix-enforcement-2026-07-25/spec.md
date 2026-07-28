<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 16, yellow: 0, red: 0 }
---

# role × 層マトリクスを機構で強制し ValueObject の層勾配を是正する

## Goal

- [GO-01] R1 の role × 層配置規約に反するカタログ宣言を、起草者の自己規律ではなく active track の catalogue lint が検出・拒否し、ValueObject の配置根拠を review 可能な記録として残せるようにする。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D1, knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D2]

## Scope

### In Scope
- [IN-01] active track の catalogue lint は、R1 で各 role に許可された layer だけを受理し、許可外の role × layer 宣言を error として拒否する。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D1] [tasks: T001, T002, T003]
- [IN-02] ValueObject は domain、usecase、infrastructure のいずれに配置しても、配置の意味論的根拠を docs または track の review record に残し、reviewer がその根拠を照合できる。cli、cli_driver、cli_composition への配置は catalogue lint で拒否される。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D2] [tasks: T001, T002, T003]
- [IN-03] 出荷 catalogue-lint config と strict preset は同じ規則集合を提供し、production adapter によりいずれもデコード可能であることを回帰検査で確認できる。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D4] [tasks: T002, T003]

### Out of Scope
- [OUT-01] ValueObject の配置が `✓` か `△` かを lint が判定したり、根拠の意味論的妥当性を機械的に判定したりすることは対象外とする。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D2] [tasks: T001, T003]
- [OUT-02] 過去 track の catalogue を新しい role × layer 規則へ遡及適用、移行、または適合監査することは対象外とする。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D3] [tasks: T001, T002, T003]
- [OUT-03] grandfathered その他の適用除外フラグ、既存宣言用の段階的監査経路、または例外経路を新設することは対象外とする。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D3] [tasks: T001, T002, T003]
- [OUT-04] crate topology、外部観測可能な CLI 契約、または application boundary 内側への transport 型漏出を catalogue lint で追加検査することは対象外とする。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D1, knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D2] [tasks: T001, T002, T003]

## Constraints
- [CN-01] role × layer の許可方針は出荷 lint config を唯一の正とし、規則一覧や許可値を Rust test のリテラルへ写経して二重に維持してはならない。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D4] [tasks: T002, T003]
- [CN-02] 出荷 config の回帰検査は production adapter でのデコード可能性と config・preset の構造的一致に限り、特定 rule の存在や規則集合を別契約として検査してはならない。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D4] [tasks: T003]
- [CN-03] role × layer 制約の適用対象は active track の catalogue lint に限り、歴史的 track artifact を現在の規則に適合させるための特別扱いを設けない。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D3] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] R1 の全 role について、許可 layer の宣言が catalogue lint の出荷 config に存在し、許可外 layer を指定した active-track catalogue は lint error となる。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-02] 複数 layer へ置ける role を含め、R1 で `✗` の role × layer 組合せを持つ active-track catalogue は lint error となり、許可された組合せは layer constraint によって拒否されない。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-03] ValueObject の domain、usecase、infrastructure 配置は layer constraint で受理され、配置根拠を docs または track review record で reviewer が確認できる。一方で cli、cli_driver、cli_composition 配置は lint error となる。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D2] [tasks: T001, T002, T003]
- [ ] [AC-04] 出荷 config と strict preset は production adapter でそれぞれデコードでき、両者から得た規則集合は構造として一致する。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D4] [tasks: T002, T003]
- [ ] [AC-05] 出荷 config の回帰検査は config の規則値を Rust リテラルで再記述せず、撤去済み rule 種別の不在を個別 assertion として追加しない。 [adr: knowledge/adr/2026-07-25-0538-role-layer-matrix-enforcement.md#D4] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/type-designer-kind-selection.md#R1
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rules
- knowledge/conventions/coding-principles.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 16  🟡 0  🔴 0

