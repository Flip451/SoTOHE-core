<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 23, yellow: 0, red: 0 }
---

# 規約を機構と突き合わせて改訂する

## Goal

- [GO-01] テンプレートが出荷する規約とレビュー指示を、各規範の強制機構、必要駆動の抽象、型安全な境界、および宣言済み環境前提に整合させ、consumer 側で同じ設計逸脱を再生産しない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D2, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D4, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8]

## Scope

### In Scope
- [IN-01] knowledge/conventions/ の現存する規範的要求それぞれに、機械 lint、宣言突合、review 観点、または強制なしのいずれかの強制先を注記するメタ規則を追加し、有限な対象集合の完全性は harness-policy review で判断する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1] [tasks: T001, T002, T003, T004, T006, T007, T008]
- [IN-02] type-designer-kind-selection.md と関連規約を改訂し、抽象は複数実装またはテスト差し替えが必要な場合だけ導入する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D2] [tasks: T002]
- [IN-06] type-designer-kind-selection.md と関連規約を改訂し、driver は消費する単能 port を直接受け取り、command と query を混載する facade port を新設しない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3] [tasks: T006]
- [IN-07] type-designer-kind-selection.md と関連規約を改訂し、usecase 境界は検証済み Command 型へ一本化し、cli は一度だけ Command へパースして入力境界を呼び出す。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D4] [tasks: T007]
- [IN-08] type-designer-kind-selection.md と関連規約を改訂し、role × layer 規則は固有 crate 名ではなく層の性質で表す。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5] [tasks: T008]
- [IN-03] testing.md を、test-obligation を品質保証の正とし、層ごとのテスト責務、fake 優先、codec・parser・evaluator の property-based testing、および自ソース部分文字列 assert 禁止を定める構成へ全面改稿する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6] [tasks: T003]
- [IN-04] 対応プラットフォーム、入力エンコーディング方針、資源上限、並行モデルを consumer が宣言できる環境前提の置き場を、枠と記入指針だけを含む形で knowledge/conventions/ に新設する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7] [tasks: T004]
- [IN-05] .harness/custom/review-prompts/ の review-scope.json が宣言する全 code scope に、diff が接する外部境界と未宣言の前提への依存を確認するドメイン非依存のメタ問いを追加し、spec review prompt には環境前提宣言の必要性を問う対応形を追加する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8] [tasks: T005]

### Out of Scope
- [OUT-01] 既存コード、既存の抽象ペア、または既存の境界実装を、改訂後の規約に合わせて遡及修正することは対象外とする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D2] [tasks: T002, T006, T007, T008]
- [OUT-02] 新しい lint、catalogue preset、または規約注記を機械抽出する実装を導入することは対象外とし、必要なら後続 ADR で扱う。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1] [tasks: T001]
- [OUT-03] 特定 OS、特定プロトコル、その他のドメイン固有の境界チェック項目をテンプレートの review prompt や環境前提宣言の既定値として追加することは対象外とする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8] [tasks: T004, T005]

## Constraints
- [CN-01] D1 の注記対象は現時点の knowledge/conventions/ にある有限な規範的要求であり、注記漏れの判定を機械的な全規則抽出の完了条件に置き換えてはならない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1] [tasks: T001]
- [CN-02] 環境前提宣言は consumer 所有とし、テンプレートは宣言枠と記入指針だけを提供して、プロジェクト固有の前提を既定として決めてはならない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7] [tasks: T004]
- [CN-03] レビュー問いの注入先は review-scope.json が宣言する code scope とその briefing_file、および spec review prompt に限り、doc scope を含めず、外部境界の列挙で到達性を確定できない間接境界も未宣言の前提への依存として扱う。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8] [tasks: T005]

## Acceptance Criteria
- [ ] [AC-01] enforce-by-mechanism.md に D1 のメタ規則があり、knowledge/conventions/ の現存する規範的要求は機械 lint、宣言突合、review 観点、または強制なしとして注記され、harness-policy review でその完全性を評価できる。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1] [tasks: T001, T002, T003, T004, T006, T007, T008]
- [ ] [AC-02] type-designer-kind-selection.md と関連規約は、複数実装が現存するかテスト境界として差し替えが必要な場合だけ trait を導入し、共有所有だけを理由に trait を導入しないことを明確にする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D2] [tasks: T002]
- [ ] [AC-06] type-designer-kind-selection.md と関連規約は、driver が消費する複数の単能 port を直接受け取り、command と query を混載する facade port を新設しないことを明確にする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3] [tasks: T006]
- [ ] [AC-07] type-designer-kind-selection.md と関連規約は、cli が一度だけ Command へパースして usecase 入力境界を呼び出すことを明確にする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D4] [tasks: T007]
- [ ] [AC-08] type-designer-kind-selection.md と関連規約は、R1 の層規則を層の性質で解決することを明確にする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5] [tasks: T008]
- [ ] [AC-03] testing.md は新規コードの line coverage 80% 目標を保持せず、test-obligation 機構を品質保証の正として、テストピラミッド、fake 優先、限定的な mock、property-based testing、および自ソース部分文字列 assert の禁止を明示する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6] [tasks: T003]
- [ ] [AC-04] 新しい環境前提宣言の置き場は、対応プラットフォーム、入力エンコーディング方針、資源上限、並行モデルを consumer が記入できる枠と指針を含み、特定ドメインの前提を既定値として含まない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7] [tasks: T004]
- [ ] [AC-05] review-scope.json が宣言する全 code scope の review prompt と spec review prompt は、D8 の対応する境界メタ問いを含む。問いは OS、プロセス、エンコーディング、並行、資源上限、時刻、別バージョンの自成果物を分類として扱い、未宣言の前提への依存を報告させる一方、doc scope とドメイン固有チェックリストには注入しない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 23  🟡 0  🔴 0

