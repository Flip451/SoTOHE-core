---
adr_id: 2026-03-23-1010-three-level-signals
decisions:
  - id: 2026-03-23-1010-three-level-signals_grandfathered
    status: accepted
    grandfathered: true
---
# TSUMIKI-01/SPEC-05: 3-level signals with SignalBasis

## Status

Accepted

## Context

spec の品質を可視化する信号機のレベル数をどうするか。

## Decision

3 段階（Blue/Yellow/Red）を採用。各レベルはアクション可能な判断にマップ:
- Blue: proceed（進行可能）
- Yellow: verify（要確認）
- Red: block（ブロック）

`SignalBasis` enum で各信号の理由を内部的に捕捉するが、外部レベルは 3 段階のまま。`DonePending`/`DoneTraced` と同じパターン（internal nuance, unified external kind）。

## Rejected Alternatives

- 4+ levels (Green for CI-verified, split Red into unverified/contradicted): 実運用で区別が困難。判断基準が曖昧になる
- Single enum without basis separation: 理由情報が失われ、デバッグや改善が困難

## Consequences

- Good: シンプルな 3 値判定。アクションが明確
- Good: `SignalBasis` で内部的に理由を追跡可能
- Bad: CI 通過 vs 人間確認の区別が外部レベルに現れない（Phase 3 の SPEC-03 で対処予定）

## Reassess When

- Phase 3 で CI 証拠ベースの昇格/降格が入った時点で、4 段階の必要性を再評価
