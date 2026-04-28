---
adr_id: 2026-03-11-0030-status-override-auto-clear
decisions:
  - id: 2026-03-11-0030-status-override-auto-clear_grandfathered
    status: accepted
    grandfathered: true
---
# StatusOverride の自動クリア

## Status

Accepted

## Context

トラック全体のブロック/キャンセルを表す StatusOverride を、タスクが全て解決された時にどう扱うか。

## Decision

全タスクが resolved（done or skipped）になったら StatusOverride を自動クリアする。

## Rejected Alternatives

- Manual override management: stale な override がトラック完了後も残り続けるリスク

## Consequences

- Good: 完了済みトラックに stale override が残らない
- Bad: 意図的に override を残したいケース（例: 完了後もブロック表示したい）に対応できない

## Reassess When

- 完了後も override を維持したいユースケースが出現した場合
