---
description: Run the autonomous implementation full-cycle for a track task.
---

Autonomous implementation wrapper using Claude Code orchestration.

Arguments:
- Use `$ARGUMENTS` as the task summary.
- If empty, ask for a short task summary and stop.

Execution:
- Resolve the current track in this order:
  1. If the current git branch matches `track/<id>`, use that track.
  2. Otherwise, use the latest materialized active track (non-archived, non-done, `branch != null`).
  3. If no materialized active track exists, fall back to the latest branchless planning-only track (`status=planned`, `branch=null`).
- Read the current track's `spec.md`, `plan.md`, `metadata.json`, and `verification.md` before implementation.
- If the resolved track is branchless planning-only (`status=planned`, `branch=null`), stop and return `/track:activate <track-id>` as the next command. Do not use this command to bypass activation.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `plan.md` before writing code.
- Map `$ARGUMENTS` to one or more approved tasks in `metadata.json`.
- Use `cargo make track-transition <track_dir> <task_id> in_progress` to mark selected tasks as `in_progress` and auto-render `plan.md` + `registry.md`. Do NOT edit `plan.md` directly.
- Execute the implementation autonomously inside Claude Code using Agent Teams, focused tests, and repo-local commands. Prefer Rust CLI / `cargo make` wrappers over ad-hoc workflow scripts.
- If `$ARGUMENTS` matches `knowledge/external/guides.json` `trigger_keywords`, rely on the injected guide summaries before opening cached raw documents.
- Before completion, run the equivalent of `/track:review` until findings reach zero, then run `cargo make ci`.
- Update `verification.md` with the work performed, focused/manual verification results, open issues, and `verified_at`.
- Use `cargo make track-transition <track_dir> <task_id> done --commit-hash <hash>` to mark completed tasks as `done`. If work remains blocked, keep tasks in `in_progress` and report the blocker.

Behavior:
- This command is transitional compatibility only. Prefer `/track:implement` for the primary implementation lane.
- It must not add new workflow behavior beyond parity with the current track guardrails.
- While this command remains in the repo, it must obey the same activation guard as `/track:implement`.
- After execution, summarize:
  1. Result (success/failure)
  2. Key outputs or blockers
  3. Next recommended action
