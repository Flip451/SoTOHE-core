<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 23, yellow: 0, red: 0 }
---

# Review System v2: frozen scope 廃止とスコープ独立型レビュー

## Goal

既存のレビューシステム (v1) を完全に置き換える。
frozen scope を廃止し、毎回 diff から動的にスコープを計算するスコープ独立型レビューシステムを実装する。
v1 の構造的問題（frozen scope / current partition / check_approved のスコープ不整合、has_scope_drift によるサイクル全体無効化、ワークフローアーティファクトのハッシュ循環）を根本解決する。

## Scope

### In Scope
- Domain 純粋型: ScopeName, ReviewTarget, ReviewHash, Verdict, FastVerdict, Finding, ReviewOutcome<V>, ReviewState [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T001]
- Domain port traits: ReviewReader, ReviewWriter, CommitHashReader, CommitHashWriter [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T002]
- UseCase port traits (Reviewer, DiffGetter, ReviewHasher) + ReviewCycle<R,H,D> オーケストレーター [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T003]
- Infrastructure: SystemReviewHasher (v1 移植), GitDiffGetter (v1 移植, merge-base セマンティクス), ReviewScopeConfig glob 分類 [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T004]
- Infrastructure: review.json v2 codec + FsReviewReader/Writer (fs4 locking) + FsCommitHashReader/Writer (.commit_hash) [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T005]
- CLI 統合: check-approved, review status, record-round を v2 API に接続 + composition root + review-scope.json パターン拡張 [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T006]
- v1 レビューコード削除: domain ReviewCycle, CycleGroupState, frozen scope 関連型, usecase RecordRoundProtocol, infra 旧 codec [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T007]

### Out of Scope
- ポリシー変更による他スコープの approval 自動無効化（accepted risk, 手動 reset で対応） [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md]
- check-approved と commit 間の TOCTOU 完全解決（accepted risk, single-user 前提） [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md]
- git index ベースのハッシュ計算（worktree ベースを維持, add-all で同期） [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md]

## Constraints
- Domain 型は純粋（I/O なし）。port trait の呼び出しは usecase 以上 [source: convention — knowledge/conventions/hexagonal-architecture.md]
- 永続化 port は domain 層、アプリケーション port は usecase 層に配置 [source: convention — knowledge/conventions/hexagonal-architecture.md]
- TDD: テストを先に書く（Red → Green → Refactor） [source: convention — .claude/rules/05-testing.md]
- パニック禁止（unwrap/expect/panic/todo は非テストコードで使用不可） [source: convention — .claude/rules/04-coding-principles.md]
- 同期のみ（async なし） [source: track/tech-stack.md]
- review.json の並行書き込みは fs4::lock_exclusive で保護 [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md]

## Domain States

| State | Description |
|-------|-------------|
| ReviewState::Required(NotStarted) | final 未実施。レビューが必要 |
| ReviewState::Required(FindingsRemain) | 最新 final が findings_remain。修正後に再レビューが必要 |
| ReviewState::Required(StaleHash) | 最新 final のハッシュと現在のハッシュが不一致。コード変更後の再レビューが必要 |
| ReviewState::NotRequired(Empty) | スコープに対象ファイルなし。レビュー不要 |
| ReviewState::NotRequired(ZeroFindings) | 最新 final が zero_findings かつハッシュ一致。approved |

## Acceptance Criteria
- [ ] 全スコープが NotRequired の場合のみ check-approved が成功する [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T003, T006]
- [ ] before/after hash 比較でレビュー中のファイル変更を検出し FileChangedDuringReview を返す [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T003]
- [ ] 1 つのスコープの変更が他のスコープの approval を無効化しない [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T003]
- [ ] .commit_hash 進行後、diff スコープがインクリメンタルに縮小する [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T004, T005]
- [ ] review.json v2 (schema_version: 2) がスコープ毎の rounds 履歴（findings 含む）を保持する [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T005]
- [ ] v1 の frozen scope 関連コード (CycleGroupState, has_scope_drift, ReviewPartitionSnapshot 等) が完全に削除される [source: knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md] [tasks: T007]
- [ ] cargo make ci が全て通過する（v1 テストの v2 移行を含む） [source: convention — .claude/rules/07-dev-environment.md] [tasks: T007]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 23  🟡 0  🔴 0

