<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 45, yellow: 0, red: 0 }
---

# 規約を機構と突き合わせて改訂する

## Goal

- [GO-01] テンプレートが出荷する規約とレビュー指示を、各規範の強制機構、必要駆動の抽象、型安全な境界、および宣言済み環境前提に整合させ、consumer 側で同じ設計逸脱を再生産しない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D2, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D4, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7, knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8]

## Scope

### In Scope
- [IN-01] knowledge/conventions/ の現存する規範的要求それぞれに、機械 lint、宣言突合、review 観点、または強制なしのいずれかの強制先を注記するメタ規則を追加し、有限な対象集合の完全性は harness-policy review で判断する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1] [tasks: T001, T002, T003, T004, T006, T007, T008]
- [IN-02] type-designer-kind-selection.md と関連規約を改訂し、層内で任意に追加する trait と実装の組は複数実装またはテスト差し替えが必要な場合だけ導入する。inbound port trait と secondary port は構造規則が要求する port としてこの必要性テストの対象外とし、支配するアーキテクチャ規則に従う。 [adr: knowledge/adr/2026-08-25-2239-required-ports-exempt-from-necessity-test.md#D1] [tasks: T002]
- [IN-06] type-designer-kind-selection.md と関連規約を改訂し、driver は消費する単能 port を直接受け取り、command と query を混載する facade port を新設しない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3] [tasks: T006]
- [IN-07] type-designer-kind-selection.md と関連規約を改訂し、command usecase の入力境界は検証済み Command を 1 個、query usecase の入力境界は検証済み Query を 1 個だけ受け取り、cli は一度だけ対応する型へパースして入力境界を呼び出す。 [adr: knowledge/adr/2026-08-25-1021-validated-usecase-input-boundaries.md#D1] [tasks: T007]
- [IN-08] type-designer-kind-selection.md と関連規約を改訂し、role × layer 規則は固有 crate 名ではなく層の性質で表す。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5] [tasks: T008]
- [IN-03] testing.md を、test-obligation を品質保証の正とし、層ごとのテスト責務、fake 優先、codec・parser・evaluator の property-based testing、および自ソース部分文字列 assert 禁止を定める構成へ全面改稿する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6] [tasks: T003]
- [IN-04] 対応プラットフォーム、入力エンコーディング方針、資源上限、並行モデルを consumer が宣言できる環境前提の置き場を、枠と記入指針だけを含む形で knowledge/conventions/ に新設する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7] [tasks: T004]
- [IN-05] .harness/custom/review-prompts/ の review-scope.json が宣言する全 code scope に、diff が接する外部境界と未宣言の前提への依存を確認するドメイン非依存のメタ問いを追加し、spec review prompt には環境前提宣言の必要性を問う対応形を追加する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8] [tasks: T005]
- [IN-09] D1 のメタ規則を実装する consumer 所有の overlay/knowledge/conventions/enforce-by-mechanism.md 初期値を、改訂した convention 文書と同期する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T009]
- [IN-10] D2 の必要駆動の抽象規則を実装する consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を、改訂した convention 文書と同期する。 [adr: knowledge/adr/2026-08-25-2239-required-ports-exempt-from-necessity-test.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T011]
- [IN-11] D3 の port 注入粒度および command と query を混載する facade port を新設しない規則を実装する consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を、改訂した convention 文書と同期する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T012]
- [IN-12] command usecase が検証済み Command を 1 個、query usecase が検証済み Query を 1 個だけ受け取り、cli が usecase 所有の boundary 型へ一度だけパースして対応する入力境界を呼び出す規則を実装する consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を、改訂した convention 文書と同期する。 [adr: knowledge/adr/2026-08-25-1021-validated-usecase-input-boundaries.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T013]
- [IN-13] D5 の層の性質で表す role × layer 規則を実装する consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を、改訂した convention 文書と同期する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T014]
- [IN-14] D6 の testing.md 全面改稿を実装する consumer 所有の overlay/knowledge/conventions/testing.md 初期値を、改訂した convention 文書と同期する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T017]
- [IN-15] D7 の環境前提宣言の置き場として、consumer が記入する枠と指針だけを含む consumer 所有の overlay/knowledge/conventions/environment-assumptions.md 初期値を追加する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T018]
- [IN-16] D1 の強制機構注記を反映する consumer 所有の overlay/knowledge/conventions/coding-principles.md、overlay/knowledge/conventions/security.md および overlay/knowledge/conventions/README.md 初期値を、各 workspace 側 convention 文書と同期する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T010, T015]
- [IN-17] D3 の port 注入粒度および command と query を混載する facade port を新設しない規則を反映する consumer 所有の overlay/knowledge/conventions/coding-principles.md 初期値を、workspace 側 convention 文書と同期する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T015]
- [IN-18] 必要性テストから structure-required port を除外する D1 の refine を反映する consumer 所有の overlay/knowledge/conventions/prefer-type-safe-abstractions.md 初期値を、workspace 側 convention 文書と同期する。 [adr: knowledge/adr/2026-08-25-2239-required-ports-exempt-from-necessity-test.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T016]

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
- [ ] [AC-02] type-designer-kind-selection.md と関連規約は、層内で任意に追加する trait と実装の組を、複数実装が現存するかテスト境界として差し替えが必要な場合だけ導入し、共有所有だけを理由に導入しないことを明確にする。inbound port trait と secondary port は構造規則が要求する port としてこの必要性テストの対象外とし、支配するアーキテクチャ規則に従う。 [adr: knowledge/adr/2026-08-25-2239-required-ports-exempt-from-necessity-test.md#D1] [tasks: T002]
- [ ] [AC-06] type-designer-kind-selection.md と関連規約は、driver が消費する複数の単能 port を直接受け取り、command と query を混載する facade port を新設しないことを明確にする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3] [tasks: T006]
- [ ] [AC-07] type-designer-kind-selection.md と関連規約は、command usecase が検証済み Command を 1 個、query usecase が検証済み Query を 1 個だけ受け取り、cli が一度だけ対応する型へパースして入力境界を呼び出すことを明確にする。 [adr: knowledge/adr/2026-08-25-1021-validated-usecase-input-boundaries.md#D1] [tasks: T007]
- [ ] [AC-08] type-designer-kind-selection.md と関連規約は、R1 の層規則を層の性質で解決することを明確にする。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5] [tasks: T008]
- [ ] [AC-03] testing.md は新規コードの line coverage 80% 目標を保持せず、test-obligation 機構を品質保証の正として、テストピラミッド、fake 優先、限定的な mock、property-based testing、および自ソース部分文字列 assert の禁止を明示する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6] [tasks: T003]
- [ ] [AC-04] 新しい環境前提宣言の置き場は、対応プラットフォーム、入力エンコーディング方針、資源上限、並行モデルを consumer が記入できる枠と指針を含み、特定ドメインの前提を既定値として含まない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7] [tasks: T004]
- [ ] [AC-05] review-scope.json が宣言する全 code scope の review prompt と spec review prompt は、D8 の対応する境界メタ問いを含む。問いは OS、プロセス、エンコーディング、並行、資源上限、時刻、別バージョンの自成果物を分類として扱い、未宣言の前提への依存を報告させる一方、doc scope とドメイン固有チェックリストには注入しない。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D8] [tasks: T005]
- [ ] [AC-09] fresh template export は、D1 のメタ規則を反映した consumer 所有の overlay/knowledge/conventions/enforce-by-mechanism.md 初期値を出荷する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T009, T019]
- [ ] [AC-10] fresh template export は、D2 の必要駆動の抽象規則を反映した consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を出荷する。 [adr: knowledge/adr/2026-08-25-2239-required-ports-exempt-from-necessity-test.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T011, T019]
- [ ] [AC-11] fresh template export は、D3 の port 注入粒度および command と query を混載する facade port を新設しない規則を反映した consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を出荷する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T012, T019]
- [ ] [AC-12] fresh template export は、command usecase が検証済み Command を 1 個、query usecase が検証済み Query を 1 個だけ受け取り、cli が usecase 所有の boundary 型へ一度だけパースして対応する入力境界を呼び出す規則を反映した consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を出荷する。 [adr: knowledge/adr/2026-08-25-1021-validated-usecase-input-boundaries.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T013, T019]
- [ ] [AC-13] fresh template export は、D5 の層の性質で表す role × layer 規則を反映した consumer 所有の overlay/knowledge/conventions/type-designer-kind-selection.md 初期値を出荷する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D5, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T014, T019]
- [ ] [AC-14] fresh template export は、D6 の test-obligation を品質保証の正とする改訂を反映した consumer 所有の overlay/knowledge/conventions/testing.md 初期値を出荷する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D6, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T017, T019]
- [ ] [AC-15] fresh template export は、consumer が記入する枠と指針だけを含む consumer 所有の overlay/knowledge/conventions/environment-assumptions.md 初期値を出荷する。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T018, T019]
- [ ] [AC-16] fresh template export の出力に対して bin/sotp template check-convention-shipping が成功し、exported tree に overlay が供給しない convention が含まれないことを検証できる。改訂または追加した consumer 所有の convention 初期値が export に存在することは、AC-17 の export 比較で別途確認できる。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D7, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T019]
- [ ] [AC-17] fresh template export の consumer 所有 overlay/knowledge/conventions/ 初期値は、track が改訂または追加した README.md、coding-principles.md、prefer-type-safe-abstractions.md、security.md、enforce-by-mechanism.md、type-designer-kind-selection.md、testing.md、environment-assumptions.md の workspace 側の内容と同期していることを内容比較で確認できる。environment-assumptions.md は CN-02 / OUT-03 に従い、宣言枠と記入指針だけを含む。 [adr: knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T019]
- [ ] [AC-18] fresh template export の consumer 所有 overlay/knowledge/conventions/coding-principles.md、security.md および README.md は、各 workspace 側文書と同じ規範的要求および強制機構注記を持ち、consumer 向けの framing だけが異なることを内容比較で確認できる。 [adr: knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T010, T015, T019]
- [ ] [AC-19] fresh template export の consumer 所有 overlay/knowledge/conventions/coding-principles.md は、workspace 側文書と同じ port 注入粒度および command と query を混載する facade port を新設しない規則を持つことを内容比較で確認できる。 [adr: knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md#D3, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T015, T019]
- [ ] [AC-20] fresh template export の consumer 所有 overlay/knowledge/conventions/prefer-type-safe-abstractions.md は、workspace 側文書と同じ structure-required port を必要性テストの対象外とする規則および強制機構注記を持つことを内容比較で確認できる。 [adr: knowledge/adr/2026-08-25-2239-required-ports-exempt-from-necessity-test.md#D1, knowledge/adr/2026-08-26-0000-consumer-shipped-convention-initial-values.md#D1] [tasks: T016, T019]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 45  🟡 0  🔴 0

