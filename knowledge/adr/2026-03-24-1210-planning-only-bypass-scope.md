# Planning-only bypass は NotStarted+empty のみ許可

## Status

Accepted

## Context

`check-approved` に `planning_only` フラグを追加して、コードファイルが staged されていない場合に review guard を bypass する機能を実装した。

bypass のスコープとして 2 つの選択肢があった:
- A: `planning_only=true` なら review 状態に関係なく常に bypass
- B: `planning_only=true` かつ review が NotStarted+empty groups の場合のみ bypass

## Decision

**B を採用**: planning_only bypass は NotStarted+empty groups の場合のみ。

レビューが開始された（NotStarted 以外、または groups が非空）場合は、planning_only であっても Approved + matching hash が必要。

理由:
1. **レビュープロセスの完全性**: レビューが開始されたならば、それは承認まで完了すべき。途中で docs だけコミットして状態を中途半端にするのは不健全
2. **整合性**: review state が Approved + hash で特定のコード状態に紐づいている。docs 変更で hash が変わると状態が不整合になる
3. **fail-closed 原則**: 判断に迷う場合はブロックする方が安全

## Rejected Alternatives

- A: `planning_only=true` で全状態 bypass: Approved 後に docs を変更すると hash が変わり review state が形骸化する。レビュー開始後の integrity が保証されない
- C: review state に `planning_only_allowed` フラグを追加: 複雑度が増し、domain 層の変更が必要

## Consequences

- Good: レビューサイクルの完全性が保証される
- Good: fail-closed でセキュリティ的に安全
- Bad: Approved 後の docs-only 変更が再レビューを要求する（hash 変更による invalidation）
- Bad: 上記の Bad は review.json 分離後も残る（tree hash は全ファイルを含むため）

## Reassess When

- コードファイルのみの hash を計算する機能が実装された場合（tree hash ではなく code-only hash）
- review.json 分離トラック実施時に、hash 計算のスコープを再検討
