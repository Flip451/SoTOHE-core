# Spec: GitHub PR-based Review Cycle

## Goal

既存のローカルレビューループ (`/track:review`) に影響を与えず、GitHub PR 上で Codex Cloud の `@codex review` を利用した非同期レビューサイクルを実行する `/track:pr-review` コマンドを追加する。push → PR 作成 → `@codex review` コメント投稿 → ポーリング → 結果解析のフローを自動化する。

## Prerequisites

- **Codex Cloud GitHub App** がリポジトリにインストール済みであること
- `gh` CLI が認証済みであること

## Scope

### In scope

- `scripts/pr_review.py`: PR オーケストレーションスクリプト（push / ensure-pr / trigger-review / poll-review / run サブコマンド）
- `@codex review` コメント投稿によるレビュートリガー（`gh api` 経由）
- GitHub API ポーリングによるレビュー完了検出（author + state チェック）
- 完了した Codex Cloud レビューの取得・パース（review body + inline comments）
- `ReviewFinding` 型への正規化、actionable findings (P0/P1) のカウント、pass/fail 判定
- `AGENTS.md`: Codex Cloud 向けレビューガイドライン（コーディングルール、セキュリティ規約、severity policy）
- `Makefile.toml` ラッパータスク: `track-pr-push` / `track-pr-ensure` / `track-pr-review`
- `.claude/commands/track/pr-review.md` コマンドプロンプト
- `.claude/settings.json` / `verify_orchestra_guardrails.py` のパーミッション更新
- テスト: PR lifecycle / trigger comment / poll timeout・success / result parsing / fail-closed / path sanitization / guardrails
- ドキュメント更新: CLAUDE.md / workflow.md / DEVELOPER_AI_WORKFLOW.md

### Out of scope

- GitHub Actions ワークフローの追加（ローカルオーケストレーションのみ）
- 非 Codex reviewer の構造化出力対応（v2 で検討）
- PR の自動マージ（ユーザー手動操作を維持）
- 既存の `/track:review` ローカルワークフローの変更
- Codex Cloud レビュー自体のカスタマイズ（`AGENTS.md` で設定する範囲のみ）

## Constraints

- `gh` CLI 直接実行は `.claude/settings.json` で forbidden のまま維持。`cargo make` ラッパー経由のみ
- `git push` 直接実行もブロック維持。`scripts/pr_review.py` 内から `git push` を呼ぶ
- レビュートリガーは `@codex review` コメントの投稿（`gh api` で PR comment を POST）
- レビューは非同期: Codex Cloud がレビューを投稿するまでポーリングが必要
- ポーリング: デフォルト間隔 15 秒、タイムアウト 10 分（設定可能）
- レビュー完了判定: GitHub API でトリガータイムスタンプ以降に作成された review のうち、author が Codex bot かつ state が完了であるものを検出（古い review の誤検出を防止）
- GitHub App 未インストール検出: タイムアウト時に Codex bot の activity（コメント・review）が PR 上に一切ない場合、「GitHub App not installed」として generic timeout と区別したエラーを出力
- 結果パース: GitHub API から review body + inline comments を取得し、actionable findings をカウント
- severity policy: P0 (CRITICAL/HIGH) と P1 (MEDIUM) のみ actionable。LOW/INFO はスキップ
- PR review comment に絶対パス・secrets・内部環境情報（ホスト名、内部 URL、環境変数値等）を含めない（パス・環境情報サニタイズ）
- 1 run = 1 `@codex review` トリガー（前回の Codex review は編集・削除しない、履歴保持）
- 非構造化 reviewer（例: `claude` profile）は fail-closed で明確なエラーメッセージを出力する
- `AGENTS.md` に Review guidelines セクションを配置し、Codex Cloud がレビュー時に参照するルールを記述

## Acceptance Criteria

1. `/track:review` が既存通り動作する（回帰なし）
2. `/track:pr-review` が `track/<id>` ブランチを push し、PR を作成 or 再利用する
3. `@codex review` コメントが PR に投稿され、Codex Cloud レビューがトリガーされる
4. ポーリングにより Codex Cloud レビューの完了が検出される
5. レビュー結果（body + inline comments）が取得・パースされ、actionable findings がカウントされる
6. `gh` / `git push` の直接実行が `.claude/settings.json` で引き続きブロックされる
7. `cargo make verify-orchestra` が新ラッパーを含めてパスする
8. 非構造化 reviewer provider（`claude` profile）で `/track:pr-review` 実行時に fail-closed エラーが出力される
9. `cargo make ci` が全チェック通過する

## Related Conventions (Required Reading)

- `project-docs/conventions/security.md`
