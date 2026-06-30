---
description: Run feature-batch implement → DRY check → review → commit loop for the current track (per-task split only when a per-scope diff ceiling is about to be exceeded).
---

> Operational SSoT: `.harness/workflows/track/full-cycle.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:full-cycle`. No arguments.

## Claude Code invocation constraints

This command sequences sub-commands by reading their `.claude/commands/track/*.md` definitions:

- `/track:implement` — parallel implementation via Agent Teams (`run_in_background: true`)
- `/track:dry-check` — DFP; dispatches `dry-fix-lead` per its own adapter rules
- `/track:review` — RFP; dispatches `review-fix-lead` per its own adapter rules
- `/track:commit` — guarded commit

Key tool interactions:

- Scope ceiling config: Read `.harness/config/review-scope.json`
- Diff measurement: `git diff --numstat <batch-base> --`, `git ls-files --others --exclude-standard` (read-only)
- Staging: `cargo make add-all`
- Task transitions: `bin/sotp track transition <task_id> done --commit-hash <hash>`

## Report format

After execution, summarize:

1. Batches executed (count and task IDs in each), with per-batch commit hash.
2. Tasks completed (count and IDs).
3. Tasks remaining (if stopped early).
4. Failure details (if any).
5. Recommended next command.
