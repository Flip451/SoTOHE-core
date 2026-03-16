<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# STRAT-03 Phase 4: PR review orchestration の Rust 化

scripts/pr_review.py の全責務を sotp pr サブコマンド群に移行し、/track:pr-review が Python 非依存で成立するようにする。M2 マイルストーン達成に直結。

## Infrastructure 層: GhClient 拡張

- [x] GhClient trait に PR review 系メソッド追加 (post_issue_comment, list_reviews, list_issue_comments, list_review_comments) + SystemGhClient 実装

## Usecase 層: sanitize + review 型 + パースロジック

- [x] sanitize_text usecase ユーティリティ実装 (秘匿情報・絶対パス・内部IP・localhost・RFC1918 除去、libs/usecase に配置)
- [x] PR review usecase 型 (ReviewFinding, ReviewResult) + severity 分類 + review body パース + paginated JSON パース (libs/usecase/src/pr_review.rs)

## CLI 層: 新サブコマンド群

- [x] sotp pr trigger-review サブコマンド実装 (@codex review コメント投稿、reviewer provider 検証)
- [x] sotp pr poll-review サブコマンド実装 (GitHub API ポーリング、タイムアウト、bot activity 検出、stale review 排除、paginated API 対応)
- [x] sotp pr review-cycle フルサイクルコマンド (push → ensure-pr → trigger → poll → parse → report、track/ ブランチ必須の fail-closed ガード)

## 切替・クリーンアップ

- [x] Makefile.toml の track-pr-review を Rust に切替 + scripts/pr_review.py, scripts/test_pr_review.py 削除
- [x] scripts-selftest リストから test_pr_review.py 除去 + ドキュメント参照修正
