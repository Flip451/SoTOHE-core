---
name: track-plan
description: Use when Codex is asked to plan a feature via the canonical track planning workflow.
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

- Requires `--sandbox workspace-write`: the planning workflow writes its declared artifacts and
  rendered views to the working tree.

### (3) Codex invocation mapping

The shared workflow SSoT owns context intake, phase ordering, task state, gates, retry and
recovery behavior. This adapter supplies only the Codex skill names and capability dispatch form:

| Workflow surface | Codex adapter |
|---|---|
| `init`, `review`, `commit` | `$track-init`, `$track-review`, `$track-commit` |
| `spec-design` | `$track-spec-design` |
| `type-design` | `$track-type-design` |
| `impl-plan` | `$track-impl-plan` |

When the workflow selects `adr-editor` or `adr-diagnoser`, invoke the corresponding
`bin/sotp capability exec adr-editor --briefing-file <path>` or
`bin/sotp capability exec adr-diagnoser --briefing-file <path>` with `--host` omitted. The
dispatcher resolves the configured provider and runs its subprocess; do not force the Codex
root's provider or hand-assemble a provider command. Use the matching `$track-*` adapter when the
workflow requests a phase re-entry.

### (4) Reporting format

- On successful completion, print: `PLAN_STATUS: completed — phases 0-3 done, impl-plan.json ready`
- On gate failure or block, print: `PLAN_STATUS: blocked — phase <n>: <reason>`
