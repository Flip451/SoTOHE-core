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

- Write the configured briefing, then run `bin/sotp phase enter spec-design`. Phase entry runs
  its declared convergence checks and launches the configured writer only after they pass. Do
  not launch the writer from this skill.
- This skill is single-shot per the workflow SSoT: when the spec → ADR signal turns red,
  surface the failing element ids and cited ADR paths back to the caller (`$track-plan`), which
  owns the back-and-forth escalation (adr-editor dispatch, retry counters). Do not dispatch
  `adr-editor` from inside this skill.

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

- On successful completion, print: `SPEC_DESIGN_STATUS: completed — spec.json written, signal blue`
- On gate failure or block, print: `SPEC_DESIGN_STATUS: blocked — <signal>: <reason>`
