# Verification: GitHub PR-based Review Cycle

## Scope Verified

- [ ] `/track:review` ローカルレビューが回帰なしで動作する
- [ ] `/track:pr-review` がブランチ push → PR 作成/再利用のフローを実行する
- [ ] `@codex review` コメントが PR に投稿され、Codex Cloud レビューがトリガーされる
- [ ] ポーリングにより Codex Cloud レビュー完了が検出される（トリガー後タイムスタンプでフィルタ、タイムアウト含む）
- [ ] 古い（トリガー前の）Codex レビューが誤検出されない
- [ ] GitHub App 未インストール時にタイムアウトと区別された明確なエラーが出力される
- [ ] レビュー結果（body + inline comments）が GitHub API から取得・パースされる
- [ ] actionable findings (P0/P1) のカウントと pass/fail 判定が正しく動作する
- [ ] 絶対パス・secrets・内部環境情報（ホスト名、内部 URL、環境変数値等）が review comments に漏洩しない
- [ ] `gh` / `git push` 直接実行が引き続きブロックされる
- [ ] `cargo make verify-orchestra` がパスする
- [ ] 非構造化 reviewer provider（`claude` profile）で fail-closed エラーが出力される
- [ ] `cargo make ci` が全チェック通過する

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

(実装後に記録)

## verified_at

(実装後に記録)
