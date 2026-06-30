---
description: Run parallel interactive implementation for the current track.
---

> Operational SSoT: `.harness/workflows/track/implement.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:implement`. Use `$ARGUMENTS` as optional scope notes (target module, constraints, priority).

## Claude Code invocation constraints

- **Parallel implementation**: use Agent Teams (multiple subagents with `run_in_background: true`) for independent tasks. Serialize `cargo add` / `cargo update` / `Cargo.lock`-changing steps through a single worker.
- **Task state transitions**: `bin/sotp track transition <task_id> in_progress` / `done` — do NOT edit `plan.md` directly (read-only view).
- **Test validation per worker**: `cargo make test`; reserve full-suite commands for single workers to avoid `target/` build lock contention.
- **CI gate before reporting**: `cargo make ci`
- **Commit hash recording**: after `/track:commit`, `bin/sotp track transition <task_id> done --commit-hash <hash>`

## Report format

After execution, summarize:

1. Implemented scope.
2. Updated `metadata.json` task states (todo → in_progress → done, or blocked in_progress).
3. Remaining tasks.
4. Recommended next command (`/track:review`, `/track:commit <message>`, or `/track:full-cycle`).
