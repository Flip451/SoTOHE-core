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
  `impl-plan.json` task states (orchestrator-owned transitions), and runs `cargo make ci` to
  verify correctness (matching `.harness/workflows/track/implement.md` Step 5).
- Do not run `git add` / `git commit` / `git push` directly. Hand the implementation back to the
  enclosing `$track-full-cycle` lifecycle, which owns the DFP → orchestrator `done` transition →
  review → commit sequence defined in the workflow SSoT. Do not hand off straight to
  `$track-commit`.

### (3) Sub-workflow and capability invocation

- Implementation work is always delegated to the `implementer` capability through
  `bin/sotp capability exec implementer --briefing-file <path>`; the dispatcher resolves the
  provider from `.harness/config/agent-profiles.json` and runs it as a separate process. The
  root session never implements a task inline and never loads the implementer skill itself;
  direct editing by the root session is recovery after a failed delegation only.
- Task state transitions are the orchestrator's, never the `implementer` capability's. Their
  command, sequencing, and timing live in the workflow SSoT — do not restate them here.
- CI verification uses `cargo make ci` (full gate, matching `.harness/workflows/track/implement.md` Step 5).

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

- On successful completion, print: `IMPLEMENT_STATUS: completed — <n> tasks implemented, CI passing` (implementation handoff only; the orchestrator owns any task-state transition per the workflow SSoT)
- On failure or block, print: `IMPLEMENT_STATUS: blocked — task <id>: <reason>`
