---
name: track-spec-design
description: Use when Codex is asked to author the track's spec.json via the spec-designer capability (Phase 1). Translates the ADR into a behavioral contract.
---

# Track-Spec-Design (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/spec-design.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-spec-design` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the spec-designer capability writes `spec.json`
  and renders `spec.md` to the working tree.

### (3) Sub-workflow and capability invocation

- The spec.json authoring is delegated entirely to the `spec-designer` capability via
  `.codex/agents/spec-designer.toml`.
- Back-and-forth escalation (when the spec → ADR signal turns red) re-invokes the
  `adr-editor` capability via `.codex/agents/adr-editor.toml`.

### (4) Reporting format

- On successful completion, print: `SPEC_DESIGN_STATUS: completed — spec.json written, signal blue`
- On gate failure or block, print: `SPEC_DESIGN_STATUS: blocked — <signal>: <reason>`
