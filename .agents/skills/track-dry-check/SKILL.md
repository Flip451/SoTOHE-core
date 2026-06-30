---
name: track-dry-check
description: Use when Codex is asked to run the DRY fix phase (DFP) for the current track — drives sotp dry write → fix DRY violations → sotp dry check-approved loop until the DRY gate passes.
---

# Track-Dry-Check (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/dry-check.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-dry-check` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the dry-fix-lead capability writes source refactors
  to eliminate DRY violations.
- Do not run `git add` / `git commit` / `git push` directly.

### (3) Sub-workflow and capability invocation

- The DRY fix loop is delegated to the `dry-fix-lead` capability via
  `$dry-fix-lead` (`.codex/agents/dry-fix-lead.toml`).
- DRY gate verification uses `bin/sotp dry check-approved --track-id <id>`.

### (4) Reporting format

- On successful completion, print: `DRY_CHECK_STATUS: completed — DRY gate APPROVED`
- On failure or block, print: `DRY_CHECK_STATUS: blocked — <n> unresolved pairs: <reason>`
