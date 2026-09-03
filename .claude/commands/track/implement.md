---
description: Run parallel interactive implementation for the current track.
---

> Operational SSoT: `.harness/workflows/track/implement.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:implement`. Use `$ARGUMENTS` as optional scope notes (target module, constraints, priority).

## Claude Code invocation constraints

- **Context intake**: follow the implement workflow SSoT's `Summary-first context intake`.
  Before selecting or dispatching tasks, use its CLI summaries as the primary context and treat
  the selected task briefing as primary for task details. Do not bulk-read `*-types.json`,
  review or binding JSON, full sub-workflow texts, or a `Related Conventions` list. Open only a
  targeted diff or the artifact body named by a blocker; the dispatcher supplies resolved
  convention paths with the implementer briefing for the delegated capability.
- **Parallel implementation**: use Agent Teams (multiple subagents with `run_in_background: true`) for independent tasks. Serialize `cargo add` / `cargo update` / `Cargo.lock`-changing steps through a single worker.
- **Task state transitions**: the calling orchestrator, never the `implementer` capability, performs them; do NOT edit `plan.md` directly (read-only view).
- **Test validation per worker**: `cargo make test`; reserve full-suite commands for single workers to avoid `target/` build lock contention.
- **CI gate before reporting**: `cargo make ci`
- **Completion timing**: owned by the workflow SSoT (`.harness/workflows/track/implement.md` Step 7 and `full-cycle.md` Step 1d / Step 3) — this adapter does not restate the transition ordering.

## Report format

After execution, summarize:

1. Implemented scope.
2. Implementation handoff: implemented task IDs and their verification results (tasks are handed off `in_progress`; the orchestrator owns transitions per the workflow SSoT).
3. Remaining tasks.
4. Recommended next command: `/track:full-cycle` (it owns the DFP → transition → review → commit ordering defined in the workflow SSoT). Standalone `/track:review` or `/track:commit` straight after implementation is not sanctioned.
