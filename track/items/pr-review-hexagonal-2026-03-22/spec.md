# spec: pr-review-hexagonal (INF-16)

## Goal

`libs/usecase/src/pr_review.rs` の `resolve_reviewer_provider` から `std::fs` / `std::io` を除去し、usecase 層の hexagonal purity を達成する。

## Scope

### IN scope

1. `resolve_reviewer_provider` のシグネチャ変更: `&Path` → `&str`（ファイル内容）
2. `PrReviewError` から `Io` variant と `ProfilesNotFound` variant を削除
3. CLI 側 (`apps/cli/src/commands/pr.rs`) でファイル読み込みを行い `&str` を渡す
4. CLI 側のエラーパステスト追加（ファイル不存在・読み取り失敗時の fail-closed 確認）
5. usecase テストの書き換え（tempfile → 直接 `&str`）

### OUT of scope

- `pr_review.rs` の他の関数のリファクタリング
- INF-17（warning → error 昇格）— 別トラック

## Constraints

- `sotp verify usecase-purity` が `pr_review.rs` に対して warning ゼロになること
- 既存テストのカバレッジを維持すること

## Related Conventions (Required Reading)

- `project-docs/conventions/hexagonal-architecture.md`

## Acceptance Criteria

1. `bin/sotp verify usecase-purity` が warning ゼロで pass
2. `cargo make ci` が pass
3. `resolve_reviewer_provider` が `&str` を受け取り、I/O を行わない
4. `PrReviewError` に `std::io::Error` 依存がない
5. CLI 側でファイル不存在時に適切なエラーメッセージで fail-closed すること
