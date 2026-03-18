# Spec: WF-42+WF-43 Review Workflow Critical Fixes

## 目標

正規の `/track:review` → `/track:commit` ワークフローと `/track:pr-review` の3つのクリティカルバグを修正し、レビュー・コミットパイプラインを復旧する。

## スコープ

### WF-42 課題1: CODEX_BOT_LOGINS 欠落（HIGH）

- **問題**: `CODEX_BOT_LOGINS` が `chatgpt-codex-connector[bot]` を認識しない
- **影響**: `/track:pr-review` が Codex Cloud のレビューを検出できずタイムアウト（600秒）
- **対象ファイル**: `apps/cli/src/commands/pr.rs`
- **修正**: 定数配列に `chatgpt-codex-connector[bot]` を追加

### WF-42 課題2: 完了判定ロジックの不一致（HIGH）

- **問題**: `poll_review` が reviews API と comments API のみをチェックし、`issues/{pr}/reactions` API の bot 👍 リアクションを検出しない
- **影響**: findings なし時の完了シグナル（👍 リアクション）を見逃し、レビュー完了を検出できない
- **確認済み動作**（PR #2, #36）:
  - findings なし → bot が PR に 👍 リアクション + "Didn't find any major issues" テキストコメント
  - findings あり → bot が `COMMENTED` state の PR review を投稿 + review body にインライン findings
- **対象ファイル**: `apps/cli/src/commands/pr.rs`（`poll_review`, `poll_review_for_cycle`）
- **修正**: `GhClient` trait に `list_reactions` メソッドを追加し、bot の `+1` リアクションを完了シグナルとして検出

### WF-43: code_hash 自己参照循環（CRITICAL）

- **問題**: `record-round` が `git write-tree` で index hash を計算し `metadata.json` に書き込むが、再 staging 後に `check-approved` が計算する hash は `metadata.json` の変更を含むため永久にミスマッチ
- **影響**: `/track:review` → `/track:commit` フローが完全にブロックされる
- **根本原因**: `git write-tree` が staged index 全体（`metadata.json` 含む）をハッシュする
- **修正**: hash 計算時に `metadata.json` を除外する。`git ls-files --stage` + `git mktree` で `metadata.json` を除外した tree を構築する方式（staging 状態を変更しない読み取り専用アプローチ）

## 制約

- `metadata.json` の schema_version 3 は変更しない
- `ReviewState` ドメイン型のインターフェースは維持する（hash 除外は infrastructure 層で完結）
- 既存の `index_tree_hash()` は互換性のため残す（新メソッド `index_tree_hash_excluding()` を追加）
- `GhClient` trait の既存メソッドは変更しない（`list_reactions` を追加のみ）

## 受け入れ基準

1. `is_codex_bot("chatgpt-codex-connector[bot]")` が `true` を返す
2. `poll_review` が bot の 👍 リアクションを zero-findings 完了として検出する
3. `record-round` → 再 stage → `check-approved` で hash が一致する
4. ソースコードを変更した場合は hash が正しく不一致になる（セキュリティ保証）
5. `cargo make ci` が通る
6. 既存の `index_tree_hash()` テストが壊れない

## 出典

- [source: discussion — PR #2, #36 実データ分析 2026-03-18]
- [source: discussion — spec-template-foundation-2026-03-18 での実体験]
- [source: inference — review-infra-hardening-2026-03-18 で導入された ReviewState インフラの設計バグ]
