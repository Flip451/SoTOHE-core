---
adr_id: 2026-05-26-1123-remove-plan-only-activate-lane
decisions:
  - id: D1
    user_decision_ref: "chat_segment:remove-plan-only-activate:2026-05-26"
    status: proposed
---
# plan-only / activate ワークフローレーンの削除

## Context

`/track:plan-only` と `/track:activate` による 2 段階のレーンは現在使われておらず、内部の activation・復旧ロジックが保守の負担になっている。

## Decision

### D1: plan-only / activate のレーンを削除する

`/track:plan-only` と `/track:activate` のレーンを削除する。

## Reassess When

- materialize 前に別ブランチでトラックを計画・レビューしたい要求が再び出てきたとき。

## Related

- `knowledge/adr/2026-05-26-0518-active-track-write-guard.md` — frozen ブロックを branch ベースの write validation に置き換える ADR。
