---
name: track-adr2pr
description: Use when Codex is asked to drive a prepared ADR all the way to a reviewed PR (init → review → commit → plan phases → review → commit → full-cycle → pr-review), autonomously without merging.
---

# Track-Adr2pr (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/adr2pr.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-adr2pr` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow orchestrates commits, PR creation, and
  file writes across multiple sub-workflows.
- Do not run `git push` under any circumstance. PR operations are handled via `bin/sotp pr` wrappers.

### (3) Sub-workflow and capability invocation

- Sub-workflows are invoked by their Codex skill name (e.g. `$track-init`, `$track-review`, etc.).
- Capabilities (spec-designer, type-designer, impl-planner, adr-editor, review-fix-lead) are
  invoked via their `.codex/agents/<name>.toml` agent definitions.

### (4) Reporting format

- On successful completion (only when the final `$track-pr-review` step reaches its
  zero-findings terminal state per `.harness/workflows/track/adr2pr.md`), print:
  `ADR2PR_STATUS: completed — PR <url> reviewed, no merge performed`
- On failure or block, print: `ADR2PR_STATUS: blocked — <reason>`
