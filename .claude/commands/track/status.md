---
description: Show current track progress from track/registry.md and metadata.json state.
---

Report current progress for the track workflow.

Execution rules:
- Report the current git branch and whether it is a track branch (matches `track/<id>` pattern).
- Read `track/registry.md`.
- Resolve the current track in this order:
  1. If the current git branch matches `track/<id>`, use that track.
  2. Otherwise, use the latest materialized active track (non-archived, non-done, `branch != null`).
  3. If no materialized active track exists, fall back to the latest branchless planning-only track (`status=planned`, `branch=null`).
- If any track exists, identify the current track directory under `track/items/`.
- Read the current track's `metadata.json` when present.
- Read the current track's `spec.md` and `plan.md` (if present).
- Read `verification.md` when present.
- Derive the current phase:
  - `Ready to Activate` for branchless planning-only tracks
  - `Planning` for materialized `planned` tracks
  - `In Progress` for tracks with active implementation
  - `Done` for completed tracks
- Summarize:
  - current git branch and whether it is a track branch
  - current focus from `track/registry.md`
  - active tracks
  - current track id/name
  - phase (`Ready to Activate`, `Planning`, `In Progress`, `Done`)
  - metadata status / updated_at (if present)
  - task state counts from `metadata.json` (todo / in_progress / done)
  - manual verification status from `verification.md` (if present)
  - next recommended action

Next-command rules:
- If the current track is branchless planning-only, recommend `/track:activate <track-id>`.
- If the current track is materialized and `planned`, recommend `/track:implement`.
- If the current track is already in implementation, recommend the next workflow command that matches its state.

Output format:
1. Track summary
2. Current track status
3. Recommended next command
