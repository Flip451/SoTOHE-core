---
description: Run the declared-batch implement → DRY check → review → commit loop for the current track (batches consumed from batch-plan.json in declaration order).
---

> Operational SSoT: `.harness/workflows/track/full-cycle.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:full-cycle`. No arguments are required; a workflow caller
may pass the SSoT's `--single-batch` option when it needs a one-batch return.

## Claude Code invocation constraints

This command sequences sub-commands by reading their `.claude/commands/track/*.md` definitions:

- `/track:implement` — parallel implementation via Agent Teams (`run_in_background: true`)
- `/track:dry-check` — DFP; dispatches `dry-fix-lead` per its own adapter rules
- `/track:review` — RFP; dispatches `review-fix-lead` per its own adapter rules
- `/track:commit` — guarded commit

Key tool interactions:

- Batch declaration: Read `track/items/<id>/batch-plan.json` (read-only — impl-planner is its sole writer)
- Staging: `bin/sotp git add-all`
- Task transitions: run `bin/sotp track transition` from the orchestrator host. The command's sequencing, timing, and ownership boundary live in the workflow SSoT (`.harness/workflows/track/full-cycle.md` Step 1d / Step 3) — do not restate them here.

### Gate waiting

- Every long-running gate in the loop (implementer dispatch, DRY fix wrapper, review-fix wrapper,
  `cargo make ci`, `cargo make track-commit-message`) is run as one blocking call whose result is
  read once. Do not poll logs or re-run status probes; if the host backgrounds a call, read the
  result once after the single completion notification.
- `bin/sotp test-obligation evaluate` is only a synchronous step inside repair work on the
  orchestrator host; never launch it in the background or as a commit-gate prerequisite.

## Report format

After execution, summarize:

1. Batches executed (count and task IDs in each), with per-batch commit hash.
2. Tasks completed (count and IDs).
3. Tasks remaining (if stopped early).
4. Failure details (if any).
5. Recommended next command.
