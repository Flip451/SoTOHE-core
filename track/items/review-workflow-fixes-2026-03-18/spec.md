---
status: draft
version: "1.0"
---

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

- **問題**: `poll_review` が reviews API と comments API のみをチェックし、`issues/{pr}/reactions` API の bot 👍 リアクションを検出しない。さらに `review_cycle()` は `poll_review_for_cycle` が review JSON を返す前提で `parse_review()` を呼ぶため、zero-findings（review オブジェクトなし、reaction のみ）のケースで review-cycle パスが破綻する
- **影響**: findings なし時の完了シグナル（👍 リアクション）を見逃し、レビュー完了を検出できない。ポーラーだけ修正しても `review_cycle` 側が review JSON を要求するため、zero-findings PR で primary review-cycle が正常完了しない
- **確認済み動作**（PR #2, #36）:
  - findings なし → bot が PR に 👍 リアクション + "Didn't find any major issues" テキストコメント（review オブジェクトは投稿されない）
  - findings あり → bot が `COMMENTED` state の PR review を投稿 + review body にインライン findings
- **対象ファイル**: `apps/cli/src/commands/pr.rs`（`poll_review`, `poll_review_for_cycle`, `review_cycle`）
- **修正**:
  1. `GhClient` trait に `list_reactions` メソッドを追加
  2. `poll_review_for_cycle` の返り値を拡張し、zero-findings（reaction 検出、review なし）と findings-present（review JSON あり）を区別できるようにする
  3. `review_cycle` で zero-findings を受け取った場合は `parse_review` をスキップし、直接成功を報告する

### WF-43: code_hash 自己参照循環（CRITICAL）

- **問題**: `record-round` が `git write-tree` で index hash を計算し `metadata.json` に書き込むが、再 staging 後に `check-approved` が計算する hash は `metadata.json` の変更を含むため永久にミスマッチ
- **影響**: `/track:review` → `/track:commit` フローが完全にブロックされる
- **根本原因**: `git write-tree` が staged index 全体（`metadata.json` 含む）をハッシュする
- **修正**: hash 計算時に `metadata.json` を除外する。一時 index ファイルを使用し、実際の staging 状態を変更しない読み取り専用アプローチ:
  1. `git write-tree` で現在の index から tree オブジェクトを取得
  2. 一時 index ファイルに `GIT_INDEX_FILE=$tmp git read-tree $tree` で復元
  3. 一時 index 上で `GIT_INDEX_FILE=$tmp git rm --cached <metadata.json path>` で除外
  4. `GIT_INDEX_FILE=$tmp git write-tree` で除外後の tree hash を取得
  5. 一時 index ファイルを削除

## 制約

- `metadata.json` の schema_version 3 は変更しない
- `ReviewState` ドメイン型のインターフェースは維持する（hash 除外は infrastructure 層で完結）
- 既存の `index_tree_hash()` は互換性のため残す（新メソッド `index_tree_hash_excluding()` を追加）
- `GhClient` trait の既存メソッドは変更しない（`list_reactions` を追加のみ）

## 受け入れ基準

1. `is_codex_bot("chatgpt-codex-connector[bot]")` が `true` を返す
2. `poll_review` が bot の 👍 リアクションを zero-findings 完了として検出する
3. `review_cycle` が zero-findings（reaction のみ、review なし）で正常に成功を返す
4. `record-round` → 再 stage → `check-approved` で hash が一致する
5. ソースコードを変更した場合は hash が正しく不一致になる（セキュリティ保証）
6. `cargo make ci` が通る
7. 既存の `index_tree_hash()` テストが壊れない

## 出典

- [source: discussion — PR #2, #36 実データ分析 2026-03-18]
- [source: discussion — spec-template-foundation-2026-03-18 での実体験]
- [source: inference — review-infra-hardening-2026-03-18 で導入された ReviewState インフラの設計バグ]
