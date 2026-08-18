---
adr_id: "2026-07-29-0839-sotp-tracing-instrumentation"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29"
    status: proposed
---
# 全 sotp コマンドに tracing 計装を入れ横断分析可能にする

## Context

`sotp telemetry` は track 単位の workflow telemetry 集計を提供するが、sotp コマンド全体の実行頻度・所要時間・失敗率を横断的に観測する手段がない。CI 並列化（別 ADR）のボトルネック特定や、ゲート運用コストの定量化には全コマンドの計装が前提になる。

## Decision

### D1: 全 sotp コマンドに tracing 計装を入れる

tracing crate による span / event 計装を全コマンド共通の entry point に入れ、実行コマンド・所要時間・exit 結果をローカル jsonl に記録する。既存の `sotp telemetry` 集計をこの記録の分析に拡張する。記録はローカル完結とし、外部送信は行わない。

## Consequences

- 良: ボトルネック・失敗率・利用頻度がデータで判断できる。CI 並列化や UX 改善の優先度付けの根拠になる。
- 負: 記録ファイルの肥大化管理（ローテーション）が必要。

## Reassess When

- 記録粒度が不足（コマンド内部の span が必要）と判明したとき。
