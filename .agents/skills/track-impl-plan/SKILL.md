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

Prepare the configured briefing and enter the phase through
`bin/sotp phase enter impl-plan`. Do not launch the writer directly. The workflow SSoT owns
context intake, convergence, binary gates, and recovery.

### (4) Reporting format

- On successful completion, print: `IMPL_PLAN_STATUS: completed — impl-plan.json written, coverage and batch-plan gates passed`
- On gate failure or block (either gate), print: `IMPL_PLAN_STATUS: blocked — <reason>`
