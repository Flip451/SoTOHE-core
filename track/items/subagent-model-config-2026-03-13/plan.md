<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# サブエージェントのデフォルトモデルを Sonnet に変更

settings.json の CLAUDE_CODE_SUBAGENT_MODEL を claude-sonnet-4-6 に変更する。
サブエージェントモデルのオーバーライドガイダンスを .claude/rules/ に追加する。
track-plan/SKILL.md の古いハードコードモデル参照を修正する。

## Phase 1: Settings change + rule

- [x] Change CLAUDE_CODE_SUBAGENT_MODEL in .claude/settings.json from claude-opus-4-6 to claude-sonnet-4-6
- [x] Create .claude/rules/11-subagent-model.md — guidance for when to override default (opus for complex Rust implementation, haiku not recommended)

## Phase 2: Skill fix + CI

- [x] Fix hardcoded model references: replace gpt-5.3-codex in track-plan/SKILL.md and codex-system/SKILL.md, and default_model-only lookup in .claude/commands/track/review.md and codex-system/SKILL.md (Execution Tips), with {model} resolved via provider_model_overrides > default_model as defined in 02-codex-delegation.md
- [x] Add verify_orchestra_guardrails.py: (1) assert CLAUDE_CODE_SUBAGENT_MODEL is in allowlist [claude-sonnet-4-6, claude-opus-4-6, claude-haiku-4-5-20251001], (2) assert no hardcoded Codex model literals (regex: gpt-\d+) anywhere in files under .claude/skills/ or .claude/commands/ (full file scan, not limited to --model flags), (3) for codex-system/SKILL.md, track-plan/SKILL.md, and .claude/commands/track/review.md: positive check that override-first resolution is referenced AND negative check that default_model-only instructions are absent. Add matching pass/fail tests in test_verify_scripts.py (extend fixture). Then run cargo make ci
