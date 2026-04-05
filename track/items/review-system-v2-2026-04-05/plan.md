<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Review System v2: frozen scope 廃止とスコープ独立型レビュー

Review System v2: frozen scope を廃止し、スコープ独立型レビューシステムを実装する。
毎回 diff から動的にスコープを計算し、before/after hash でレビュー中の変更を検出。
approved 判定は最新 final verdict のハッシュ一致のみ。fast は参考値。
ADR: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md

## Domain 型 + Port 定義

ScopeName, ReviewTarget, ReviewHash, Verdict, FastVerdict, Finding, ReviewOutcome<V>, ReviewState を domain 層に実装。
Make Illegal States Unrepresentable: Verdict::FindingsRemain は非空保証、Finding::new は空 message 拒否。
全型に unit test を追加。

- [x] Domain 純粋型 (ScopeName, ReviewTarget, ReviewHash, Verdict, FastVerdict, Finding, ReviewOutcome<V>, ReviewState) + unit tests
- [ ] Domain port traits (ReviewReader, ReviewWriter, CommitHashReader, CommitHashWriter)

## UseCase: ReviewCycle

Reviewer, DiffGetter, ReviewHasher の usecase port trait を定義。
ReviewCycle<R, H, D> オーケストレーターを実装: review(), fast_review(), get_review_targets(), get_review_states()。
before/after hash 比較、UnknownScope エラー、空スコープ Skipped を含む。
mock port を使った unit test で全フローを検証。

- [ ] UseCase port traits (Reviewer, DiffGetter, ReviewHasher) + ReviewCycle<R,H,D> implementation + unit tests with mock ports

## Infrastructure: ハッシュ・diff（v1 移植）

v1 SystemGitHasher を SystemReviewHasher に移植（ソート済みマニフェスト、tombstone、rvw1: 接頭辞、O_NOFOLLOW）。
v1 GitDiffScopeProvider を GitDiffGetter に移植（merge-base セマンティクス、4 ソース和集合）。
ReviewScopeConfig の glob 分類ロジックを v1 review_group_policy から移植（operational 除外、other_track 除外、プレースホルダ展開）。

- [ ] Infrastructure: SystemReviewHasher + GitDiffGetter + ReviewScopeConfig glob classification (v1 migration)

## Infrastructure: 永続化

review.json v2 codec（schema_version: 2, スコープ毎 rounds 配列、findings 含む）。
FsReviewReader / FsReviewWriter（fs4::lock_exclusive による排他ロック）。
FsCommitHashReader（.commit_hash 読み込み + infra 内で ancestry 検証）。
FsCommitHashWriter（atomic write + clear）。

- [ ] Infrastructure: FsReviewReader/Writer (review.json v2 codec, fs4 locking) + FsCommitHashReader/Writer (.commit_hash, ancestry validation, atomic write, clear)

## CLI 統合

CLI コマンド（check-approved, review status, record-round）を v2 API に接続。
composition root で port 構築と ReviewCycle への注入。
review-scope.json パターン拡張（harness-policy に track/review-scope.json 追加、planning_only 廃止）。
track-commit-message フローに .commit_hash 書き込みを追加。

- [ ] CLI integration: check-approved/review-status/record-round commands + composition root + review-scope.json pattern updates + .commit_hash in commit flow

## v1 クリーンアップ

v1 レビューコード削除: domain ReviewCycle, CycleGroupState, frozen scope 関連型。
usecase RecordRoundProtocol, has_scope_drift, check_cycle_staleness_any 削除。
infrastructure 旧 review.json codec, effective_diff_base 削除。
既存テストを v2 API に更新。

- [ ] v1 cleanup: remove domain ReviewCycle/CycleGroupState/frozen scope, usecase RecordRoundProtocol/has_scope_drift/staleness, infra old codec/effective_diff_base, update tests
