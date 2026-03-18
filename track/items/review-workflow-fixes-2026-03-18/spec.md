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
  2. zero-findings 検出の2段階フォールバック:
     - 第1段階: `issues/{pr}/reactions` API で bot の `+1` リアクション検出（`created_at >= trigger_timestamp`）
     - 第2段階（reaction が古い場合のフォールバック）: bot の issue comment で "Didn't find any major issues" テキストを `created_at >= trigger_timestamp` で検出。GitHub API は同一ユーザーの同一リアクションを重複作成せず HTTP 200 を返すため、2回目以降の zero-findings で reaction の `created_at` が更新されない場合がある。テキストコメントは毎回新規作成されるため、常にフレッシュな `created_at` を持つ
  3. `poll_review` (standalone) の zero-findings 時ペイロードを既存契約に準拠: `{"verdict":"zero_findings","findings":[]}` を stdout に出力し exit 0 で終了する（reaction 由来であることは stderr に出力）
  4. `poll_review_for_cycle` の返り値を拡張し、zero-findings（reaction/comment 検出、review なし）と findings-present（review JSON あり）を区別できるようにする
  5. `review_cycle` で zero-findings を受け取った場合は `parse_review` をスキップし、直接成功を報告する

### WF-43: code_hash 自己参照循環（CRITICAL）

- **問題**: `record-round` が `git write-tree` で index hash を計算し `metadata.json` に書き込むが、再 staging 後に `check-approved` が計算する hash は `metadata.json` の変更を含むため永久にミスマッチ
- **影響**: `/track:review` → `/track:commit` フローが完全にブロックされる
- **根本原因**: `git write-tree` が staged index 全体（`metadata.json` 含む）をハッシュする。`code_hash` を metadata.json に書き込むとファイル内容が変わり、hash が変わる自己参照循環
- **修正方式**: code_hash 正規化（方式 D）。hash 計算時に不安定フィールドを固定センチネル値に置き換えた正規化版で tree hash を計算する:
  - `review.code_hash` → `"PENDING"`（自己参照フィールド）
  - `updated_at` → `"1970-01-01T00:00:00Z"`（`FsTrackStore::update()` が毎回更新するため、複数 write 間で変動する）
  - これにより上記2フィールド以外の全フィールド（review.status, review.groups, tasks 等）が hash に含まれ、tamper 検出可能
- **ドメインメソッド変更**: 現在の `ReviewState::record_round` は freshness check と code_hash 書き込みを一体で行うが、方式 D では「freshness check → review state 書き込み → 外部で正規化 hash 計算 → code_hash 書き戻し」の4段階が必要。以下のようにメソッドを分離する:
  - `record_round_with_pending(round_type, group, result, expected_groups, pre_update_hash)`: freshness check（stored hash vs `pre_update_hash`、初回は `code_hash == None` なのでスキップ）→ review verdict/status/groups 書き込み → `code_hash` を `"PENDING"` に設定。`code_hash` が `None` の場合も `Some("PENDING")` を挿入
  - `set_code_hash(hash)`: 外部で計算した正規化 hash を `code_hash` に書き戻すだけのメソッド
  - 既存の `record_round` は互換性のため残す
- **record-round のフロー（CLI 層）**:
  1. **Pre-update 正規化 hash 計算**: 現在の staged metadata.json から正規化 hash を計算
  2. **Review state 書き込み**: `store.update()` 内で `record_round_with_pending(round_type, group, result, expected_groups, pre_update_hash)` を呼び出し。freshness check + review state + code_hash: `"PENDING"` を単一 write で書き込み → re-stage
  3. **Post-update 正規化 hash 計算**: staged metadata.json（code_hash: `"PENDING"` 入り）を正規化 → 一時 index で `git write-tree` → hash H1。正規化は `code_hash` を `"PENDING"` に、`updated_at` を epoch に置き換えるため、step 2 で書いた `"PENDING"` がそのまま使われ、正規化は実質ノーオプ（code_hash 部分）
  4. **code_hash 書き戻し**: `store.update()` 内で `set_code_hash(H1)` を呼び出し → re-stage。`updated_at` が変わるが正規化対象なので hash に影響しない
- **check-approved のフロー**:
  1. staged metadata.json を読み（code_hash: H1 入り）
  2. 正規化（`review.code_hash` → `"PENDING"`, `updated_at` → `"1970-01-01T00:00:00Z"`）を適用
  3. 一時 index で `git write-tree` → H1（review state は record-round step 2 と同一、updated_at は正規化済み）
  4. staged metadata.json の code_hash H1 と比較 → 一致 → OK
- **利点**:
  - metadata.json は SSoT のまま（code_hash を含む）
  - metadata.json は hash に含まれる（正規化される2フィールド以外で tamper 検出可能）
  - hash は commit される tree の意味的内容を正確に表す
  - re-stage 可能（staging 順序依存なし）
  - pre-update freshness check で「前回レビュー後のコード変更」を検出可能
- **実装**: `GitRepository` trait（`libs/infrastructure/src/git_cli.rs:25`）に `index_tree_hash_normalizing()` メソッドを追加。内部で一時 `GIT_INDEX_FILE` を使い、metadata.json の blob を正規化版に差し替えて `git write-tree` を実行
- **決定論的シリアライズ**: `TrackReviewDocument.groups` を `HashMap` から `BTreeMap` に変更し、JSON キー順序を決定論的にする
- **調査**: git-appraise (Google) のレビュー状態外部保存パターン、ビルド時 hash 埋め込みパターン、Solidity metadata hash のセンチネル方式を参考
- **対象ファイル**:
  - `libs/infrastructure/src/git_cli.rs`（`GitRepository` trait に `index_tree_hash_normalizing` 追加 + 実装）
  - `libs/domain/src/review.rs`（`ReviewState::record_round_with_pending` + `set_code_hash` メソッド追加）
  - `libs/infrastructure/src/track/codec.rs`（`TrackReviewDocument.groups` を `BTreeMap` に変更）
  - `apps/cli/src/commands/review.rs`（`run_record_round`, `run_check_approved` で正規化 hash + 新ドメインメソッドを使用）

## 制約

- `metadata.json` の schema_version 3 は変更しない
- `ReviewState` ドメイン型のインターフェースは維持する
- 既存の `index_tree_hash()` は互換性のため残す（新メソッド `index_tree_hash_normalizing()` を追加）
- 正規化対象フィールドは `review.code_hash`（→ `"PENDING"`）と `updated_at`（→ `"1970-01-01T00:00:00Z"`）の2つ
- `null`/`None` は使わない。code_hash が未設定の場合も正規化前に `"PENDING"` を**挿入**する
- 正規化は metadata.json の JSON シリアライズに依存するため、`serde_json` の決定論的出力を前提とする。`TrackReviewDocument.groups` は `BTreeMap` を使用してキー順序を保証する
- `GhClient` trait の既存メソッドは変更しない（`list_reactions` を追加のみ）
- `record-round` の pre-update freshness check は既存の `ReviewState::record_round` のコード変更検出を維持する

## 受け入れ基準

1. `is_codex_bot("chatgpt-codex-connector[bot]")` が `true` を返す
2. `poll_review` が bot の 👍 リアクションを zero-findings 完了として検出する（トリガー時刻以降）
3. `poll_review` が reaction が古い場合にテキストコメントフォールバックで zero-findings を検出する
4. `poll_review` (standalone) が zero-findings 時に `{"verdict":"zero_findings","findings":[]}` を stdout に出力する
5. `review_cycle` が zero-findings（reaction/comment のみ、review なし）で正常に成功を返す
6. `record-round` → re-stage metadata.json → `check-approved` で正規化 hash が一致する
7. `record-round` の pre-update freshness check が「前回レビュー後のコード変更」を正しく検出する
8. 初回レビュー（code_hash 未設定）で record-round → check-approved が正常に動作する
9. ソースコードを変更した場合は正規化 hash が正しく不一致になる（セキュリティ保証）
10. metadata.json の review.status や tasks を改ざんした場合も正規化 hash が不一致になる（tamper 検出）
11. `updated_at` の変動は正規化により hash に影響しない
12. `cargo make ci` が通る
13. 既存の `index_tree_hash()` テストが壊れない
14. 複数 review group がある場合でも正規化 hash が一致する（BTreeMap 順序保証）

## 出典

- [source: discussion — PR #2, #36 実データ分析 2026-03-18]
- [source: discussion — spec-template-foundation-2026-03-18 での実体験]
- [source: inference — review-infra-hardening-2026-03-18 で導入された ReviewState インフラの設計バグ]
- [source: inference — git-appraise (Google) のレビュー状態外部保存パターン]
- [source: inference — Solidity metadata hash のセンチネル方式]
- [source: inference — GitHub Reactions API は同一ユーザーの重複リアクションを再作成せず HTTP 200 を返す]
