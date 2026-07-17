---
name: track-plan
description: Use when Codex is asked to plan a feature via the canonical track planning workflow — a state-machine orchestrator that drives Phase 0 → Phase 1 → Phase 2 → Phase 3.
---

# Track-Plan (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/plan.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-plan` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: each phase writes artifacts (metadata.json, spec.json,
  type catalogues, impl-plan.json) and rendered views to the working tree.

### (3) Sub-workflow and capability invocation

- Phase 0 is delegated to `$track-init`, then `$track-review`, then `$track-commit`; their
  transition rules and inputs are owned by the plan workflow SSoT.
- Phase 1 is delegated to `$track-spec-design` (which dispatches the `spec-designer`
  capability through `bin/sotp capability exec`, provider resolved from
  `.harness/config/agent-profiles.json`).
- Phase 2 is delegated to `$track-type-design` (which dispatches the `type-designer`
  capability the same way).
- Phase 3 is delegated to `$track-impl-plan` (which dispatches the `impl-planner`
  capability the same way).
- Back-and-forth escalation transitions (lifecycle branching, guardian judgment, retry
  counters) are owned by the plan workflow SSoT. When the SSoT selects a capability to dispatch
  (`adr-editor`, `adr-diagnoser`, or a phase writer), invoke it through
  `bin/sotp capability exec <capability> --host codex --briefing-file <path>` (invoke the
  matching `.codex/agents/<capability>.toml` in-host only on
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host`); upstream phase re-invocations go through the
  matching `$track-*` skill.

### (4) Reporting format

- On successful completion, print: `PLAN_STATUS: completed — phases 0-3 done, impl-plan.json ready`
- On gate failure or block, print: `PLAN_STATUS: blocked — phase <n>: <reason>`
