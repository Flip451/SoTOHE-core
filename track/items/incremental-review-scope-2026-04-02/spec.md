<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 8, yellow: 21, red: 0 }
---

# Incremental review scope: approved_head で差分ベースをインクリメンタルにする

## Goal

check-approved と record-round が main からの累積 diff ではなく、前回承認済みコミット以降のインクリメンタル diff を使うように変更する。
これにより track branch 上で複数回コミットする際、前回コミット済みファイルの再レビューが不要になる。

## Scope

### In Scope
- domain 層に ApprovedHead newtype を追加（validated git SHA、40文字 hex 検証） [source: inference — Codex planner design + user feedback: newtype for clean implementation] [tasks: T001]
- ReviewCycle に approved_head: Option<ApprovedHead> フィールドを追加（getter/setter、None 初期化） [source: inference — Codex planner design decision: store per-cycle in review.json] [tasks: T001]
- review_json_codec の CycleDocument に approved_head を追加（required、null or SHA string） [source: inference — Codex planner design decision] [tasks: T002]
- infra 層に effective_diff_base ヘルパーを実装（approved_head 優先、無効時 base_ref フォールバック） [source: inference — Codex planner design decision: infra layer for git ref resolution] [tasks: T003]
- RecordRoundProtocolImpl::execute の diff scope 計算で effective_diff_base を使用 [source: inference — root cause fix for cumulative diff problem] [tasks: T004]
- check-approved snapshot 構築で effective_diff_base を使用 [source: inference — both code paths must use same diff base] [tasks: T005]
- track-commit-message で commit 成功後に HEAD SHA を approved_head として review.json に永続化 [source: inference — Codex planner design decision: auto-record in commit wrapper] [tasks: T006]
- sotp review set-approved-head リカバリコマンド（保存失敗時の再試行手段） [source: discussion — user feedback: recovery command needed for persistence failure] [tasks: T006]

### Out of Scope
- 旧 review.json の後方互換（新規トラックのみ使用） [source: feedback — Rust-first policy, feedback_no_legacy_migration.md]
- RVW-44: reset-cycle CLI コマンド（別トラック） [source: knowledge/strategy/TODO.md §RVW-44]
- RVW-47: review status コマンド（別トラック） [source: knowledge/strategy/TODO.md §RVW-47]
- レビュー呼び出しフロー自体の変更（スコープ計算のみ変更） [source: inference — minimal change surface]

## Constraints
- TDD ワークフロー必須（Red → Green → Refactor） [source: convention — .claude/rules/05-testing.md]
- 同期のみ（async なし） [source: track/tech-stack.md]
- ライブラリコードでパニック禁止 [source: convention — .claude/rules/04-coding-principles.md]
- レイヤー依存方向の厳守: domain → (なし), usecase → domain, infra → domain+usecase, cli → all [source: architecture-rules.json]
- ApprovedHead は domain 層の newtype（validated git SHA, 40文字 hex） [source: discussion — user preference for clean implementation over raw String]
- effective_diff_base ヘルパーは infra 層に配置（git ref 解決が infra の責務） [source: inference — Codex planner design decision]
- approved_head が無効（rebase 後等）の場合は base_ref にフォールバック（fail-closed: スコープが累積に拡大） [source: inference — Codex planner design decision: fail-closed fallback]
- commit 成功後の approved_head 保存失敗時: commit は巻き戻さずエラー報告のみ。リカバリは set-approved-head コマンドで対応 [source: discussion — user feedback: recovery command needed]

## Domain States

| State | Description |
|-------|-------------|
| NoApprovedHead | approved_head = None. First commit on track branch / after review.json reset. Diff base = base_ref (main) |
| ApprovedHeadSet | approved_head = Some(SHA). Subsequent commits use this as diff base for incremental scope |
| ApprovedHeadInvalid | approved_head SHA no longer exists (e.g. after rebase). Falls back to base_ref (fail-closed) |

## Acceptance Criteria
- [ ] ApprovedHead newtype が domain 層に存在し、40文字 hex を検証するコンストラクタを持つ [source: discussion — user preference] [tasks: T001]
- [ ] ReviewCycle が approved_head: Option<ApprovedHead> を保持し getter/setter がある [source: inference — Codex planner design] [tasks: T001]
- [ ] review.json の encode/decode で approved_head が round-trip する [source: inference — codec correctness] [tasks: T002]
- [ ] approved_head が有効な場合、record-round と check-approved が approved_head を diff base として使用する [source: inference — core requirement] [tasks: T004, T005]
- [ ] approved_head が無効な場合、base_ref にフォールバックする（fail-closed） [source: inference — Codex planner design] [tasks: T003]
- [ ] 回帰テスト: 承認コミット後に TODO.md のみ変更 → スコープが前回のコードファイルに再拡大しない [source: inference — the bug this track fixes] [tasks: T004]
- [ ] track-commit-message が commit 成功後に HEAD SHA を approved_head として review.json に記録する [source: inference — Codex planner design] [tasks: T006]
- [ ] sotp review set-approved-head コマンドが存在し、保存失敗時のリカバリに使用できる [source: discussion — user feedback] [tasks: T006]
- [ ] cargo make ci が通る [source: convention — track/workflow.md §Quality Gates] [tasks: T001, T002, T003, T004, T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 8  🟡 21  🔴 0

