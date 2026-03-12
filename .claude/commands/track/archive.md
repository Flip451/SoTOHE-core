---
description: Archive a completed track, moving it out of active view.
---

Archive a completed track to reduce registry noise.

Arguments:
- Use `$ARGUMENTS` as the track ID to archive. If empty, list completed tracks and ask the user to choose.

Execution:

## Step 1: Validate target track

1. If `$ARGUMENTS` is empty, list all tracks with `done` status from `track/items/*/metadata.json` and ask the user to choose. (Archived tracks already reside in `track/archive/` and should not appear here.)
2. Locate `track/items/<id>/metadata.json` and read it.
3. Confirm the track status is `done` and all tasks are resolved (`done` or `skipped`). If the track is not fully resolved (`planned`, `in_progress`, `blocked`, `cancelled`), refuse and explain why.

## Step 2: Update metadata.json

1. Set `status` to `"archived"` in `metadata.json`.
2. Update `updated_at` to the current ISO 8601 timestamp.

## Step 3: Move track directory

1. Run `mkdir -p track/archive` to ensure the archive root exists.
2. Run `git mv track/items/<id> track/archive/<id>` to physically move the track directory.
   - Using `git mv` preserves rename history (do not use `shutil.move` or `mv`).
3. The moved files retain the updated metadata.json from Step 2.

## Step 4: Regenerate rendered views

1. Run `cargo make track-sync-views` to regenerate `plan.md` and `registry.md`.
2. Verify the track moved from Active/Completed to the Archived Tracks section in `registry.md`.

## Step 5: Stage changes

1. Write the changed file paths (including `track/archive/<id>/metadata.json` and other moved files, plus `track/registry.md`) to `tmp/track-commit/add-paths.txt`.
2. Run `cargo make track-add-paths`.

Behavior:
- This command moves the track directory from `track/items/` to `track/archive/` and regenerates views.
- Archived tracks reside in `track/archive/<id>/` and remain fully readable.
- Archived tracks are excluded from `Current Focus`, `Active Tracks`, and latest-track verification in registry.md.
- Verify scripts (`verify-plan-progress`, `verify-track-metadata`) scan both `track/items/` and `track/archive/` via `all_track_directories()`. This is intentional: archived tracks should still pass validation.
- `verify-latest-track` skips archived tracks when selecting the latest track for artifact checks.
- Claude Code deny rules (`Read`, `Grep`, `Glob`) on `track/archive/**` prevent archived content from polluting AI search context.

Output:

1. Archived track ID
2. Updated files
3. Suggested next command (`/track:commit <message>` to persist the archive)
