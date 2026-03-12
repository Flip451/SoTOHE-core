---
description: Show current track progress from track/registry.md and metadata.json state.
---

Report current progress for the track workflow.

Execution rules:
- Report the current git branch and whether it is a track branch (matches `track/<id>` pattern).
- Read `track/registry.md`.
- Resolve the current track: if the current git branch matches `track/<id>`, use that track. Otherwise, fall back to the latest active track by `updated_at`.
- If any track exists, identify the current track directory under `track/items/`.
- Read the current track's `metadata.json` when present.
- Read the current track's `spec.md` and `plan.md` (if present).
- Read `verification.md` when present.
- Summarize:
  - current git branch and whether it is a track branch
  - current focus from `track/registry.md`
  - active tracks
  - current track id/name
  - metadata status / updated_at (if present)
  - task state counts from `metadata.json` (todo / in_progress / done)
  - manual verification status from `verification.md` (if present)
  - next recommended action

Output format:
1. Track summary
2. Current track status
3. Recommended next command
