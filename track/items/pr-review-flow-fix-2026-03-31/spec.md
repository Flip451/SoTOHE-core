<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 13, yellow: 3, red: 0 }
---

# Phase D: PR review flow fix (WF-66/ERR-08)

## Goal

PR review フローにおける 2 つの問題を解消する:
1. WF-66: push() と review_cycle() の check_task_completion_guard が中間 push をブロックし、PR review を利用不能にする
2. ERR-08: review_cycle() のポーリングが中断されると trigger state が失われ、再開できない

## Scope

### In Scope
- check_task_completion_guard を push() と review_cycle() から削除し wait_and_merge_with() に移動 [source: knowledge/strategy/rvw-remediation-plan.md §Phase D T001] [tasks: T001]
- SKILL.md (pr-review.md, merge.md) の更新: merge のみがタスク完了を要求する旨を明記 [source: knowledge/strategy/rvw-remediation-plan.md §Phase D T002] [tasks: T002]
- review_cycle() の trigger state を tmp/pr-review-state/<track-id>.json に永続化 [source: knowledge/strategy/rvw-remediation-plan.md §Phase D T003] [tasks: T003]
- track-pr-review --resume サブコマンドの追加 [source: knowledge/strategy/rvw-remediation-plan.md §Phase D T004] [tasks: T004]
- ガード移動とresume機能のテスト [source: knowledge/strategy/rvw-remediation-plan.md §Phase D T005] [tasks: T005]

### Out of Scope
- 他の Phase (A/B/C/E/F/G) の修正 [source: knowledge/strategy/rvw-remediation-plan.md §Phase D]
- PR review の verdict 解析ロジック変更 [source: inference -- Phase D はフロー制御のみ]

## Constraints
- 変更は apps/cli/src/commands/pr.rs と SKILL docs のみ。domain/usecase/infrastructure 層への変更なし [source: inference -- Phase D はCLI層フロー制御]
- TDD: テストを先に書く [source: convention -- .claude/rules/05-testing.md]
- unwrap() は本番コード禁止 [source: convention -- .claude/rules/04-coding-principles.md]
- tmp/ ディレクトリは .gitignore 済み（state ファイルはコミットされない） [source: inference -- transient state は git 管理外]

## Domain States

| State | Description |
|-------|-------------|
| TriggerState | PR review の trigger 情報を保持する構造体: pr_number, comment_id, trigger_timestamp, head_hash, track_id |

## Acceptance Criteria
- [ ] 未完了タスクがある状態で cargo make track-pr-push が成功する [source: knowledge/strategy/TODO.md §WF-66] [tasks: T001, T005]
- [ ] 未完了タスクがある状態で cargo make track-pr-review が成功する [source: knowledge/strategy/TODO.md §WF-66] [tasks: T001, T005]
- [ ] cargo make track-pr-merge が未完了タスクでブロックする [source: knowledge/strategy/rvw-remediation-plan.md §Phase D T001] [tasks: T001, T005]
- [ ] review_cycle() 中断後に --resume で poll を再開できる [source: knowledge/strategy/TODO.md §ERR-08] [tasks: T003, T004]
- [ ] cargo make ci が通る [source: convention -- .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005]

## Related Conventions (Required Reading)
- project-docs/conventions/task-completion-flow.md

## Signal Summary

### Stage 1: Spec Signals
🔵 13  🟡 3  🔴 0

