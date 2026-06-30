---
name: track-type-design
description: Use when Codex is asked to author per-layer type catalogues via the type-designer capability (Phase 2). Translates spec.json and ADR design decisions into per-layer types.json entries.
---

# Track-Type-Design (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/type-design.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-type-design` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the type-designer capability writes
  `<layer>-types.json` files and rendered views to the working tree.

### (3) Sub-workflow and capability invocation

- The type catalogue authoring is delegated entirely to the `type-designer` capability via
  `.codex/agents/type-designer.toml`.
- Back-and-forth escalation (when a type-signal turns red) may re-invoke the `spec-designer`
  or `adr-editor` capability via their respective `.codex/agents/<name>.toml` agents.

### (4) Reporting format

- On successful completion, print: `TYPE_DESIGN_STATUS: completed — catalogues written, signals blue`
- On gate failure or block, print: `TYPE_DESIGN_STATUS: blocked — <signal>: <reason>`
