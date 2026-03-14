---
description: Plan a feature without materializing its track branch yet.
---

Canonical wrapper for planning-only track creation.

Arguments:
- Use `$ARGUMENTS` as the feature name.
- If empty, ask for a feature name and stop.

Execution:
- Follow the same planning workflow as `/track:plan`.
- After approval, create the track artifacts under `track/items/<track-id>/` with `schema_version: 3`, `status: planned`, and `branch: null`.
- Do not create or switch to `track/<track-id>` during this command.
- Render `plan.md` from `metadata.json`; do not write `plan.md` directly.
- Update `track/registry.md` as the rendered view of the new planning-only track.
- Do not implement code in this command.

Behavior:
- Treat this command as the planning gate for work that should be reviewed or landed before activation.
- After execution, summarize:
  1. Plan summary
  2. Created track id/path
  3. Created/updated files
  4. Suggested next command (`/track:ci`, `/track:review <track-id>`, `/track:commit <track-id> -- <message>`, or `/track:activate <track-id>`)
