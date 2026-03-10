---
description: Show current track progress from track/registry.md and metadata.json state.
---

Report current progress for the track workflow.

Execution rules:
- Read `track/registry.md`.
- If any track exists, identify the latest active track directory under `track/items/`.
- Read the latest track's `metadata.json` when present.
- Read the latest track's `spec.md` and `plan.md` (if present).
- Read `verification.md` when present.
- Summarize:
  - current focus from `track/registry.md`
  - active tracks
  - latest track id/name
  - metadata status / updated_at (if present)
  - task state counts from `metadata.json` (todo / in_progress / done)
  - manual verification status from `verification.md` (if present)
  - next recommended action

Output format:
1. Track summary
2. Latest track status
3. Recommended next command
