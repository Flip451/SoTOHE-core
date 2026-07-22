---
name: track-done
description: Use when Codex is asked to return to the configured base branch after a track PR has merged and report completion.
---

# Track-Done (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/done.md` — the provider-agnostic workflow contract for this skill.
Do not duplicate step sequence, gate conditions, state transitions, or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-done` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.
- This skill accepts no arguments.

### (2) Tool constraints

- Requires `--sandbox workspace-write`: the workflow changes the checked-out branch and attempts a guarded fast-forward sync.
- Use `bin/sotp track switch-base` for the branch switch and sync.
- Use read-only inspection to surface the completion summary.
- Do not invoke `git switch`, `git checkout`, `git pull`, `git merge`, `git push`, `git add`, or `git commit` directly.

### (3) Reporting format

- On successful completion, print: `DONE_STATUS: completed — on <base>; <sync-result>`.
- On failure or block, print: `DONE_STATUS: blocked — <reason>`.
