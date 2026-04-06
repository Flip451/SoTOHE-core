<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 21, yellow: 4, red: 0 }
---

# Review v2 運用文書アップデート + review skill 更新

## Goal

review-system-v2 マージ後に特定された運用文書の穴（RV2-10..15）を埋め、
review.md skill の v2 cleanup（RV2-04）と自律 fix+review ループ（RV2-02）を実装する。

## Scope

### In Scope
- review.md の v2 cleanup: 散在する RV2-04 制限ノートを集約、provider-support サブセクション化 [source: knowledge/strategy/TODO.md §L RV2-04] [tasks: T002]
- review.md にチャネル単位 fail-closed 契約を明記 [source: knowledge/strategy/TODO.md §L RV2-12] [tasks: T002]
- review.md + workflow.md に check-approved NotStarted bypass 仕様を文書化 [source: knowledge/strategy/TODO.md §L RV2-13] [tasks: T002, T004]
- pr-review.md に同一コミット再レビュー不可の注記追加 [source: knowledge/strategy/TODO.md §L RV2-10] [tasks: T003]
- pr-review.md に手動ポーリング禁止を明記 [source: knowledge/strategy/TODO.md §L RV2-11] [tasks: T003]
- workflow.md の v1 残存参照監査（RV2-15） [source: knowledge/strategy/TODO.md §L RV2-15] [tasks: T004]
- create_dir_all ガード無効化パターン convention 新規作成 [source: knowledge/strategy/TODO.md §L RV2-14] [tasks: T005]
- review-fix-lead エージェント定義 + review.md 自律ループ化 [source: knowledge/strategy/TODO.md §L RV2-02, feedback — feedback_subagent_review_loop.md] [tasks: T006]

### Out of Scope
- RV2-05..09（Rust コード修正が必要な穴）は別 track [source: knowledge/strategy/TODO.md §L]
- RV2-06 v2 escalation 再設計は別 track [source: knowledge/strategy/TODO.md §L RV2-06]
- claude-heavy profile の auto-record 実装 [source: inference — planner recommendation]
- RV2-03 ReviewState::Running variant 追加 [source: knowledge/strategy/TODO.md §L RV2-03]

## Constraints
- Rust コード変更なし — skill / agent 定義 / convention / workflow 文書のみ [source: inference — scope definition]
- claude-heavy 制限は事実として残す（auto-record 未実装） [source: inference — planner risk assessment]
- NotStarted bypass は実装通り狭く記述: 全 scope NotStarted + review.json 不在 [source: apps/cli/src/commands/review/mod.rs L346]
- RV2-02 は target behavior として明記（escalation 未実装のため） [source: knowledge/strategy/TODO.md §L RV2-06]
- review-fix-lead の無限ループ防止は Agent timeout（60 分/scope）で制御 [source: discussion]

## Domain States

| State | Description |
|-------|-------------|
| TODO.md uncommitted | RV2-05..15 が main 上で未コミット |
| docs updated | review.md / pr-review.md / workflow.md の穴が埋まった状態 |
| convention added | create_dir_all convention が knowledge/conventions/ に追加済み |
| agent defined | review-fix-lead agent が定義され review.md が自律ループ化された状態 |

## Acceptance Criteria
- [ ] review.md に RV2-04 制限ノートが 1 箇所のみ（Step 1 近辺）に存在する [source: knowledge/strategy/TODO.md §L RV2-04] [tasks: T002]
- [ ] review.md にチャネル単位 fail-closed 契約セクションが存在する [source: knowledge/strategy/TODO.md §L RV2-12] [tasks: T002]
- [ ] review.md + workflow.md に NotStarted bypass 仕様が記載されている [source: knowledge/strategy/TODO.md §L RV2-13] [tasks: T002, T004]
- [ ] pr-review.md に同一コミット再レビュー不可 + 手動ポーリング禁止が記載されている [source: knowledge/strategy/TODO.md §L RV2-10, RV2-11] [tasks: T003]
- [ ] knowledge/conventions/ に create_dir_all convention ファイルが存在する [source: knowledge/strategy/TODO.md §L RV2-14] [tasks: T005]
- [ ] .claude/agents/review-fix-lead.md が存在し、scope 所有 / 自律ループ / timeout / cross-scope fail-closed の契約が定義されている [source: knowledge/strategy/TODO.md §L RV2-02] [tasks: T006]
- [ ] review.md Step 2c/Step 3 が review-fix-lead エージェントを使用する形に更新されている [source: knowledge/strategy/TODO.md §L RV2-02] [tasks: T006]
- [ ] cargo make ci が通る [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md
- knowledge/conventions/impl-delegation-arch-guard.md
- knowledge/conventions/task-completion-flow.md

## Signal Summary

### Stage 1: Spec Signals
🔵 21  🟡 4  🔴 0

