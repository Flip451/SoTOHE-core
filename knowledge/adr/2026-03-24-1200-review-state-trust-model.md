# Review state trust model と metadata.json 自己参照問題

## Status

Accepted

## Context

`check-approved` は `metadata.json` 内の `review` セクションを読んで review 状態を判定する。しかし `metadata.json` 自体が `track/items/<id>/` 配下にあり、planning-only allowlist に含まれる。

これにより、`metadata.json` の `review` オブジェクトを `{status: "not_started", groups: {}}` にリセットして staging すると、`detect_planning_only()` が true を返し、`check-approved` の NotStarted+empty fast-path を通過してコミットできる。つまり、ガードが自身の状態を格納するファイルをガードしているという自己参照的な循環が存在する。

この問題はレビューサイクル中に gpt-5.4 reviewer が指摘した（Full Model R7/R8）。

## Decision

現時点では既知の制限として受け入れ、将来の `review.json` 分離トラックで構造的に修正する。

修正方針:
1. `metadata.json` から `review` セクションを `track/items/<id>/review.json` に分離
2. `review.json` を planning-only allowlist から除外
3. `record-round` が `review.json` を PrivateIndex 経由で書き込み（check-approved の前に Approved 状態が書かれる）
4. `check-approved` が `review.json` を読む
5. 手動リセットによる bypass は `review.json` が planning-only でないため不可能になる

循環は発生しない: `record-round` がレビュープロセス内で Approved を書き → `check-approved` がコミット時にそれを確認する。

## Rejected Alternatives

- `metadata.json` を planning-only allowlist から除外: タスク状態遷移（`track-transition`）のたびに review が必要になり、日常操作に大きな影響
- review state を git notes に格納: pre-commit 時点で HEAD が存在しないため、タイミングが困難
- `check-approved` で committed version（HEAD）と比較: PrivateIndex の二相プロトコルとの整合性が複雑

## Consequences

- Good: 現行の planning-only bypass が正常に機能（タスク遷移、計画変更が review なしでコミット可能）
- Bad: metadata.json 手動リセットによる bypass ベクトルが残存
- Bad: bypass は commit diff で視認可能だが、構造的防止ではない

## Reassess When

- `review.json` 分離トラック実施時にこの ADR を Superseded にする
