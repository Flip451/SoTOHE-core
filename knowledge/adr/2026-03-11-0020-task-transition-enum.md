---
adr_id: 2026-03-11-0020-task-transition-enum
decisions:
  - id: 2026-03-11-0020-task-transition-enum_grandfathered
    status: accepted
    grandfathered: true
---
# TaskTransition を明示的 enum コマンドに

## Status

Accepted

## Context

タスクの状態遷移 API をどう設計するか。文字列ベース（"start", "complete"）か、型安全な enum か。

## Decision

`TaskTransition` を明示的な enum コマンドとして定義する。

## Rejected Alternatives

- String-based transitions (Python 実装): exhaustive match ができず、無効な遷移文字列がランタイムエラーに

## Consequences

- Good: 型安全な遷移 API。exhaustive match でカバレッジ保証
- Good: 新しい遷移を追加すると全 match 箇所でコンパイルエラー（見落とし防止）
- Bad: 遷移追加のたびに enum variant 追加が必要

## Reassess When

- 遷移の種類が大幅に増え、enum が肥大化した場合
