---
name: track-implement
description: Use when Codex is asked to run the canonical implementation workflow for the current track.
---

# Track-Implement (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/implement.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-implement` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the implementation workflow writes source and test files
  to the working tree.
- Do not run `git add` / `git commit` / `git push` directly. Hand the implementation back to the
  enclosing `$track-full-cycle` lifecycle; do not hand off straight to `$track-commit`.

### (3) Sub-workflow and capability invocation

Implementation work is delegated through the Codex capability dispatcher:
`bin/sotp capability exec implementer --briefing-file <path>`. The dispatcher resolves the
provider from `.harness/config/agent-profiles.json` and runs the provider subprocess. The
workflow SSoT owns context intake, task ordering/state, obligation and CI gates, and recovery;
do not bypass that contract or launch an implementer directly.

### (4) Reporting format

- On successful completion, print: `IMPLEMENT_STATUS: completed — <n> tasks implemented, CI passing` (implementation handoff only; the orchestrator owns any task-state transition per the workflow SSoT)
- On failure or block, print: `IMPLEMENT_STATUS: blocked — task <id>: <reason>`
