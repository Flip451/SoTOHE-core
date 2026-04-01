---
description: Stage review is complete and create a guarded commit, then attach a git note.
---

Canonical command for final commit in the track workflow.

Arguments:

- Use `$ARGUMENTS` as the commit message.
- On a non-track branch, guarded commit execution is only supported via an explicit planning-only selector: `<track-id> -- <commit message>`. Materialized tracks must be committed from their `track/<id>` branch.
- If empty, do not commit immediately. Inspect the current track (branch-bound if on a `track/<id>` branch, otherwise latest by timestamp), current diff, and recent task context, then propose 2-3 concrete commit message candidates and ask the user to choose one or edit one.

Execution:

## Step 0: Fill missing commit message when omitted

If `$ARGUMENTS` is empty:

1. Resolve the current track: if the current git branch matches `track/<id>`, use that track. Otherwise, fall back to the latest active track by `updated_at`.
   - On a non-track branch, do not auto-detect a branchless planning-only track.
   - For actual commit execution from a non-track branch, require an explicit track-id selector and use it only for the planning-only lane.
2. Inspect the current track under `track/items/` when present.
3. Read available context from `spec.md`, `plan.md`, `verification.md`, and current changed files.
4. Propose 2-3 commit message candidates that follow the repository's current style and reflect the actual change scope.
5. Stop after presenting the candidates. Do not run `cargo make track-commit-message` yet.

## Step 1: Pre-commit review

Before committing:

1. Run `git diff --cached --stat` and verify the staged scope matches the intended commit.
2. If the staged diff is empty or includes unrelated changes, stop and fix staging before continuing.
3. Resolve the current track in this order:
   - current `track/<id>` branch
   - explicit planning-only `<track-id>` selector on a non-track branch
   - otherwise latest materialized active track (non-archived, non-done, `branch != null`) for read-only context only
   - do not execute a guarded commit from a non-track branch without the explicit planning-only selector
4. If the current track exists, read `track/registry.md` and confirm it reflects the current track status, next command, and completion state.
5. When updating `track/registry.md`, use `sync_rendered_views()` to regenerate from metadata.json. Registry rendering is deterministic from metadata alone — do not pass branch context.
6. If `track/registry.md` is stale for the current track, update and stage it as part of this same commit (pre-commit step, not post-commit):
   - If the track status is `"archived"`: the track should already be in the `Archived Tracks` section. Verify registry reflects this. No Active/Completed table changes needed.
   - If this is a **completion commit** — all tasks in `metadata.json` are resolved (`"status": "done"` or `"status": "skipped"`) (metadata.json is SSoT) — take these actions:
     1. Remove the track row from the `Active Tracks` table.
     2. Add a new row to `Completed Tracks` with columns `Track | Result | Updated` (e.g., `Done | YYYY-MM-DD`).
     3. In the `Current Focus` section, set `Latest active track` to the next remaining active track, or `None yet` if none remain. Set `Next recommended command` and `Last updated` accordingly.
   - Otherwise (mid-track commit): update the `Status`, `Next`, and `Updated` columns in-place under `Active Tracks`, and update `Last updated` in `Current Focus`.
   - After editing `track/registry.md`, write its path to `tmp/track-commit/add-paths.txt`, then run:

     ```bash
     cargo make track-add-paths
     ```

7. If the selected track is branchless planning-only (`status=planned`, `branch=null`), enforce the planning-only allowlist before committing:
   - `track/items/<id>/`
   - `track/registry.md`
   - `track/tech-stack.md`
   - `knowledge/DESIGN.md`
   If staged changes exceed that allowlist, stop and require `/track:activate <track-id>` first.

## Step 2: Guarded commit

Write the chosen commit message to `tmp/track-commit/commit-message.txt`.
If this is a planning-only commit from a non-track branch, also write `track/items/<track-id>` to
`tmp/track-commit/track-dir.txt` so the guarded commit path uses explicit track context.
If you are on a non-track branch without that selector, stop instead of running the wrapper.
Then run:

```bash
cargo make track-commit-message
```

This exact wrapper performs `ci + git commit -F tmp/track-commit/commit-message.txt` and deletes the scratch file on success.
If the commit fails, report the error and stop. Do not proceed to note generation.

## Step 3: Attach git note

After a successful commit, attach a structured git note to HEAD.

### 3a. Use generated scratch note (normal path)

Generate the note from current track context unless the user explicitly provided some other
traceability source:

1. Find the current track under `track/items/` (if any exist).
2. Read `spec.md`, `plan.md`, and `verification.md` from that track.
3. Run `git show HEAD --stat` to get the list of changed files.
4. Generate the note text using the format below and write it to `tmp/track-commit/note.md`.
5. Run: `cargo make track-note`

### 3b. Skip note (no track and no generated scratch note)

If no track directory exists and no generated scratch note is available, skip note generation
and mention this in the summary.

## Note format

```markdown
## Task Summary: <brief description>

**Track:** <track-id or "no-track">
**Task:** <task description from metadata.json done task, or commit subject>
**Date:** <YYYY-MM-DD>

### Changes

- <filename>: <what changed — one line per key file>

### Why

<1–3 sentences from spec.md/plan.md or commit context>
```

## Summary output

After execution, report:

1. Commit result (success/failure) and commit hash
2. Commit message used
3. `track/registry.md` status: updated / already current / skipped (reason)
4. Git note status: applied (source: generated tmp scratch file) or skipped (reason)
5. Next recommended action
