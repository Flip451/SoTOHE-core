<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# Signal Report Command

## Goal

- [GO-01] 4 chain の Yellow / Red の発生箇所を 1 コマンドで横断列挙し、発生単位データが永続化されない chain も含めて、gate が block した原因を手動の JSON 読解なしに診断できるようにする。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1, knowledge/adr/2026-07-31-2134-signal-report-occurrence-source.md#D1]

## Scope

### In Scope
- [IN-01] `sotp signal report` を追加し、`adr_user`、`spec_adr`、`catalog_spec`、`impl_catalog` の 4 chain における Yellow / Red 信号の発生箇所を報告する。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T001, T005]
- [IN-02] 報告する各 signal occurrence に、entry id、参照文字列、判定理由、および対象ファイル位置を含める。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T001, T004]
- [IN-03] `--chain <id>` による chain 選択と、Yellow / Red の選択による結果の絞り込みを提供する。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T001, T003, T005]
- [IN-04] 発生単位 signal 成果物がある chain はその成果物を読み、存在しない chain ⓪・①は正規の入力成果物から既存の評価規則で発生単位データをメモリ上に導出する。report は導出した signal または集計値を永続化しない読み取り専用 view とする。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1, knowledge/adr/2026-07-31-2134-signal-report-occurrence-source.md#D1] [tasks: T001, T002]

### Out of Scope
- [OUT-01] `sotp track resolve` の blocker 表示へ signal occurrence の要約を統合することは対象外とする。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T005]
- [OUT-02] 通常の signal 評価規則、signal または集計値の永続化、ならびに gate 合否と strictness の判定規則を変更することは対象外とする。chain ⓪・①で report 表示用に既存の評価規則から発生単位データをメモリ上に導出することは、この除外に含まれない。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1, knowledge/adr/2026-07-31-2134-signal-report-occurrence-source.md#D1] [tasks: T002]

## Constraints
- [CN-01] report は診断用の読み取り専用 view であり、実行前後で既存の signal calculation artifacts を変更せず、chain ⓪・①で導出した発生単位データも永続化しない。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1, knowledge/adr/2026-07-31-2134-signal-report-occurrence-source.md#D1] [tasks: T001, T002, T006]
- [CN-02] 選択条件に一致する occurrence を、診断に必要な entry id、参照文字列、判定理由、対象ファイル位置とともに欠落なく報告する。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T001, T004]

## Acceptance Criteria
- [ ] [AC-01] `sotp signal report` を実行すると、発生単位 signal 成果物がある chain ではその成果物を用い、chain ⓪・①では正規の入力成果物からメモリ上で導出して、4 chain の Yellow / Red occurrence が報告される。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1, knowledge/adr/2026-07-31-2134-signal-report-occurrence-source.md#D1] [tasks: T001, T002, T006, T005]
- [ ] [AC-02] report の各 occurrence には entry id、参照文字列、判定理由、および対象ファイル位置が表示される。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T001, T004]
- [ ] [AC-03] `sotp signal report --chain <id>` は指定した chain の occurrence のみを報告し、Yellow / Red 選択を指定した実行は選択した色の occurrence のみを報告する。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T001, T003, T005]
- [ ] [AC-04] report の実行は既存の calculation artifacts を読み取り専用で扱い、chain ⓪・①で導出した発生単位データを含め、signal または集計値を persist しない。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1, knowledge/adr/2026-07-31-2134-signal-report-occurrence-source.md#D1] [tasks: T001, T002, T006]
- [ ] [AC-05] `sotp track resolve` の表示は、この report command によって新たな signal occurrence の要約を追加されず従来どおりである。 [adr: knowledge/adr/2026-07-29-0839-signal-report-command.md#D1] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0

