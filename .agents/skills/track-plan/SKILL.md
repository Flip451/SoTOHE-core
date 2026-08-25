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
  transition rules and inputs are owned by the plan workflow SSoT. Phase 0 follows
  `.harness/policies/pre-track-adr-authoring.md` §In-track 意味変更の裁定権 as its sole
  normative source; this skill states no procedure of its own for that phase.
- Phase 1 is delegated to `$track-spec-design`, which enters `spec-design` through
  `bin/sotp phase enter spec-design` after preparing its configured briefing.
- Phase 2 is delegated to `$track-type-design`, which enters `type-design` through
  `bin/sotp phase enter type-design` after preparing its configured briefing.
- Phase 3 is delegated to `$track-impl-plan`, which enters `impl-plan` through
  `bin/sotp phase enter impl-plan` after preparing its configured briefing.
- Back-and-forth escalation transitions (lifecycle branching, guardian judgment, retry
  counters) are owned by the plan workflow SSoT. When the SSoT selects `adr-editor` or
  `adr-diagnoser`, invoke `bin/sotp capability exec adr-editor --host codex --briefing-file <path>`
  or `bin/sotp capability exec adr-diagnoser --host codex --briefing-file <path>`. Invoke the
  matching `.codex/agents/<capability>.toml` in-host only on
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host`; upstream phase re-invocations go through the
  matching `$track-*` skill and its phase-entry path.

### (4) Context intake

- Follow the workflow SSoT's summary-first context intake: take progress, review necessity,
  obligation state, and catalogue state from the CLI summaries it names (`bin/sotp track resolve`,
  `bin/sotp track task-counts`, `bin/sotp track next-task`, `bin/sotp review results`,
  `bin/sotp test-obligation results`, `bin/sotp catalog check`, `bin/sotp ref-verify results`).
- Do not bulk-read `*-types.json`, `review.json`, bindings JSON, full sub-workflow texts, or a
  `Related Conventions` list at intake; open an artifact body only for a targeted diff or the
  blocker it names. Convention paths are listed in each delegated briefing and read by the
  delegated capability, not by this root session.

### (5) Reporting format

- On successful completion, print: `PLAN_STATUS: completed — phases 0-3 done, impl-plan.json ready`
- On gate failure or block, print: `PLAN_STATUS: blocked — phase <n>: <reason>`
