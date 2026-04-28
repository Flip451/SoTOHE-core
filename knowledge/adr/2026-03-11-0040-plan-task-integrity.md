---
adr_id: 2026-03-11-0040-plan-task-integrity
decisions:
  - id: 2026-03-11-0040-plan-task-integrity_grandfathered
    status: accepted
    grandfathered: true
---
# Plan-task 参照整合性を構築時に検証

## Status

Accepted

## Context

plan.md のセクションが参照する task_ids が metadata.json のタスクと一致するか、いつ検証するか。

## Decision

Plan-task の参照整合性を構築時（PlanView 生成時）に検証する。不整合があればエラーを返す。

## Rejected Alternatives

- Runtime validation on access: 不整合がアクセス時まで検出されず、中間状態で破綻する可能性

## Consequences

- Good: 無効な plan を早期に検出。Python バリデーションと一致
- Bad: plan 構築がやや厳密（task_id のタイポで即エラー）

## Reassess When

- plan と tasks の関係が疎結合になり、参照整合性が不要になった場合
