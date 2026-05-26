---
description: Switch to main, pull latest, and show track completion summary.
---

Canonical command for returning to main after a track is merged.

Arguments:
- `$ARGUMENTS` is unused.

## Step 1: Switch to main

Run:

```bash
cargo make track-switch-main
```

This checks out `main` and pulls the latest changes from origin.

## Step 2: Completion summary

After switching to main:

1. Read `track/registry.md` and show:
   - Latest completed track name and date
   - Number of active tracks remaining
2. Recommend next action:
   - If active tracks remain: `/track:implement` or `/track:full-cycle <task>`
   - If no active tracks: `/track:plan <feature>` to start new work

## Behavior

After execution, summarize:
1. Branch switch result (success/failure)
2. Latest completed track
3. Active track count
4. Recommended next command
