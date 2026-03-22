<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# INF-16: pr_review.rs hexagonal リファクタリング — std::fs / std::io を CLI 層に移動

pr_review.rs の resolve_reviewer_provider から std::fs / std::io を CLI 層に移動。INF-15 の既存 violation を解消し、INF-17 での error 昇格を可能にする。

## Phase 1: usecase 層の I/O 除去

T001: resolve_reviewer_provider のシグネチャを &Path → &str に変更。
PrReviewError から Io variant と ProfilesNotFound variant を削除。
std::path::Path import を削除。テストを &str ベースに書き換え。

- [x] resolve_reviewer_provider(&Path) → resolve_reviewer_provider(&str) に変更。PrReviewError から Io/ProfilesNotFound を削除。テスト書き換え。

## Phase 2: CLI 呼び出し元修正 + CI

T002: apps/cli/src/commands/pr.rs の 2 箇所でファイル読み込みを CLI 側に移動。
CI 全通し + usecase-purity warning ゼロ確認。

- [x] CLI 側 (pr.rs) でファイル読み込みを行い &str を渡す。ファイル不存在時の fail-closed テスト追加。CI 全通し + usecase-purity warning ゼロ確認。
