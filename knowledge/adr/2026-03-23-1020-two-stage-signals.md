---
adr_id: 2026-03-23-1020-two-stage-signals
decisions:
  - id: 2026-03-23-1020-two-stage-signals_grandfathered
    status: accepted
    grandfathered: true
---
# Two-stage signal architecture

## Status

Accepted

## Context

spec の品質とドメインモデルの品質を 1 つのゲートにまとめるか、独立した 2 ゲートにするか。

## Decision

2 段階の独立ゲート:
- Stage 1 (spec signals): source tag provenance → `spec.md` frontmatter
- Stage 2 (domain state signals): design confidence → `metadata.json domain_state_signals`

Sequential gate: Stage 1 must pass before Stage 2。
共有プリミティブ: `ConfidenceSignal`/`SignalCounts`。`SignalBasis` は Stage 1 only。`TrackMetadata` は Stage 2 only。

## Rejected Alternatives

- Single unified store in metadata.json: spec authority と track state が混在
- Single-stage evaluation: requirement confidence と design confidence の区別ができない

## Consequences

- Good: spec の品質とドメインモデルの品質を独立して追跡可能
- Good: Stage 1 だけで先行リリース可能
- Bad: Stage 1 の存在チェックが形骸化するリスク
- Bad: Stage 2 の信号基準が Phase 3 まで主観判定
- Bad: 両 Stage が green でも spec ↔ Domain States 間の矛盾は未検出（Phase 3 の 3-12 で対処予定）

## Reassess When

- Phase 3 で SPEC-03/SPEC-01 が入り、信号基準が客観化された時点
- 実運用で 2 ゲート管理が煩雑と判明した場合
