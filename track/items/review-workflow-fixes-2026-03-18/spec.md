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
  2. reaction の `created_at` をトリガー時刻と比較し、トリガー後のリアクションのみ有効とする（過去の zero-findings の `+1` が残っていても誤検出しない）
  3. `poll_review` (standalone) の zero-findings 時ペイロードを既存契約に準拠: `{"verdict":"zero_findings","findings":[]}` を stdout に出力し exit 0 で終了する（reaction 由来であることは stderr に出力）
  4. `poll_review_for_cycle` の返り値を拡張し、zero-findings（reaction 検出、review なし）と findings-present（review JSON あり）を区別できるようにする
  5. `review_cycle` で zero-findings を受け取った場合は `parse_review` をスキップし、直接成功を報告する

### WF-43: code_hash 自己参照循環（CRITICAL）

- **問題**: `record-round` が `git write-tree` で index hash を計算し `metadata.json` に書き込むが、再 staging 後に `check-approved` が計算する hash は `metadata.json` の変更を含むため永久にミスマッチ
- **影響**: `/track:review` → `/track:commit` フローが完全にブロックされる
- **根本原因**: `git write-tree` が staged index 全体（`metadata.json` 含む）をハッシュする。`code_hash` を metadata.json に書き込むとファイル内容が変わり、hash が変わる自己参照循環
- **修正方式**: code_hash 正規化（方式 D）。hash 計算時に `review.code_hash` フィールドを固定センチネル文字列 `"PENDING"` に置き換えた正規化版で tree hash を計算する。これにより code_hash 以外の全フィールド（review.status, review.groups 含む）が hash に含まれ、自己参照循環を回避する:
  1. `record-round`:
     - (a) review verdict/status/groups を `metadata.json` に書き込み（`store.update()` 経由）→ re-stage
     - (b) staged metadata.json を読み、`review.code_hash` を `"PENDING"` に置き換えた正規化版を作成
     - (c) 一時 index で正規化版の blob を差し替えて `git write-tree` → hash H1 を取得
     - (d) `metadata.json` に code_hash: H1 を書き込み → re-stage
     - **重要**: hash は post-update の review state を反映する（pre-update ではない）
  2. `check-approved`:
     - (a) staged metadata.json を読み（code_hash: H1 + post-update review state 入り）
     - (b) 同じ正規化（`review.code_hash` → `"PENDING"`）を適用
     - (c) 一時 index で `git write-tree` → H1（review state は record-round と同一）
     - (d) staged metadata.json の code_hash H1 と比較 → 一致 → OK
- **利点**:
  - metadata.json は SSoT のまま（code_hash を含む）
  - metadata.json は hash に **完全に** 含まれる（code_hash 以外の全フィールドで tamper 検出可能）
  - hash は commit される tree を正確に表す（code_hash フィールドのみ正規化）
  - re-stage 可能（staging 順序依存なし）
  - metadata-only の改変も検出可能（review.status, tasks 等の改ざんを hash で検出）
- **実装**: `GitRepository` trait に `index_tree_hash_normalizing()` メソッドを追加。内部で一時 `GIT_INDEX_FILE` を使い、metadata.json の blob を正規化版に差し替えて `git write-tree` を実行
- **決定論的シリアライズ**: `TrackReviewDocument.groups` を `HashMap` から `BTreeMap` に変更し、JSON キー順序を決定論的にする。これにより複数の review group がある場合でも、別プロセスの record-round と check-approved で同一のシリアライズ結果が保証される
- **調査**: git-appraise (Google) のレビュー状態外部保存パターン、ビルド時 hash 埋め込みパターン、Solidity metadata hash のセンチネル方式を参考
- **対象ファイル**:
  - `libs/domain/src/git.rs` or `libs/domain/src/review.rs`（`GitRepository` trait に `index_tree_hash_normalizing` 追加）
  - `libs/infrastructure/src/git_cli.rs`（正規化 + 一時 index による実装）
  - `libs/infrastructure/src/track/codec.rs`（`TrackReviewDocument.groups` を `BTreeMap` に変更）
  - `apps/cli/src/commands/review.rs`（`run_record_round`, `run_check_approved` で正規化 hash を使用）

## 制約

- `metadata.json` の schema_version 3 は変更しない
- `ReviewState` ドメイン型のインターフェースは維持する
- 既存の `index_tree_hash()` は互換性のため残す（新メソッド `index_tree_hash_normalizing()` を追加）
- 正規化のセンチネル値は固定文字列 `"PENDING"`（JSON の `"code_hash": "PENDING"`）とする。`null` は「未設定」と区別がつかないため明示的な文字列を使用
- 正規化は metadata.json の JSON シリアライズに依存するため、`serde_json` の決定論的出力を前提とする。`TrackReviewDocument.groups` は `BTreeMap` を使用してキー順序を保証する
- `GhClient` trait の既存メソッドは変更しない（`list_reactions` を追加のみ）

## 受け入れ基準

1. `is_codex_bot("chatgpt-codex-connector[bot]")` が `true` を返す
2. `poll_review` が bot の 👍 リアクションを zero-findings 完了として検出する（トリガー時刻以降のリアクションのみ）
3. `poll_review` (standalone) が zero-findings 時に `{"verdict":"zero_findings","findings":[]}` を stdout に出力する
4. `review_cycle` が zero-findings（reaction のみ、review なし）で正常に成功を返す
5. `record-round` → re-stage metadata.json → `check-approved` で正規化 hash が一致する
6. ソースコードを変更した場合は正規化 hash が正しく不一致になる（セキュリティ保証）
7. metadata.json の review.status や tasks を改ざんした場合も正規化 hash が不一致になる（tamper 検出）
8. `cargo make ci` が通る
9. 既存の `index_tree_hash()` テストが壊れない
10. 複数 review group がある場合でも、別プロセスの record-round と check-approved で正規化 hash が一致する（BTreeMap 順序保証）

## 出典

- [source: discussion — PR #2, #36 実データ分析 2026-03-18]
- [source: discussion — spec-template-foundation-2026-03-18 での実体験]
- [source: inference — review-infra-hardening-2026-03-18 で導入された ReviewState インフラの設計バグ]
- [source: inference — git-appraise (Google) のレビュー状態外部保存パターン]
- [source: inference — Solidity metadata hash のセンチネル方式]
