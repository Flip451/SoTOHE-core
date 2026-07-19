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
- **Phase 0**: invoke `/track:init`, `/track:review`, then `/track:commit`; their transition
  rules and inputs are owned by the plan workflow SSoT.
- **Phase writer dispatch** — write the phase briefing, then invoke the matching
  `bin/sotp capability exec <capability> --host claude --briefing-file <path>` command. The
  dispatcher resolves the capability's provider internally from
  `.harness/config/agent-profiles.json` and returns one of two outcomes:
  - `CAPABILITY_EXEC_OUTCOME: executed` — the subprocess dispatch already ran. Exit code 0
    only proves the provider exited cleanly; **parse the capability's terminal status /
    gate verdict from its output** (e.g. `IMPL_PLAN_STATUS: completed` + task-coverage
    gate OK, per-phase writer completion contract). Advance to the next phase ONLY on the
    capability's explicit success verdict; on `blocked` / `failed` or a red signal, run the
    corresponding back-and-forth loop instead.
  - `CAPABILITY_EXEC_OUTCOME: delegate-in-host` — an in-host delegation instruction with
    `capability`, `briefing_file`, and `discipline` fields. **You MUST then invoke the
    matching Claude Agent tool** with `subagent_type: "<capability>"` and pass the briefing
    path + discipline body as its task prompt; do NOT proceed to the next phase without
    that Agent invocation, otherwise the phase artifact is never written.

| Phase | Capability | Briefing path |
|---|---|---|
| 1 | spec-designer | `tmp/spec-designer-briefing.md` |
| 2 | type-designer | `tmp/type-designer-briefing.md` |
| 3 | impl-planner | `tmp/impl-planner-briefing.md` |
| B&F | adr-editor / adr-diagnoser | capability-specific briefing path |

- **Semantic review check**: `bin/sotp ref-verify run`

## Report format

On completion, present:

1. Per-phase gate results (🔵🟡🔴 / OK / ERROR) and final `max_retry` counters per loop.
2. Generated track artifact paths (`metadata.json` / `spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json`).
3. Back-and-forth edits that occurred (target artifact and its writer).
4. ADR working-tree diff against HEAD (if any) — expected escalation diffs and track-born drafts are reported as carried to the PR merge audit (no synchronous termination decision is requested).
5. Suggested next commands: standard lane (`/track:implement` → `/track:review` → `/track:commit`, or `/track:full-cycle`) or planning-review-first (`/track:review` → `/track:commit`).
