# Spec: WF-42+WF-43 Review Workflow Critical Fixes

## 目標

正規の `/track:review` → `/track:commit` ワークフローと `/track:pr-review` の2つのクリティカルバグを修正し、レビュー・コミットパイプラインを復旧する。

## スコープ

### WF-42: CODEX_BOT_LOGINS 欠落（HIGH）

- **問題**: `CODEX_BOT_LOGINS` が `chatgpt-codex-connector[bot]` を認識しない
- **影響**: `/track:pr-review` が Codex Cloud のレビューを検出できずタイムアウト（600秒）
- **対象ファイル**: `apps/cli/src/commands/pr.rs`
- **修正**: 定数配列に `chatgpt-codex-connector[bot]` を追加

### WF-43: code_hash 自己参照循環（CRITICAL）

- **問題**: `record-round` が `git write-tree` で index hash を計算し `metadata.json` に書き込むが、再 staging 後に `check-approved` が計算する hash は `metadata.json` の変更を含むため永久にミスマッチ
- **影響**: `/track:review` → `/track:commit` フローが完全にブロックされる
- **根本原因**: `git write-tree` が staged index 全体（`metadata.json` 含む）をハッシュする
- **修正**: hash 計算時に `metadata.json` を除外する。`git ls-files --stage` + `git mktree` で `metadata.json` を除外した tree を構築する方式（staging 状態を変更しない読み取り専用アプローチ）

## 制約

- `metadata.json` の schema_version 3 は変更しない
- `ReviewState` ドメイン型のインターフェースは維持する（hash 除外は infrastructure 層で完結）
- 既存の `index_tree_hash()` は互換性のため残す（新メソッド `index_tree_hash_excluding()` を追加）

## 受け入れ基準

1. `is_codex_bot("chatgpt-codex-connector[bot]")` が `true` を返す
2. `record-round` → 再 stage → `check-approved` で hash が一致する
3. ソースコードを変更した場合は hash が正しく不一致になる（セキュリティ保証）
4. `cargo make ci` が通る
5. 既存の `index_tree_hash()` テストが壊れない
