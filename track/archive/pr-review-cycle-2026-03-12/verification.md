# Verification: GitHub PR-based Review Cycle

## Scope Verified

- [x] `/track:review` ローカルレビューが回帰なしで動作する (TestTrackReviewRegression)
- [x] `/track:pr-review` がブランチ push → PR 作成/再利用のフローを実行する (cmd_push, cmd_ensure_pr implemented)
- [x] `@codex review` コメントが PR に投稿され、Codex Cloud レビューがトリガーされる (cmd_trigger_review implemented)
- [x] ポーリングにより Codex Cloud レビュー完了が検出される（トリガー後タイムスタンプでフィルタ、タイムアウト含む） (TestPollReview)
- [x] 古い（トリガー前の）Codex レビューが誤検出されない (test_rejects_stale_review)
- [x] GitHub App 未インストール時にタイムアウトと区別された明確なエラーが出力される (test_github_app_not_installed_message)
- [x] レビュー結果（body + inline comments）が GitHub API から取得・パースされる (TestCmdParseReview)
- [x] actionable findings (P0/P1) のカウントと pass/fail 判定が正しく動作する (test_commented_with_inline_findings)
- [x] 絶対パス・secrets・内部環境情報（ホスト名、内部 URL、環境変数値等）が review comments に漏洩しない (TestSanitizeText)
- [x] `gh` / `git push` 直接実行が引き続きブロックされる (verify-orchestra PASSED)
- [x] `cargo make verify-orchestra` がパスする (CI PASSED)
- [x] 非構造化 reviewer provider（`claude` profile）で fail-closed エラーが出力される (test_claude_provider_fails_closed)
- [x] `cargo make ci` が全チェック通過する (229 Rust tests + 487+355 Python tests PASSED)
- [x] CHANGES_REQUESTED 状態で actionable==0 でも fail する (test_changes_requested_with_no_actionable_fails)
- [x] GitHub サーバータイムスタンプをトリガー時刻として使用（ローカルクロック偏差を排除）
- [x] 同秒レビューが正しく受理される (test_accepts_same_second_review)
- [x] Reused PR の stale bot activity がカウントされない (test_stale_bot_comments_not_counted_as_activity)
- [x] RFC1918 プライベート IP がサニタイズされる（URL 内・括弧内含む、substring false positive なし）
- [x] `github_pat_*` / `glpat-*` トークンがサニタイズされる
- [x] paginated GitHub API レスポンスが正しくパースされる (TestParsePaginatedJson)
- [x] エラーメッセージが sanitize_text を経由して出力される
- [x] 絶対パスサニタイズが /etc, /opt, /srv, /workspace, /root, /usr/local をカバー
- [x] multi-line inline comment の line/end_line が正しくマッピングされる (test_multiline_comment_line_range)

## Manual Verification Steps

- [ ] `track/<id>` ブランチで `/track:pr-review` を実行し、PR が作成されることを確認
- [ ] `@codex review` コメントが PR に投稿されることを確認
- [ ] Codex Cloud がレビューを投稿するまでポーリングが行われることを確認
- [ ] レビュー完了後、findings サマリーが表示されることを確認
- [ ] 指摘ありの場合、修正後に再度 `/track:pr-review` を実行し、新しいレビューラウンドが追加されることを確認
- [ ] 指摘 0 件の場合、pass 判定が返ることを確認
- [ ] 既存の `/track:review` が変更なく動作することを確認（回帰テスト）
- [ ] Codex Cloud GitHub App 未インストール時に generic timeout と区別されたエラーが出ることを確認
- [ ] 古い Codex レビューが存在する PR で再トリガーした場合、新しいレビューのみが検出されることを確認

## Result / Open Issues

- Manual verification steps require a live GitHub repository with Codex Cloud GitHub App installed.
- All automated tests pass (44 tests). Manual E2E testing deferred to first real PR review run.
- Implementation review: 2 sessions (8 rounds total), all findings resolved.

## verified_at

2026-03-12
