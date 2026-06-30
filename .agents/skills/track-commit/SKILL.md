---
name: track-commit
description: Use when Codex is asked to create a guarded commit for the current track after review is complete, then attach a git note.
---

# Track-Commit (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/commit.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-commit` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow stages files and creates a commit.
- Staging is done via `cargo make track-add-paths` (writing paths to
  `tmp/track-commit/add-paths.txt`) or `cargo make add-all`.
- Commit creation uses `cargo make track-commit-message` exclusively.
- Do not run `git add` / `git commit` / `git push` directly.

### (3) Sub-workflow and capability invocation

- No capabilities are delegated; commit is a standalone terminal workflow step.

### (4) Reporting format

- On successful completion, print: `COMMIT_STATUS: completed — <short-hash> <subject>`
- On failure or block, print: `COMMIT_STATUS: blocked — <reason>`
