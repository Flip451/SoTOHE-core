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

- Implementation work is delegated to `$track-implement` (which owns task-state transitions,
  the test-obligation gate, and CI validation per the implement workflow SSoT, and dispatches
  the `implementer` capability with profile-resolved provider routing internally). Do not
  dispatch the `implementer` capability directly from this skill.
- The DRY fix phase is delegated to `$track-dry-check` (which owns the opt-out pre-check and
  terminal-state verification per the dry-check workflow SSoT, and routes `dry-fix-lead`
  through its provider-resolving wrapper internally). Do not invoke the wrapper or
  `$dry-fix-lead` directly from this skill.
- The review loop is delegated to `$track-review`.
- Commit creation is delegated to `$track-commit`.

### (4) Reporting format

- On successful completion, print: `FULL_CYCLE_STATUS: completed — <n> tasks done, committed <short-hash>`
- On failure or block, print: `FULL_CYCLE_STATUS: blocked — <phase>: <reason>`
