<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 15, yellow: 0, red: 0 }
---

# Sotp Tracing Instrumentation

## Goal

- [GO-01] 全 `sotp` コマンドの実行頻度、所要時間、および失敗率を横断的に観測できるようにし、CI 並列化、ゲート運用コスト、UX 改善の優先度をデータに基づいて判断できるようにする。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1]

## Scope

### In Scope
- [IN-01] すべての `sotp` コマンドを共通の entry point で tracing 計装し、コマンド横断で一貫した実行観測を行う。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T002, T010]
- [IN-02] 各コマンド実行について、コマンド identity、所要時間、および exit 結果をローカル JSONL 記録へ残す。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T002, T005, T007, T008, T010]
- [IN-03] 既存の `sotp telemetry` を、このローカル記録からコマンド横断の利用頻度、所要時間、および失敗率を分析できるように拡張する。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T003, T006, T009]
- [IN-04] 記録ファイルの増大を運用可能に保つため、ローテーションを扱う。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T004, T005, T008]

### Out of Scope
- [OUT-01] tracing 記録または telemetry 分析結果を外部サービスへ送信することは対象外とする。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T005, T006, T008]
- [OUT-02] コマンド内部の詳細な span を追加することは対象外とし、必要性が判明した場合に再評価する。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T010]

## Constraints
- [CN-01] 計装記録はローカルで完結させ、外部送信を行わない。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T002, T003, T005, T006, T008, T009, T010]
- [CN-02] ローカル JSONL の active 記録ファイルには正の最大バイト数と正の保持ファイル数を設定し、次の記録の追記で最大バイト数を超える前に active ファイルをローテーションする。ローテーション済みファイルが保持数を超える場合は最も古いものを削除する。 [adr: knowledge/adr/2026-08-01-0902-sotp-tracing-rotation-policy.md#D1] [tasks: T004, T005, T008]
- [CN-03] コマンド trace の記録に失敗した場合、その失敗で panic せず、また黙って握りつぶさず、typed `CommandTraceWriteError` として `Result` を通じて caller へ伝搬する。 [adr: knowledge/adr/2026-08-01-2356-sotp-tracing-write-failure-policy.md#D1] [conv: knowledge/conventions/coding-principles.md#Error Handling: Result and ? Operator, knowledge/conventions/coding-principles.md#No Panics in Library Code] [tasks: T002, T005, T008, T010]
- [CN-04] `sotp telemetry` は、形式不正な記録および未知の `schema_version` を持つ記録を fail-open で skip し、skip した記録件数を報告する。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T003, T006]

## Acceptance Criteria
- [ ] [AC-01] 任意の `sotp` コマンドを実行すると、そのコマンド identity、所要時間、および exit 結果を含むローカル JSONL 記録が生成される。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T002, T005, T007, T008, T010]
- [ ] [AC-02] `sotp telemetry` はローカル記録を分析し、コマンド横断の実行頻度、所要時間、および失敗率を確認できる。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T003, T006, T009]
- [ ] [AC-03] 計装と telemetry 分析の実行は、記録または分析結果を外部へ送信しない。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T002, T003, T005, T006, T007, T008, T009, T010]
- [ ] [AC-04] 正の最大バイト数と正の保持ファイル数を設定し、各記録が最大バイト数以下で累積サイズがその上限を超える複数のコマンド記録を生成すると、上限を超える追記の前に active JSONL ファイルがローテーションされる。ローテーション後は active ファイルが上限を超えず、保持されるローテーション済みファイル数は設定した保持数以下であることを確認できる。 [adr: knowledge/adr/2026-08-01-0902-sotp-tracing-rotation-policy.md#D1] [tasks: T005, T008]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 15  🟡 0  🔴 0

