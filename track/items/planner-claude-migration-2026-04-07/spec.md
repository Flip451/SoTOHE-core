<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "0.1.0"
signals: { blue: 16, yellow: 0, red: 0 }
---

# Planner capability の Claude 移行 (Phase 1)

## Goal

default profile の planner capability を Codex CLI (gpt-5.4) から Claude (--bare -p, Opus) に移行し、
Claude による設計レビューを即座に利用可能にする。
Phase 2 の hexagonal 統一 resolver 実装に向けた設計資料を保存する。

## Scope

### In Scope
- agent-profiles.json default profile の planner provider 変更 (codex → claude) [source: feedback — ユーザー承認 (2026-04-06 planning session)] [tasks: T001]
- providers.claude に default_model フィールド追加 [source: knowledge/research/2026-04-06-1257-claude-code-reviewer-capability.md] [tasks: T001]
- SKILL.md Phase 1.5/2 の planner 呼び出しパターンを Claude 経路に更新 [source: .claude/skills/track-plan/SKILL.md §Phase 1.5, §Phase 2] [tasks: T002]
- rules/02, 08, 11 および track/workflow.md, knowledge/DESIGN.md の planner 参照更新 [source: .claude/rules/08-orchestration.md §Delegation Rules] [tasks: T003, T004, T005, T006, T007]
- Codex planner 設計レビュー出力の保存と Phase 2 TODO 追記 [source: feedback — ユーザー承認 (2026-04-06 planning session)] [tasks: T008, T009]

### Out of Scope
- Rust コード変更（domain型、usecase port、infrastructure adapter） [source: feedback — Phase 2 で別 track として実施 (2026-04-06)]
- agent-profiles.json の config/ 配下への移動 [source: feedback — Phase 2 で統一 resolver と合わせて実施]
- codex-heavy / claude-heavy profile の変更 [source: feedback — ユーザー確認: そのまま維持]
- track-local-plan Rust CLI ラッパーの変更 [source: feedback — codex-heavy profile で引き続き使用]

## Constraints
- codex-heavy profile は Codex planner を維持すること [source: feedback — ユーザー確認 (2026-04-06)]
- Rust コード変更なし（doc/config のみ） [source: feedback — Phase 1 scope (2026-04-06)]
- 既存の cargo make ci が通ること [source: track/workflow.md §Quality Gates]

## Acceptance Criteria
- [ ] agent-profiles.json default profile の planner が claude になっている [source: feedback — ユーザー承認] [tasks: T001]
- [ ] SKILL.md Phase 1.5 の planner 呼び出しが Claude 経路を示している [source: .claude/skills/track-plan/SKILL.md §Phase 1.5] [tasks: T002]
- [ ] rules/02, 08 の default specialist profile 記述が Claude planner を反映している [source: .claude/rules/08-orchestration.md] [tasks: T003, T004]
- [ ] cargo make ci が通る [source: track/workflow.md §Quality Gates] [tasks: T001, T002, T003, T004, T005, T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 16  🟡 0  🔴 0

