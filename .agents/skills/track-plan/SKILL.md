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

- Phase 0 is delegated to `$track-init`.
- Phase 1 is delegated to `$track-spec-design` (which internally uses the `spec-designer`
  capability via `.codex/agents/spec-designer.toml`).
- Phase 2 is delegated to `$track-type-design` (which internally uses the `type-designer`
  capability via `.codex/agents/type-designer.toml`).
- Phase 3 is delegated to `$track-impl-plan` (which internally uses the `impl-planner`
  capability via `.codex/agents/impl-planner.toml`).
- Back-and-forth escalation re-invokes the upstream phase skill when a downstream signal fails.

### (4) Reporting format

- On successful completion, print: `PLAN_STATUS: completed — phases 0-3 done, impl-plan.json ready`
- On gate failure or block, print: `PLAN_STATUS: blocked — phase <n>: <reason>`
