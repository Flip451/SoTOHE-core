---
description: Initialize a new track directory and its branch (Phase 0).
---

Arguments:

- Use `$ARGUMENTS` as the feature name (or a slug-ready phrase).
- If empty, ask for a feature name and stop.
- Derive `<track-id>` from `$ARGUMENTS`: kebab-case ASCII + date suffix `YYYY-MM-DD` from `date -u +"%Y-%m-%d"`.

Execution:

1. Create the track branch from main and switch to it:
   ```bash
   cargo make track-branch-create '<track-id>'
   ```
2. Create `track/items/<track-id>/metadata.json`:
   - `schema_version`: 5
   - `id`: `<track-id>`
   - `title`: `<human-readable title>`
   - `branch`: `track/<track-id>`
   - `created_at` / `updated_at`: `date -u +"%Y-%m-%dT%H:%M:%SZ"`
3. Verify identity schema:
   ```bash
   cargo make verify-track-metadata
   ```

Report: track id, track directory, branch name, `verify-track-metadata` result.
