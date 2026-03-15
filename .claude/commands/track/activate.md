---
description: Materialize a planning-only track and switch to its track branch.
---

Canonical command for moving a planning-only track into the implementation lane.

Arguments:
- Use `$ARGUMENTS` as the target track id.
- If empty, ask for the track id and stop.

Execution:
- Read `track/items/$ARGUMENTS/metadata.json`, `spec.md`, and `plan.md`.
- Confirm the target track is planning-only: `schema_version: 3`, `status: planned`, and `branch: null`.
- Confirm the worktree is clean before activation if materialization has not happened yet.
- Run:
  `cargo make track-activate '$ARGUMENTS'`
- Activation persists `metadata.json.branch`, syncs rendered views, and creates an activation commit before switching when a branch change is needed.
- If activation fails after metadata materialization but before branch switch completes, re-run the same command to resume.
- Do not implement code in this command.

Behavior:
- After execution, summarize:
  1. Target track id
  2. Whether branch materialization happened now or was already present
  3. Whether an activation commit was created
  4. Current git branch after the command
  5. Suggested next command (`/track:implement` → `/track:review` → `/track:commit`, or `/track:full-cycle <task>`)
