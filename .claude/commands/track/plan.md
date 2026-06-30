---
description: Plan a feature via the canonical track planning workflow (Phase 0-3 orchestrator).
---

> Operational SSoT: `.harness/workflows/track/plan.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:plan`. `$ARGUMENTS`:

- `<feature>`: feature name / slug for a new track
- `<integer>`: `max_retry` count (default 5; bare integer = `max_retry`)
- `<feature> <integer>`: both (space-separated)
- Empty: ask the user for a feature name and stop

## Claude Code invocation constraints

- **Progress tracking**: use `TaskCreate` to register Phase 0–3 steps + Termination as tasks.
- **Timestamps**: `date -u +"%Y-%m-%dT%H:%M:%SZ"` (ISO 8601 UTC) — manual input is forbidden.
- **Phase 0** (`/track:init`): run by reading `.claude/commands/track/init.md`.
- **Phase writer subagents** — provider routing from `.harness/config/agent-profiles.json`:

| Phase | Capability | Claude path | Codex path |
|---|---|---|---|
| 1 | spec-designer | Agent tool (`subagent_type: "spec-designer"`, `run_in_background: true`) | `bin/sotp plan codex-local --model {model} --briefing-file tmp/spec-designer-briefing.md` |
| 2 | type-designer | Agent tool (`subagent_type: "type-designer"`, `run_in_background: true`) | — |
| 3 | impl-planner | Agent tool (`subagent_type: "impl-planner"`, `run_in_background: true`) | `bin/sotp plan codex-local --model {model} --briefing-file tmp/impl-planner-briefing.md` |
| B&F | adr-editor | Agent tool (`subagent_type: "adr-editor"`, `run_in_background: true`) | — |

- **Semantic review check**: `bin/sotp ref-verify run`

## Report format

On completion, present:

1. Per-phase gate results (🔵🟡🔴 / OK / ERROR) and final `max_retry` counters per loop.
2. Generated track artifact paths (`metadata.json` / `spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json`).
3. Back-and-forth edits that occurred (target artifact and its writer).
4. ADR working-tree diff against HEAD (if any) and the user's termination decision.
5. Suggested next commands: standard lane (`/track:implement` → `/track:review` → `/track:commit`, or `/track:full-cycle`) or planning-review-first (`/track:review` → `/track:commit`).
