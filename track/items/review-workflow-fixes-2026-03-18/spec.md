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
- **根本原因**: `git write-tree` が staged index 全体（`metadata.json` 含む）をハッシュする。`record-round` が metadata.json に書き込んだ後に再 stage すると hash が変わる
- **修正方式**: staging 順序制御（方式 C）。metadata.json を hash に含めたまま、staging タイミングを制御して循環を回避する:
  1. `add-all` で全ファイルを stage（metadata.json は review 書き込み前の状態）
  2. `record-round` が staged index から `git write-tree` → hash H1 を計算
  3. `record-round` が metadata.json に review state + code_hash: H1 を disk 書き込み（**再 stage しない**）
  4. `check-approved` が staged index から `git write-tree` → H1（staged metadata.json は変わっていない）
  5. `check-approved` が **disk 上の** metadata.json から code_hash を読み取り → H1 == H1 → OK
  6. commit wrapper が metadata.json を stage してから commit
- **利点**:
  - metadata.json は SSoT のまま（コミットされる版に code_hash が含まれる）
  - metadata.json は hash 計算に含まれる（tamper 防御維持）
  - 一時 index やファイル分離が不要
- **調査**: git-appraise (Google) のレビュー状態外部保存パターンとビルド時 hash 埋め込みパターンを参考に、staging 順序制御が最もシンプルと判断
- **対象ファイル**:
  - `apps/cli/src/commands/review.rs`（`run_record_round`, `run_check_approved`）
  - `libs/infrastructure/src/git_cli.rs`（`index_tree_hash` — 変更なし、既存のまま使用）
  - `libs/usecase/src/git_workflow.rs`（staging 順序の調整が必要な場合）
  - commit wrapper（`check-approved` 後に metadata.json を stage する処理追加）

## 制約

- `metadata.json` の schema_version 3 は変更しない
- `ReviewState` ドメイン型のインターフェースは維持する
- 既存の `index_tree_hash()` をそのまま使用する（新メソッド追加不要）
- `record-round` は disk 書き込み後に metadata.json を再 stage しない
- `check-approved` は code_hash を staged index ではなく disk 上の metadata.json から読み取る
- commit wrapper は `check-approved` 通過後に metadata.json を stage してから commit する
- `GhClient` trait の既存メソッドは変更しない（`list_reactions` を追加のみ）

## 受け入れ基準

1. `is_codex_bot("chatgpt-codex-connector[bot]")` が `true` を返す
2. `poll_review` が bot の 👍 リアクションを zero-findings 完了として検出する（トリガー時刻以降のリアクションのみ）
3. `poll_review` (standalone) が zero-findings 時に `{"verdict":"zero_findings","findings":[]}` を stdout に出力する
4. `review_cycle` が zero-findings（reaction のみ、review なし）で正常に成功を返す
5. `record-round` → `check-approved` で hash が一致する（metadata.json は再 stage しない）
6. ソースコードを変更した場合は hash が正しく不一致になる（セキュリティ保証）
7. `cargo make ci` が通る
8. 既存の `index_tree_hash()` テストが壊れない

## 出典

- [source: discussion — PR #2, #36 実データ分析 2026-03-18]
- [source: discussion — spec-template-foundation-2026-03-18 での実体験]
- [source: inference — review-infra-hardening-2026-03-18 で導入された ReviewState インフラの設計バグ]
