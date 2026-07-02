---
name: track-init
description: Use when Codex is asked to initialize a new track directory and its branch (Phase 0). Creates metadata.json and its rendered views, and materializes the branch from the configured base branch.
---

# Track-Init (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/init.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-init` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow creates directories, writes
  `metadata.json`, rendered views, and creates a git branch.
- Branch creation uses `cargo make track-branch-create`. Phase 0 produces no commit;
  commits are deferred to the `/track:commit` (or `$track-commit`) adapter after review.

### (3) Sub-workflow and capability invocation

- No capabilities are delegated from this workflow; it is a standalone Phase 0 initializer.

### (4) Reporting format

- On successful completion, print: `INIT_STATUS: completed — track <id> initialized on branch track/<id>`
- On failure or block, print: `INIT_STATUS: blocked — <reason>`
