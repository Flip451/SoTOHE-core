<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# ゲートの標準出力をサマリ契約にする

## Goal

- [GO-01] 対象となるゲート・検証タスクの標準出力を、判定・フルログの保存先・失敗時だけの診断抜粋からなる簡潔なサマリ契約に統一する。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1]
- [GO-02] 出力表示を簡潔化しても、既存の exit code と状態検査コマンドによる機械判定を維持する。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2]

## Scope

### In Scope
- [IN-01] Makefile.toml と bin/sotp の既存定義でテスト実行・義務評価・コミット前の集約ゲートとして扱われるタスクに、共通の stdout サマリ契約を適用する。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T002, T003]
- [IN-02] 各対象タスクはフル実行ログを tmp/gate/ 配下に保存し、stdout でそのログファイルへのパスを示す。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [IN-03] 対象タスクの失敗時には、失敗した項目と短い理由だけを stdout の診断抜粋として表示する。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]

### Out of Scope
- [OUT-01] 対象タスクの所属を新たに定義または変更することは対象外とし、その所属は既存の Makefile.toml と bin/sotp の定義に委ねる。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T002, T003]
- [OUT-02] stdout の文面をパースして合否または状態を判定する新しい機械可読経路を導入することは対象外とする。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2] [tasks: T001, T002, T003]

## Constraints
- [CN-01] 対象タスクの stdout は PASS または FAIL の判定、フルログファイルのパス、ならびに失敗時だけの失敗項目と短い理由に限らなければならない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [CN-02] 対象タスクの成功時には、個別 PASS 行および内部レコードの Debug 表現を stdout に出力してはならない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [CN-03] 機械による合否は既存の exit code で、状態照会は既存の check 系コマンドで引き続き判定しなければならない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] 対象タスクが成功すると、stdout は PASS の判定と tmp/gate/ 配下のフルログファイルのパスを示し、個別 PASS 行や内部レコードの Debug 表現を含まない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-02] 対象タスクが失敗すると、stdout は FAIL の判定、tmp/gate/ 配下のフルログファイルのパス、失敗した項目および各項目の短い理由を示す。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-03] 対象タスクの詳細な診断情報は stdout ではなく tmp/gate/ 配下のフルログとして参照できる。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-04] 出力契約の変更後も、対象タスクの成功・失敗は従来どおり exit code で判断でき、既存の check 系コマンドで状態を照会できる。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2] [tasks: T001, T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0

