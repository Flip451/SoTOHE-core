---
description: Run the autonomous implementation full-cycle for a track task.
---

Canonical wrapper for autonomous implementation in this template.

Arguments:
- Use `$ARGUMENTS` as the task summary.
- If empty, ask for a short task summary and stop.

Execution:
- Resolve the current track: if the current git branch matches `track/<id>`, use that track. Otherwise, fall back to the latest active track by `updated_at`.
- Read the current track's `spec.md`, `plan.md`, `metadata.json`, and `verification.md` before implementation.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `plan.md` before writing code.
- Map `$ARGUMENTS` to one or more approved tasks in `metadata.json`.
- Use `cargo make track-transition <track_dir> <task_id> in_progress` to mark selected tasks as `in_progress` and auto-render `plan.md` + `registry.md`. Do NOT edit `plan.md` directly.
- Execute the implementation autonomously inside Claude Code using Agent Teams, focused tests, and repo-local commands. Prefer Rust CLI / `cargo make` wrappers over ad-hoc workflow scripts.
- If `$ARGUMENTS` matches `docs/external-guides.json` `trigger_keywords`, rely on the injected guide summaries before opening cached raw documents.
- Before completion, run the equivalent of `/track:review` until findings reach zero, then run `cargo make ci`.
- Update `verification.md` with the work performed, focused/manual verification results, open issues, and `verified_at`.
- Use `cargo make track-transition <track_dir> <task_id> done --commit-hash <hash>` to mark completed tasks as `done`. If work remains blocked, keep tasks in `in_progress` and report the blocker.

Behavior:
- This is the canonical autonomous implementation path for `/track:*`.
- It replaces legacy `takt`-driven full-cycle execution.
- After execution, summarize:
  1. Result (success/failure)
  2. Key outputs or blockers
  3. Next recommended action
