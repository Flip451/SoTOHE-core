---
description: Author the track's impl-plan.json + task-coverage.json via the impl-planner subagent (Phase 3).
---

> Operational SSoT: `.harness/workflows/track/impl-plan.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:impl-plan`. No arguments.

## Claude Code invocation constraints

Provider routing from `.harness/config/agent-profiles.json` (`capabilities.impl-planner.provider`):

- **Claude (default)**: Agent tool (`subagent_type: "impl-planner"`, `run_in_background: true`). Briefing must include: track id, paths to `spec.json` and each `<layer>-types.json`, paths to related ADR(s) and conventions.
- **Codex**: `bin/sotp plan codex-local --model {model} --briefing-file tmp/impl-planner-briefing.md`

The subagent owns: writing `impl-plan.json` and `task-coverage.json`, and evaluating the task-coverage binary gate (OK / ERROR). No direct CLI calls from this adapter body (Claude path).

## Report format

Report: track id, `impl-plan.json` and `task-coverage.json` paths, task count, gate verdict (OK / ERROR).
