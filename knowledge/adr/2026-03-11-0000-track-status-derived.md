# TrackStatus を tasks から導出し、保存しない

## Status

Accepted

## Context

トラックの状態（planned, in_progress, done 等）をどこに持つか。metadata.json に直接保存するか、タスク群の状態から導出するか。

## Decision

TrackStatus はタスク群の状態から導出する。metadata.json には保存しない。

## Rejected Alternatives

- Stored status with manual sync: 状態の不整合（タスクは全完了だが status が in_progress のまま等）が発生するリスク

## Consequences

- Good: status desync を構造的に排除
- Good: Python リファレンス実装と一致
- Bad: 導出ロジックの変更が全表示箇所に影響

## Reassess When

- TrackStatus に tasks から導出できない状態（例: 外部承認待ち）が必要になった場合
