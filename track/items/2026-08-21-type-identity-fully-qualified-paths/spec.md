<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 24, yellow: 0, red: 0 }
---

# 型・トレイト識別を完全修飾パスへ移行し、同名宣言を安全に解決する

## Goal

- [GO-01] 型シグナル評価で型・トレイトを rustdoc の paths を権威とする完全修飾パスで識別し、短名の衝突によって別の対象が同一視されないようにする。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1]
- [GO-02] catalogue 利用者が短名を通常の宣言として使い続けつつ、同一 crate 内の同名型・同名トレイトを一つの catalogue で区別して表現・評価できるようにする。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1]

## Scope

### In Scope
- [IN-01] catalogue の types / traits 宣言、trait_impls、inherent_impls、および in-crate TypeRef を完全修飾パスへ解決して識別に使用できるようにする。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T002, T004, T011]
- [IN-02] 評価器の type / trait identity map、参照解決、impl・generic 引数比較、型シグナル出力の owner 結合、および contract-map の node / edge 解決を完全修飾パス識別へ追従させる。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T005, T006, T011, T012]
- [IN-03] catalogue-lint の指定三規則、test-obligation derive の trait role index、task-contract と pre-review gate、catalog add / cite / import の entry-key 書込みを、同名併存と完全修飾パス解決に整合させる。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T007, T008, T009, T010, T013, T014, T015, T016]
- [IN-04] 短名が一意に解決できない宣言と完全修飾パスを解決できない対象を fail-closed とし、前者では候補の完全修飾パス、後者では未解決対象を診断する。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T004, T011]
- [IN-05] catalogue 文書の schema_version を任意の数値一般と区別される schema version の値として扱い、既存の受理 version 意味論、数値としての読出し、および外部 JSON の数値表現を維持する。 [adr: knowledge/adr/2026-08-21-0100-catalogue-schema-version-value-object.md#D1] [tasks: T003]

### Out of Scope
- [OS-01] functions の key、cross-crate 参照の catalogue 表記、および catalog import --type の既存の完全一致照合は変更しない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T010, T016, T017]
- [OS-02] raw rustdoc baseline の wire format と、rustdoc paths を基準とする baseline-graph renderer は変更しない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T017]
- [OS-03] 通常の renderer 表示ラベルと非曖昧時の診断表示は短名のまま保ち、型名または TypeRef を解決しない指定の lint・anchor・検索機能は変更しない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T006, T007, T008, T012, T013, T014, T017]
- [OS-04] 同名宣言を表す catalogue の具体的な宣言形式をこの behavioral contract で固定せず、重複側の改名や同名禁止 lint を恒久的な解決策にはしない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T002, T011]

## Constraints
- [CO-01] 識別の権威は rustdoc paths から得る完全修飾パスであり、短名へ暗黙に縮退してはならない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T002, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016]
- [CO-02] catalogue の型・トレイト宣言は短名を既定とし、完全修飾パスは必要な場合にのみ併記可能とする。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T002, T004, T010, T016]
- [CO-03] 曖昧または未解決の識別は成功や推測ではなく fail-closed とし、利用者が解消に必要な対象情報を診断から得られるようにする。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T004, T011]
- [CO-04] D1 が列挙する変更面は単一の fallback 修正に縮約せず、相互に独立した変更単位として段階的に実施して整合させる。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017]

## Acceptance Criteria
- [ ] [AC-01] 同一 crate 内の同名型および同名トレイトを一つの catalogue に同時に宣言でき、評価器はそれぞれを異なる完全修飾パスの対象として扱う。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T002, T004, T011]
- [ ] [AC-02] 一意に解決できる短名の型・トレイト宣言と in-crate TypeRef は完全修飾パスへ解決され、型・トレイト識別に用いられる。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T002, T004, T005, T011]
- [ ] [AC-03] 複数候補に一致し、完全修飾パスの併記もなく、文脈から一意に決められない短名宣言は、評価が候補の完全修飾パスを列挙して失敗し、短名 identity へ fallback しない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T004, T011]
- [ ] [AC-04] 完全修飾パスを解決できない対象では、その対象を示す診断とともに評価が失敗し、暗黙の短名キーへの復帰は起きない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T004, T011]
- [ ] [AC-05] 同一 crate の異なる module に同名型を置く fixture で、trait_impls の trait_ref / for_type、inherent_impls の type_name、generic 引数、および in-crate TypeRef を検査すると、各参照が対応する rustdoc paths の完全修飾パスへ解決され、片方の型の impl または比較結果がもう片方の型に紐づかず、DanglingId も発生しない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T004, T005, T011]
- [ ] [AC-06] 同じ fixture の型シグナル出力と contract-map を検査すると、owner、node、edge が対応する完全修飾パスごとに別々に生成され、片方の対象の出力へ他方の同名型が結合されない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T006, T012]
- [ ] [AC-07] 同名型または同名トレイトを含む fixture で、指定された catalogue-lint の ReferencedRoleConstraint、FieldElementUniqueAcrossEntries、NoExternalReferenceInMethods の各規則を実行すると、各参照は対応する完全修飾パスの entry に対して個別に判定され、別の同名 entry があることだけを理由に対象を取り違えない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T007, T013]
- [ ] [AC-08] 同一 crate 内の同名 trait または型を含む fixture で、test-obligation derive の trait role index、task-contract と pre-review gate の CatalogueEntryKey、および catalog add / cite / import の書込み結果を検査すると、二つの完全修飾パスが別々の entry として保持され、lookup または attribution が片方を他方に置き換えない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T008, T009, T010, T014, T015, T016]
- [ ] [AC-09] functions key、cross-crate catalogue 表記、raw rustdoc baseline wire format、および D1 で対象外とした表示・検査面の既存の振る舞いは維持される。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T003, T010, T017]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 24  🟡 0  🔴 0

