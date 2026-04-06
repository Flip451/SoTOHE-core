<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "2.0.0"
signals: { blue: 23, yellow: 0, red: 0 }
---

# review hash スコープ再設計

## Goal

review hash を index 全体の tree hash から review-scope manifest hash に置き換え、auto-record の 3 つの構造的問題を解消する。
スコープポリシーを track/review-scope.json で設定ファイル駆動にし、プロジェクト構造に依存しない汎用設計にする。

## Scope

### In Scope
- track/review-scope.json スキーマ定義 + 初期設定ファイル — review_operational, planning_only, other_track, normalize セクション [source: ADR-2026-03-26-0000 §追加決定: 設定ファイル駆動ポリシー（track/review-scope.json）] [tasks: T001]
- ReviewScopePolicyConfig serde 型 + ReviewScopePolicy (glob パターン分類) を infrastructure 層に実装 [source: ADR-2026-03-26-0000 §決定, ADR-2026-03-26-0000 §追加決定: 設定ファイル駆動ポリシー（track/review-scope.json）] [tasks: T002, T003]
- review-scope manifest hash 計算 — git diff でパス収集 → worktree 読込 → 正規化 → SHA-256 → rvw1:sha256:<hex> [source: ADR-2026-03-26-0000 §決定] [tasks: T004, T005, T006]
- GitHasher trait を review_hash(ReviewHashInput) に更新 + normalized_hash 互換 shim [source: ADR-2026-03-26-0000 §影響] [tasks: T007]
- SystemGitHasher + RecordRoundProtocolImpl を新 hash アルゴリズムに移行（single-phase 化） [source: ADR-2026-03-26-0000 §決定] [tasks: T008, T009]
- CodeHash::Pending 使用停止 + StoredReviewHash による legacy hash 判別 [source: ADR-2026-03-26-0000 §決定] [tasks: T010]
- contract テスト: scope 分類、hash 安定性、legacy 検出 [source: ADR-2026-03-26-0000 §決定, convention — .claude/rules/05-testing.md] [tasks: T011, T012, T013]
- index_tree_hash_normalizing + record_round_with_pending の production path からの除去 [source: ADR-2026-03-26-0000 §決定] [tasks: T014]

### Out of Scope
- stale hash によるレビュー無効化の挙動変更（既存の正しいセキュリティ動作） [source: ADR-2026-03-24-1200]
- ReviewReader/ReviewWriter ポート分離（review-port-separation トラックのスコープ） [source: ADR-2026-03-25-2125]
- verdict 改ざん防止（tamper-proof-review トラックのスコープ、このトラック完了後に unblock） [source: ADR-2026-03-26-0000 §追加決定: トラック依存順序]

## Constraints
- 新規ロジックは Rust で実装する（Python 不可） [source: convention — .claude/rules/04-coding-principles.md]
- TDD ワークフローに従う [source: convention — .claude/rules/05-testing.md]
- ヘキサゴナルアーキテクチャ遵守 — domain 層は純粋、SHA-256 計算は infrastructure 層 [source: convention — project-docs/conventions/hexagonal-architecture.md]
- domain の ReviewState::record_round / check_commit_ready は hash 文字列比較のみ — hash 計算ロジックに依存しない [source: ADR-2026-03-26-0000 §影響]
- stored hash format は rvw1: prefix で version 判別可能にする [source: ADR-2026-03-26-0000 §決定]

## Domain States

| State | Description |
|-------|-------------|
| ReviewPathClass | TrackContent | Implementation | ReviewOperational | PlanningOnly | OtherTrack — パス分類 |
| StoredReviewHash | Legacy(String) | ReviewScopeV1(String) — stored hash の version 判別 |
| ScopeEntryState | File { sha256 } | Deleted — manifest 内の各ファイルの状態 |

## Acceptance Criteria
- [ ] track/review-scope.json が存在しない場合、review_hash が明示的エラーで失敗する（サイレント fallback 不可、fail-closed） [source: ADR-2026-03-26-0000 §追加決定: 設定ファイル駆動ポリシー（track/review-scope.json）] [tasks: T002, T012]
- [ ] 未ステージの metadata.json に対して review_hash が成功する（worktree 直接読込） [source: ADR-2026-03-26-0000 §背景] [tasks: T004, T012]
- [ ] review.json の変更が review hash を変化させない [source: ADR-2026-03-26-0000 §背景] [tasks: T003, T012]
- [ ] 他トラックや planning-only ファイルの変更が review hash を変化させない [source: ADR-2026-03-26-0000 §背景] [tasks: T003, T012]
- [ ] 実装ファイル (libs/**, apps/**) の変更が review hash を変化させる [source: ADR-2026-03-26-0000 §決定] [tasks: T003, T012]
- [ ] track/review-scope.json のパターン変更でスコープ分類が切り替わる [source: ADR-2026-03-26-0000 §追加決定: 設定ファイル駆動ポリシー（track/review-scope.json）] [tasks: T001, T002, T011]
- [ ] track/review-scope.json 自体が review hash scope に含まれ、ポリシー変更が旧承認を無効化する [source: ADR-2026-03-26-0000 §追加決定: 設定ファイル駆動ポリシー（track/review-scope.json）] [tasks: T003, T012]
- [ ] legacy hash (rvw1: prefix なし) が check_commit_ready で明確な migration error を返す [source: ADR-2026-03-26-0000 §決定] [tasks: T010, T013]
- [ ] cargo make ci が通過する [source: convention — .claude/rules/07-dev-environment.md] [tasks: T014]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/security.md

## Signal Summary

### Stage 1: Spec Signals
🔵 23  🟡 0  🔴 0

