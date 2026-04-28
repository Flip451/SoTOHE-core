---
adr_id: 2026-03-11-0010-done-owns-commit-hash
decisions:
  - id: 2026-03-11-0010-done-owns-commit-hash_grandfathered
    status: accepted
    grandfathered: true
---
# TaskStatus::Done が Option<CommitHash> を所有する

## Status

Accepted

## Context

タスク完了時のコミットハッシュをどう保持するか。TrackTask のフィールドとして持つか、Done 状態に紐付けるか。

## Decision

`TaskStatus::Done` が `Option<CommitHash>` を所有する。コミットハッシュデータを done 状態に型レベルで束縛する。

## Rejected Alternatives

- Separate commit_hash field on TrackTask: done 以外の状態でも commit_hash が設定可能になり、不正状態を表現できてしまう

## Consequences

- Good: 不正状態（todo なのに commit_hash あり）が型レベルで排除
- Good: DMMF の "Make Illegal States Unrepresentable" 原則に適合
- Bad: commit_hash へのアクセスに match が必要

## Reassess When

- DonePending/DoneTraced 分割（WF-40 で実施済み）により構造が変わった場合
