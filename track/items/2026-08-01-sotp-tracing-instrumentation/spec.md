<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 17, yellow: 0, red: 0 }
---

# Sotp Tracing Instrumentation

## Goal

- [GO-01] track ブランチ上で、track が解決できる純表示系ではない `sotp` コマンドの完了観測を共通 entry point から既存 telemetry に記録し、利用頻度、所要時間、および失敗率を確認できるようにする。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1]

## Scope

### In Scope
- [IN-01] track ブランチ上で完了した、track が解決できる純表示系ではない `sotp` コマンドを共通の entry point で計装し、完了時にコマンド identity、所要時間、および exit 結果を観測する。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T010]
- [IN-02] track ブランチ上で完了した、track が解決できる純表示系ではないコマンドの観測イベントを、通常は既存の track-local `track/items/<track-id>/logs/telemetry.jsonl` へ追記する。`track archive` の成功時だけは、既存ログが track とともに archive へ移動するため `track/archive/<track-id>/logs/telemetry.jsonl` へ追記し、active 側のログを再作成しない。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1, knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T001, T010]
- [IN-03] 既存の `sotp telemetry` 集計を、追記されたコマンド観測イベントを含めて利用頻度、所要時間、および失敗率を分析できるように拡張する。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1, knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T003, T006]
- [IN-04] telemetry の track 帰属は現在の branch に結び付け、track ブランチ上の実行だけを当該 track の時系列観測に含める。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T010]

### Out of Scope
- [OUT-01] 計装記録または telemetry 分析結果を外部サービスへ送信することは対象外とする。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T010]
- [OUT-02] 新たな `telemetry.jsonl` 以外の command-trace sink またはローテーション方針を導入しない。既存 report reader の互換読み取りは変更対象外とする。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T001, T010]
- [OUT-03] track ブランチ以外で実行されたコマンドを telemetry に記録することは対象外とする。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T010]
- [OUT-04] コマンド内部の詳細な span を追加することは対象外とし、必要性が判明した場合に再評価する。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T010]

## Constraints
- [CN-01] 計装記録はローカルで完結させ、外部送信を行わない。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1] [tasks: T001, T003, T006, T010]
- [CN-02] 診断用 telemetry の追記失敗はコマンドの完了結果を変更せず、SoT 成果物を損なわない fire-and-forget の挙動を維持する。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D1] [tasks: T010]
- [CN-03] `sotp telemetry` は、形式不正な記録および未知の `schema_version` を持つ記録を fail-open で skip し、skip した記録件数を報告する。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T003, T006]

## Acceptance Criteria
- [ ] [AC-01] track ブランチ上で、track が解決できる純表示系ではない任意の `sotp` コマンドが完了すると、同じ track の `telemetry.jsonl` にコマンド identity、所要時間、および exit 結果を含むイベントが追記される。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1, knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T001, T010]
- [ ] [AC-02] `sotp telemetry` は既存の telemetry 記録を集計し、コマンド横断の実行頻度、所要時間、および失敗率を確認できる。 [adr: knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md#D1, knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T003, T006]
- [ ] [AC-03] telemetry の追記に失敗しても、実行済みコマンドの exit 結果は telemetry 失敗によって失敗へ変わらない。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D1] [tasks: T010]
- [ ] [AC-04] track ブランチ以外でコマンドを実行しても、track の telemetry 記録は生成または更新されない。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T010]
- [ ] [AC-05] 実行後に作成または更新されるコマンド観測の記録先は既存の `telemetry.jsonl` のみであり、別の command-trace file やローテーション済み世代は作成されない。 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T001, T010]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 17  🟡 0  🔴 0

