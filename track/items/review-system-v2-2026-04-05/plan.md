<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Review System v2: frozen scope 廃止とスコープ独立型レビュー

Review System v2: frozen scope を廃止し、スコープ独立型レビューシステムを実装する。
毎回 diff から動的にスコープを計算し、before/after hash でレビュー中の変更を検出。
approved 判定は最新 final verdict のハッシュ一致のみ。fast は参考値。
ADR: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md

## Domain 型 + Port 定義

ScopeName, ReviewTarget, ReviewHash, Verdict, FastVerdict, Finding, ReviewOutcome<V>, ReviewState を domain 層に実装。
Make Illegal States Unrepresentable: Verdict::FindingsRemain は非空保証、Finding::new は空 message 拒否。
永続化 port trait: ReviewReader + ReviewReaderError, ReviewWriter + ReviewWriterError, CommitHashReader + CommitHashWriter + CommitHashError。
ReviewScopeConfig: 純粋分類ロジック（globset 依存）。classify, get_scope_names, contains_scope, all_scope_names + ScopeConfigError。
全型に unit test を追加。

- [x] Domain 純粋型 (ScopeName, ReviewTarget, ReviewHash, Verdict, FastVerdict, Finding, ReviewOutcome<V>, ReviewState) + unit tests
- [ ] Domain port traits (ReviewReader + ReviewReaderError, ReviewWriter + ReviewWriterError including init/reset semantics: reset archives then creates new review.json without clearing .commit_hash, CommitHashReader + CommitHashWriter + CommitHashError)
- [x] Domain: ReviewScopeConfig (classify, get_scope_names, contains_scope, all_scope_names) + ScopeConfigError + globset dependency + operational/other_track exclusion + placeholder expansion (<track-id>, <other-track>) + multi-scope match: file in multiple named scopes → include in both + unit tests

## UseCase: ReviewCycle

Reviewer + ReviewerError, DiffGetter + DiffGetError, ReviewHasher + ReviewHasherError の usecase port trait を定義。
ReviewCycle<R, H, D> オーケストレーター + ReviewCycleError を実装: review(), fast_review(), get_review_targets(), get_review_states()。
before/after hash 比較、UnknownScope、FileChangedDuringReview、空スコープ Skipped、configured-but-empty scope → Empty を含む。
ReviewCycle は永続化しない — review()/fast_review() の結果を永続化するのは呼び出し側（CLI）の責務。
mock port を使った unit test で全フローを検証。

- [ ] UseCase port traits (Reviewer + ReviewerError, DiffGetter + DiffGetError, ReviewHasher + ReviewHasherError) + ReviewCycle<R,H,D> + ReviewCycleError + unit tests with mock ports

## Infrastructure: ハッシュ・diff（v1 移植）

v1 SystemGitHasher を SystemReviewHasher に移植（ソート済みマニフェスト、tombstone、rvw1: 接頭辞、O_NOFOLLOW）。
v1 GitDiffScopeProvider を GitDiffGetter に移植（merge-base セマンティクス、4 ソース和集合）。

- [ ] Infrastructure: SystemReviewHasher (sorted manifest, tombstone, rvw1: prefix, O_NOFOLLOW, post-open repo root validation) + GitDiffGetter (merge-base, 4-source union) — v1 migration

## Infrastructure: 永続化

review.json v2 codec（schema_version: 2, スコープ毎 rounds 配列、findings 含む）。
FsReviewReader / FsReviewWriter（fs4::lock_exclusive による排他ロック）。
FsCommitHashReader（.commit_hash 読み込み + infra 内で ancestry 検証）。
FsCommitHashWriter（atomic write + clear）。
.commit_hash を .gitignore に追加（ローカル状態のみ、ブランチ切替時はフォールバック）。

- [ ] Infrastructure: FsReviewReader/Writer (review.json v2 codec, fs4 locking) + FsCommitHashReader/Writer (.commit_hash, ancestry validation: fail → None fail-closed, atomic write, clear) + .commit_hash を .gitignore に追加

## CLI 統合

CLI コマンド（check-approved, review status, record-round）を v2 API に接続。
composition root で port 構築と ReviewCycle への注入（CodexReviewer adapter 構築含む）。
CommitHash フォールバック: CommitHashReader が None → git rev-parse main で SHA 解決。
review-scope.json パターン拡張（harness-policy に DEVELOPER_AI_WORKFLOW.md, README.md, track/review-scope.json を追加、planning_only 廃止、normalize 廃止）。
track-commit-message フローに .commit_hash 書き込みを追加。

- [ ] CLI integration: check-approved/review-status/record-round commands + composition root (CodexReviewer adapter construction + CommitHash fallback: None → git rev-parse main) + review-scope.json pattern updates (planning_only 廃止, normalize 廃止, harness-policy 拡張: DEVELOPER_AI_WORKFLOW.md, README.md, track/review-scope.json を追加) + .commit_hash write in commit flow

## v1 クリーンアップ

v1 レビューコード削除: domain ReviewCycle, CycleGroupState, frozen scope, ReviewPartitionSnapshot, GroupPartition, DiffScope 関連型。
usecase RecordRoundProtocol, has_scope_drift, check_cycle_staleness_any, reclassified_paths_outside_cycle_groups 削除。
infrastructure 旧 review.json codec, effective_diff_base 削除。
既存テストを v2 API に更新。
v1→v2 マイグレーション: v1 review.json (schema_version:1) は無視（デコーダが空として扱う）。既存トラックは init + clear で移行。v1 approved_head → .commit_hash に置換。

- [ ] v1 cleanup: remove domain ReviewCycle/CycleGroupState/frozen scope/ReviewPartitionSnapshot/GroupPartition/DiffScope, usecase RecordRoundProtocol/has_scope_drift/staleness/reclassified_paths_outside_cycle_groups, infra old codec/effective_diff_base, update tests + v1→v2 migration (v1 review.json schema_version:1 は無視, init+clear で移行, v1 approved_head → .commit_hash に置換)
