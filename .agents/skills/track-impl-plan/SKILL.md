---
name: track-impl-plan
description: Use when Codex is asked to author the track's impl-plan.json, task-coverage.json, task-contract.json, and batch-plan.json via the impl-planner capability (Phase 3).
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
  `task-coverage.json`, `task-contract.json`, and `batch-plan.json` (it is the sole writer of
  all four) to the working tree. `plan.md` is a derived read-only view this capability must
  not write.

### (3) Sub-workflow and capability invocation

- Write the configured briefing, then run `bin/sotp phase enter impl-plan`. Phase entry runs
  its declared convergence checks and launches the configured writer only after they pass. Do
  not launch the writer from this skill.
- This skill is single-shot per the workflow SSoT: on a task-coverage gate ERROR, report the
  gate verdict and error details back to the caller (`$track-plan`), which owns re-invocation
  and the `max_retry` counter. Do not re-dispatch `impl-planner` from inside this skill.

### (4) Reporting format

- On successful completion, print: `IMPL_PLAN_STATUS: completed — impl-plan.json written, coverage and batch-plan gates passed`
- On gate failure or block (either gate), print: `IMPL_PLAN_STATUS: blocked — <reason>`
