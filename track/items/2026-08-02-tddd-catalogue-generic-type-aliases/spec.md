<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# TDDD カタログにジェネリクスパラメータ付き型エイリアスを宣言可能にする

## Goal

- [GO-01] TDDD カタログが Rust 実装で使用するジェネリクスパラメータ付き型エイリアスをそのまま契約として表現し、カタログ設計を非ジェネリクスな代替表現へ歪めずに実装との照合を可能にする。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1]

## Scope

### In Scope
- [IN-01] カタログスキーマで alias の型契約にジェネリクスパラメータ宣言を任意で記録できるようにし、ジェネリクスを宣言しない既存 alias の読み取りと評価を維持する。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T001, T004]
- [IN-02] catalogue lint がジェネリクスパラメータ付き alias 宣言を受理し、不正な宣言を明示的な検証エラーとして扱えるようにする。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T002]
- [IN-03] catalogue-to-implementation 照合が、カタログで宣言したジェネリクスパラメータ付き alias と実装側の alias を照合し、パラメータ表記の不一致を mismatch として報告できるようにする。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T003]

### Out of Scope
- [OUT-01] alias 以外のカタログ型種別に対するジェネリクス宣言の表現拡張は本 track の対象外とする。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T001, T003]
- [OUT-02] ジェネリクスパラメータ表記の意味解析や、字句照合を構文解析ベースの照合へ置き換えることは本 track の対象外とする。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T003]

## Constraints
- [CN-01] ジェネリクスパラメータ宣言は追加的な契約とし、未宣言の既存 alias entry の評価結果を変えない。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T001, T002, T004]
- [CN-02] 照合は既存の字句照合の境界を維持し、パラメータ表記はカタログ側の宣言表記を正規形として実装側がそれに従う。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T003]
- [CN-03] ジェネリクスパラメータを含む alias 契約では、宣言の欠落は非ジェネリクス alias として有効、宣言がある場合は順序付きの型パラメータ列として扱う。schema は宣言列・パラメータ名・bound 参照を復号できない入力を拒否し、lint は空の名前・重複した名前・空の bound を検証エラーとして報告する。照合は有効な宣言どうしだけを字句的に比較し、順序、名前、または bound 表記がカタログの正規表記と異なれば mismatch として報告する。いずれの境界も入力を暗黙に補正せず、bound の意味解析は行わない。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T001, T002, T003, T004]

## Acceptance Criteria
- [ ] [AC-01] ジェネリクスパラメータを持つ alias をカタログで宣言して読み書きでき、パラメータを持たない既存 alias も従来どおり読み書き・評価できる。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T001, T002, T004]
- [ ] [AC-02] catalogue lint は、alias entry のジェネリクス宣言が順序付きの型パラメータ列であり、各パラメータ名が空でない plain な非キーワード Rust 識別子（raw identifier 表記を含まない）、重複せず、各 bound が空でない型・trait 参照である場合に通過させる。これらを満たさない宣言は検証エラーとして報告する。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T002]
- [ ] [AC-03] カタログと実装が同じジェネリクスパラメータ表記の alias を宣言すると catalogue-to-implementation 照合が一致として評価する。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T003]
- [ ] [AC-04] 実装側の alias がカタログのジェネリクスパラメータ正規表記と異なる場合、catalogue-to-implementation 照合は mismatch を報告する。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T003]
- [ ] [AC-05] 既存の generic type alias を含む実装に対して、拡張前の照合結果を変えないことを確認する回帰検証がある。 [adr: knowledge/adr/2026-07-29-0839-catalogue-generic-type-alias.md#D1] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0
