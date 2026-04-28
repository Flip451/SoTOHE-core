---
adr_id: 2026-03-23-2120-two-stage-signal-architecture
decisions:
  - id: 2026-03-23-2120-two-stage-signal-architecture_grandfathered
    status: accepted
    grandfathered: true
---
# 2 段階信号機アーキテクチャ

## Status

Accepted

## Context

Phase 2 で spec の品質保証として信号機（🔵🟡🔴）を導入する。評価対象が 2 つある:

1. **spec 全体の品質**: 要件の出典（source tags）があるか
2. **Domain States の品質**: ドメインモデリングが十分か

これらを 1 つのゲートにまとめるか、独立した 2 ゲートにするかの判断。

## Decision

2 段階の独立ゲートとして実装する。

```
Stage 1 (2-1): spec.md [source: ...] tags → ConfidenceSignal → spec 信号機
  + Domain States セクション存在チェック（Stage 2 への橋渡し）

Stage 2 (2-2): spec.md ## Domain States → per-state signal → Domain States 信号機
  + metadata.json domain_state_signals に格納
```

共有プリミティブ: `ConfidenceSignal` + `SignalCounts` は Stage 1 で定義し Stage 2 も使う。

## Rejected Alternatives

- **1 ゲート統合**: spec とドメインモデリングは異なる成熟度で進むため、1 つの信号に混ぜると粒度が粗すぎる
- **4 段階以上の信号**: Codex planner + Claude 分析で一致して却下。3 段階で十分

## Consequences

- Good: spec の品質とドメインモデルの品質を独立して追跡可能
- Good: Stage 1 だけで先行リリース可能（垂直スライス）
- Bad: Stage 1 の存在チェックが形骸化するリスク（空テーブルで通過可能）
- Bad: Stage 2 の 🔵 基準が Phase 3 まで主観判定になる
- Bad: 両 Stage が green でも spec ↔ Domain States 間の矛盾は検出されない（Phase 3 の 3-12 で対処予定）

## Reassess When

- Phase 3 で SPEC-03（CI 証拠で昇格）、SPEC-01（テスト失敗で降格）が入り、信号基準が客観化された時点で 2 段階の妥当性を再評価
- 実運用で 2 ゲート管理が煩雑と判明した場合は統合を検討
