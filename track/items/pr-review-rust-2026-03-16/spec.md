# Spec: STRAT-03 Phase 4 — PR review orchestration の Rust 化

## Feature Goal

`scripts/pr_review.py` の全責務（trigger-review, poll-review, review-cycle）を `sotp pr` サブコマンド群に移行し、`/track:pr-review` が Python 非依存で成立するようにする。parse-review は review-cycle 内部のステップとして実装する（独立コマンドとしては公開しない — Python 版でも `cmd_parse_review` は `cmd_run` 内部から呼ばれるのみ）。

## Scope

### In Scope

- `GhClient` trait に PR review 系メソッド追加（issue comment 投稿、reviews/comments 取得）
- `sanitize_text` usecase ユーティリティ（秘匿情報・パス・IP 除去の正規表現群 — `libs/usecase` に配置、既存の `review_workflow.rs` と同レイヤー）
- `ReviewFinding` / `ReviewResult` usecase 型 + severity 分類 + review body パース（`libs/usecase/src/pr_review.rs` に配置）
- `sotp pr trigger-review <pr>` — `@codex review` コメント投稿
- `sotp pr poll-review <pr> <trigger-timestamp>` — ポーリングによるレビュー待機
- `sotp pr review-cycle` — フルサイクル（push → ensure-pr → trigger → poll → parse → report）
- `Makefile.toml` の `track-pr-review` を Rust バイナリに切替
- `scripts/pr_review.py` + `scripts/test_pr_review.py` の削除

### Out of Scope

- advisory hooks (.claude/hooks/*.py) の移行（Phase 6）
- `sotp review run-codex-local` の変更
- reviewer provider routing の変更

## Constraints

- 既存の `GhClient` trait への追加は後方互換を維持
- `sanitize_text` は Python 版と同等のパターン網羅（abs paths, secrets, localhost, RFC1918）
- ポーリングのデフォルト: 15秒間隔、600秒タイムアウト（Python 版と同値）
- reviewer provider の fail-closed 検証を維持（structured provider のみ許可）
- `agent-profiles.json` からの reviewer provider 解決ロジックを踏襲
- `review-cycle` は `track/<id>` ブランチでのみ実行可能（`plan/<id>` ブランチは fail-closed で拒否 — Python 版 `_resolve_track_context()` と同等）

## Acceptance Criteria

- [ ] `cargo make track-pr-review` が `sotp pr review-cycle` を実行する（Python 非依存）
- [ ] `sotp pr trigger-review` が `@codex review` コメントを投稿し、サーバータイムスタンプを返す
- [ ] `sotp pr poll-review` が Codex bot レビューをポーリングし、JSON で返す
- [ ] `sotp pr poll-review` が stale review（trigger 前）を正しく無視する
- [ ] `sotp pr poll-review` が paginated/concatenated GitHub API レスポンスを正しくパースする
- [ ] `sotp pr poll-review` がタイムアウト時に bot activity の有無で異なるメッセージを出す
- [ ] `sotp pr review-cycle` が `plan/<id>` ブランチで fail-closed 拒否する
- [ ] `sanitize_text` が秘匿情報・絶対パス・内部 IP・RFC1918 アドレスを除去する（Python 版テスト相当）
- [ ] `scripts/pr_review.py` および `scripts/test_pr_review.py` が削除されている
- [ ] `cargo make ci` が通る
- [ ] `cargo make scripts-selftest` が通る（test_pr_review.py 除去後）
