<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Track ワークフロー UX 改善

track-commit-message の CI 出力抑制、plan/ ブランチ PR 対応、PR body-file 化、add-all エラー処理など、日常運用で発生する UX 不具合を一括修正する。
セキュリティ・状態管理には影響しない、ツーリング改善に閉じたスコープ。

## CI 出力抑制

- [x] Makefile.toml の track-commit-message タスクから dependencies = [ci] を外し、task script 内で CI を実行して出力を tmp/ci-output.log にリダイレクトする。stdout には最終結果のサマリー行のみ出力し、失敗時は tmp/ci-output.log の末尾20行を表示する

## plan/ ブランチ PR 対応

- [x] usecase 層に PR push / ensure-pr のポート trait を定義し、infrastructure 層に gh CLI adapter を実装する。apps/cli の pr.rs は薄い CLI adapter のみ担当する。ブランチ解決: track/<id> ブランチでは自動解決、plan/<id> ブランチでは明示的な --track-id 引数を要求する（explicit-selector ルール維持）。plan/ ブランチの PR タイトルは Plan: <track-id> にする。PR body は一時ファイル経由で gh pr create --body-file を使いフック誤検知を回避する。Makefile.toml の track-pr-push / track-pr-ensure を Rust CLI 呼び出しに変更する。scripts/pr_review.py の cmd_push() / cmd_ensure_pr() は sotp pr を subprocess で呼ぶだけに縮退させる。track/workflow.md に plan/ ブランチの PR ワークフロー（push + ensure-pr のみ、track-pr-review は対象外）を追記する
- [x] scripts/pr_review.py の cmd_push() / cmd_ensure_pr() を sotp pr push / sotp pr ensure-pr の subprocess 呼び出しに縮退させる。body-file ロジックは Rust 側に閉じ、Python 側には新規ロジックを追加しない。cmd_trigger_review / cmd_poll_review / cmd_run は track/<id> ブランチ専用のまま現状維持する（plan/ ブランチでの track-pr-review は対象外と明示）。STRAT-03 Phase 4 で全体 Rust 化予定

## add-all エラー処理

- [x] sotp git add-all の Rust 実装で git add 実行前に gitignore 対象パスを除外する。git check-ignore でフィルタリングしてから git add に渡すことで、gitignore 警告自体を発生させない。exit code の意味を変更しない

## CI 検証

- [x] cargo make ci が通ることを確認する
