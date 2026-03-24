# DiffScope と scope filtering は usecase 層に配置

## Status

Accepted

## Context

RVW-11（diff スコープ強制）の設計で、DiffScope 型と finding のスコープ分類ロジックをどの層に置くかが問題になった。

当初の案は domain 層への配置だったが、Codex planner のレビューで以下の問題が指摘された:

- `ReviewFinding` と `ReviewFinalPayload` は usecase 層（`verdict.rs`）に所属している
- domain 層は serde を使わない規約（`types.rs` L247: "This is a pure domain type without serde"）
- domain に `partition_findings_by_scope(Vec<ReviewFinding>, ...)` を置くと、domain → usecase の逆方向依存が発生し、クレートグラフが壊れる

## Decision

DiffScope、RepoRelativePath、FindingScopeClass、scope filtering 関数はすべて usecase 層（`libs/usecase/src/review_workflow/scope.rs`）に配置する。

理由:
1. `ReviewFinding` が usecase 所有であるため、scope filtering ロジックも usecase に置くのが自然
2. domain 層に serde を追加する必要がない
3. scope filtering は「レビュープロセスの運用ポリシー」であり、ドメイン不変条件ではない

## Rejected Alternatives

- **domain 層に DiffScope + filtering を配置**: `ReviewFinding` への依存でクレートグラフが逆転する。domain に serde を追加すれば回避できるが、既存規約に違反。
- **domain に最小限の `RepoRelativePath` のみ配置**: 技術的には可能だが、filtering ロジックの大部分が usecase に残るため分割のメリットが薄い。

## Consequences

- Good: クレートグラフが clean（usecase → domain の一方向）
- Good: domain 層の serde-free 規約を維持
- Bad: scope filtering は pure logic だが usecase 層に住む（domain テストの恩恵を受けられない）

## Reassess When

- `ReviewFinding` を domain 層に移動する場合（domain に serde を導入する判断をした時）
- scope filtering の複雑度が増し、domain invariant として扱うべきビジネスルールが出現した場合
