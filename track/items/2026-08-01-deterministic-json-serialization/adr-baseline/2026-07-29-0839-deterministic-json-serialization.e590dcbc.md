---
adr_id: "2026-07-29-0839-deterministic-json-serialization"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29; chat_segment:codex-task-019fbb90-fba3-7e81-9d57-66b4b6d5d1d3:2026-08-01:phase0-boundary-approval「承認」"
    status: proposed
---
# sotp 生成 JSON のキー順を決定的にする

## Context

sotp が生成・更新する JSON（特に review.json）は serialize のたびにキー順が変わることがあり、内容が同一でも diff にノイズが出る。review 成果物や track artifacts は git 管理・レビュー対象であり、キー順の不安定性は差分レビューのコストを直接増やす。原因候補は HashMap ベースの serialize。

## Decision

### D1: sotp が生成する全 JSON を決定的なキー順で serialize する

review.json を最優先に、sotp が書き出す全 JSON 成果物を対象とする。実装方式（IndexMap / BTreeMap への置換、または書き出し経路への canonical writer 挿入）は実装 track で選定する。いずれの方式でも「同一内容なら同一バイト列」を保証する。

## Consequences

- 良: 差分レビューのノイズが消える。「内容不変なのにキー順だけ変わった」を hash ベースの freshness 判定・commit gate が変更ありと誤検知する churn が消える。生成物の byte 比較（baseline 系ゲートと同型の検証）が可能になる。
- 負: 影響は移行時点で進行中の track に限られる — その再生成 artifact（signal 系・review.json）に一度だけキー順再配置の diff が出る。完了済み track の成果物は再 serialize されないため無影響で、リポジトリ全体の一括差分は発生しない。

## Reassess When

- serialize 性能が問題になるほど大きい JSON を扱うようになったとき。
