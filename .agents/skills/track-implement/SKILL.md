---
name: track-implement
description: Use when Codex is asked to run parallel interactive implementation for the current track — reads the approved impl-plan, marks tasks in_progress, implements them, and verifies with CI.
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

- Requires `--sandbox workspace-write`: the workflow writes Rust source files, updates
  `metadata.json` task states, and runs `cargo make ci` to verify correctness (matching
  `.harness/workflows/track/implement.md` Step 4).
- Do not run `git add` / `git commit` / `git push` directly; the commit step follows separately
  via `$track-commit`.

### (3) Sub-workflow and capability invocation

- Implementation work is delegated to the `implementer` capability per the routing in
  `.harness/config/agent-profiles.json` (default: Claude main-session / ad-hoc delegation
  per `.claude/agents/README.md`).
- Task state transitions use `bin/sotp track transition <id> <state>`.
- CI verification uses `cargo make ci` (full gate, matching `.harness/workflows/track/implement.md` Step 4).

### (4) Reporting format

- On successful completion, print: `IMPLEMENT_STATUS: completed — <n> tasks done, CI passing`
- On failure or block, print: `IMPLEMENT_STATUS: blocked — task <id>: <reason>`
