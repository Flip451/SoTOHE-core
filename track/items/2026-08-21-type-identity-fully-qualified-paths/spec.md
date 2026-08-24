<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 19, yellow: 0, red: 0 }
---

# 型・トレイト識別を完全修飾パスへ移行し、カタログ内参照と Chain ③ の検証責務を分離する

## Goal

- [GO-01] 型シグナル評価で型・トレイトを完全修飾パスで識別し、短名の衝突によって別の対象が同一視されないようにする。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1]
- [GO-02] カタログ内参照の解決と TypeRef 全体の実装適合を、それぞれの検査が保証できる範囲で fail-closed に検証し、両者の成功条件を混同しない。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1]

## Scope

### In Scope
- [IN-01] catalogue の型・トレイト宣言、trait_impls、inherent_impls、および in-crate TypeRef を完全修飾パスへ解決して識別に用いること。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T002, T004, T008, T011]
- [IN-02] 型・トレイト参照の解決、impl と generic 引数の比較、型シグナル出力の対応付け、contract-map の node / edge 解決、及び catalogue entry を扱う周辺評価を、同名併存と完全修飾パス識別に整合させること。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T004, T005, T006, T008, T012, T014]
- [IN-03] catalogue-lint が評価入力 catalogue の有効な型・トレイト宣言に対して、TypeRef に含まれるすべてのカタログ内参照を解決すること。検査を完了できない場合又は一意に解決できない場合は、位置を示して fail-closed とすること。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T008, T013]
- [IN-04] Chain ③ が実装から得た型情報に対して TypeRef 全体の適合を独立に fail-closed で検証し、catalogue-lint の成功をその検証の代替にしないこと。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T004, T005, T008, T011, T013, T015]

### Out of Scope
- [OS-01] functions の key、cross-crate 参照の catalogue 表記、catalog import --type の既存の完全一致照合、raw rustdoc baseline の wire format、及び TypeRef を解決しない表示・検査面の既存の振る舞いは変更しない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T008, T013, T014]
- [OS-02] 同名宣言を表す catalogue の具体的な宣言形式をこの behavioral contract で固定せず、重複側の改名や同名禁止を恒久的な解決策にはしない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T004, T005, T011]
- [OS-03] カタログ内参照の抽出・解決又は TypeRef 全体の照合に用いる具体的な parser、port、adapter、依存注入、層間配線の設計は固定しない。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T006, T008, T012, T013]

## Constraints
- [CO-01] 型・トレイト識別は rustdoc の paths から得る完全修飾パスを基準とし、短名へ暗黙に縮退してはならない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T002, T004, T005, T006, T008, T011, T012, T013, T014]
- [CO-02] catalogue-lint は TypeRef 全体の検査完了を確認できない限り成功してはならず、カタログ内参照か外部参照かを分類できない参照を外部参照と推測してはならない。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T004, T008, T011, T013]
- [CO-03] catalogue-lint と Chain ③ は異なる判定根拠に基づく独立した検査であり、一方の成功で他方の失敗または未検証を補ってはならない。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T004, T005, T008, T011, T013, T015]

## Acceptance Criteria
- [ ] [AC-01] 同一 crate 内の同名型および同名トレイトを一つの catalogue に同時に宣言でき、評価はそれぞれを異なる完全修飾パスの対象として扱う。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T002, T004, T011]
- [ ] [AC-02] 一意に解決できる短名の型・トレイト宣言および in-crate TypeRef は完全修飾パスへ解決され、型・トレイトの識別に用いられる。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T001, T002, T004, T005, T008, T011, T012]
- [ ] [AC-03] 複数候補に一致し、完全修飾パスの併記もなく文脈から一意に決められない短名宣言は、候補の完全修飾パスを列挙する診断とともに失敗し、短名 identity へ fallback しない。完全修飾パスを解決できない対象も対象を示す診断とともに失敗する。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T004, T005, T008, T011, T013, T015]
- [ ] [AC-04] catalogue-lint は TypeRef 全体で発見したすべてのカタログ内参照を、評価入力 catalogue の有効な型・トレイト宣言に対して一意に解決できた場合にのみ成功する。未解決または曖昧な参照は、該当箇所または候補の完全修飾パスを示して fail-closed とする。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T004, T005, T008, T011, T013]
- [ ] [AC-05] catalogue-lint は未対応または検査不能な構文、資源または深さの上限、及びカタログ内参照か外部参照かを分類できない箇所を位置とともに fail-closed とし、部分的な解決成功を成功扱いしない。generic parameter、lifetime、const 値、associated item のラベルはカタログ内参照として解決しない。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T001, T004, T005, T008, T011, T012, T013]
- [ ] [AC-06] Chain ③ の実装突合は実装から得た型情報に対して TypeRef 全体の適合を独立に fail-closed で検証する。catalogue-lint の成功は、外部 path、wrapper、型引数を含む TypeRef 全体の実装適合を検証済みとみなす根拠にならない。 [adr: knowledge/adr/2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary.md#D1] [tasks: T006, T008, T012, T015]
- [ ] [AC-07] 同名型または同名トレイトを含む入力で、型・トレイト参照、impl と generic 引数の比較、型シグナル出力および contract-map の対応付けは完全修飾パスごとに別々に扱われ、片方の対象の結果が他方へ結合されず DanglingId を生じさせない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1] [tasks: T006, T008, T012, T013, T014]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 19  🟡 0  🔴 0

