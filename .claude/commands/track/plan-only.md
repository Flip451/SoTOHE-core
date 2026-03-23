---
description: Plan a feature without materializing its track branch yet.
---

Canonical wrapper for planning-only track creation.

Arguments:
- Use `$ARGUMENTS` as the feature name.
- If empty, ask for a feature name and stop.

Execution:
- Follow the same planning workflow as `/track:plan`.
- After approval, create a planning branch `plan/<track-id>` from `main` and switch to it:
  ```bash
  cargo make track-plan-branch '<track-id>'
  ```
- Create the track artifacts under `track/items/<track-id>/` with `schema_version: 3`, `status: planned`, and `branch: null`.
- `metadata.json.branch` remains `null` — the `plan/<track-id>` branch is a temporary review branch, not the implementation branch.
- Create `spec.json` (spec SSoT) following the same schema as `/track:plan`. Do NOT write `spec.md` directly.
- Run `cargo make track-sync-views` to generate `plan.md` from `metadata.json` and `spec.md` from `spec.json`.
- Update `track/registry.md` as the rendered view of the new planning-only track.
- Do not implement code in this command.
- Do not create or switch to `track/<track-id>` — that is `/track:activate`'s responsibility.

Behavior:
- Treat this command as the planning gate for work that should be reviewed or landed before activation.
- After execution, summarize:
  1. Plan summary
  2. Created track id/path
  3. Planning branch name (`plan/<track-id>`)
  4. Created/updated files
  5. Suggested next steps:
     - Push the planning branch and create a PR for review
     - After PR merge to main, run `/track:activate <track-id>` to create the implementation branch
     - Note: on the `plan/<id>` branch (non-track branch), `/track:review` and `/track:commit` require explicit `<track-id>` selector (e.g., `/track:commit <track-id> -- <message>`)
