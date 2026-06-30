---
name: track-full-cycle
description: Use when Codex is asked to run the feature-batch implement → DRY check → review → commit loop for the current track.
---

# Track-Full-Cycle (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/full-cycle.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-full-cycle` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow writes source code, runs CI, drives
  the DRY fix phase, and creates commits.
- Do not run `git add` / `git commit` / `git push` directly; use `cargo make` wrappers.

### (3) Sub-workflow and capability invocation

- Implementation work is delegated to the `implementer` capability per the routing in
  `.harness/config/agent-profiles.json` (default: Claude main-session / ad-hoc delegation
  per `.claude/agents/README.md`).
- The DRY fix phase is delegated to the `dry-fix-lead` capability via
  `$dry-fix-lead` (`.codex/agents/dry-fix-lead.toml`).
- The review loop is delegated to `$track-review`.
- Commit creation is delegated to `$track-commit`.

### (4) Reporting format

- On successful completion, print: `FULL_CYCLE_STATUS: completed — <n> tasks done, committed <short-hash>`
- On failure or block, print: `FULL_CYCLE_STATUS: blocked — <phase>: <reason>`
