---
name: track-impl-plan
description: Use when Codex is asked to author the track's impl-plan.json and task-coverage.json via the impl-planner capability (Phase 3).
---

# Track-Impl-Plan (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/impl-plan.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-impl-plan` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the impl-planner capability writes `impl-plan.json`,
  `task-coverage.json`, and renders `plan.md` to the working tree.

### (3) Sub-workflow and capability invocation

- The implementation plan authoring is delegated entirely to the `impl-planner` capability via
  `.codex/agents/impl-planner.toml`.
- Back-and-forth escalation (when the task-coverage gate fails) re-invokes the `impl-planner`
  capability for iteration.

### (4) Reporting format

- On successful completion, print: `IMPL_PLAN_STATUS: completed — impl-plan.json written, coverage gate passed`
- On gate failure or block, print: `IMPL_PLAN_STATUS: blocked — <reason>`
