# Spec: Track ワークフロー UX 改善

## 概要

日常のトラック運用で発生する UX 不具合を一括修正する。セキュリティ・状態管理には影響しない、ツーリング改善に閉じたスコープ。

## 背景

2026-03-15 のセッションで以下の問題が発生：

1. `cargo make track-commit-message` の CI 出力が Claude Code の Bash tool 出力上限を超え、コミット成否が判別できない
2. `cargo make track-pr-push` / `track-pr-ensure` が `plan/` ブランチに非対応で、planning-only トラックの PR 作成に手動操作が必要
3. `gh pr create --body` に git 関連キーワードが含まれるとフックがブロックする
4. `cargo make add-all` が gitignore 対象ファイルの存在で失敗する

## ゴール

- `track-commit-message` が CI の詳細出力を抑制し、最終結果のみ stdout に出力する
- `track-pr-push` / `track-pr-ensure` が `plan/<id>` ブランチで動作する
- PR body がフックでブロックされない
- `add-all` が gitignore 警告で失敗しない
- `cargo make ci` が通る状態を維持する

## スコープ

### 変更対象

| ファイル | 変更内容 |
|---|---|
| `Makefile.toml` | `track-commit-message` から `dependencies = ["ci"]` を外し script 内で CI 実行 + 出力リダイレクト。`track-pr-push` / `track-pr-ensure` を Rust CLI 呼び出しに変更 |
| `libs/usecase/`, `libs/infrastructure/` | PR push / ensure-pr のポート trait と gh CLI adapter を新設 |
| `apps/cli/src/commands/pr.rs` | `sotp pr push` / `sotp pr ensure-pr` CLI adapter（薄い adapter のみ） |
| `apps/cli/src/commands/git.rs` (or related) | `add-all` で `git check-ignore` による事前フィルタを追加 |
| `scripts/pr_review.py` | `cmd_push()` / `cmd_ensure_pr()` を Rust CLI subprocess 呼び出しに縮退（新規ロジック追加なし） |
| `track/workflow.md` | ブランチ戦略セクションに `plan/` ブランチの PR ワークフローを追記 |

### 対象外

- セキュリティポリシーの変更
- track state machine の変更
- 新規スキル・コマンドの追加

## 完了条件

- [ ] `track-commit-message` の stdout が最終結果（PASSED/FAILED + サマリー）のみになる
- [ ] `plan/` ブランチで `cargo make track-pr-push '<track-id>'` / `track-pr-ensure '<track-id>'` が成功する
- [ ] `plan/` ブランチで引数省略時に `cargo make track-pr-push` / `track-pr-ensure` が fail-closed でエラーを返す
- [ ] `sotp pr ensure-pr` が `--body-file` を使い、フックでブロックされない
- [ ] Python `cmd_ensure_pr()` が Rust CLI に委譲するだけで新規ロジックを含まない
- [ ] `plan/` ブランチで `cargo make track-pr-review` が `track/<id>` ブランチ要求エラーを返す（fail-closed 維持）
- [ ] `cargo make add-all` が gitignore 対象パスを事前除外し正常終了する
- [ ] `cargo make ci` が通る
