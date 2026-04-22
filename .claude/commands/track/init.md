---
description: Initialize a new track directory and materialize its branch (Phase 0).
---

Canonical command for Phase 0 track initialization.

Writer: main. `/track:init` is the only phase command whose writer is main. The other three phase commands (`/track:spec-design` / `/track:type-design` / `/track:impl-plan`) delegate to subagents that own their respective SSoT files; `/track:init` runs the identity-only bootstrap directly from main.

Arguments:

- Use `$ARGUMENTS` as the feature name (or a slug-ready phrase).
- If empty, ask for a feature name and stop.
- Derive `<track-id>` from `$ARGUMENTS`: kebab-case ASCII (romaji or concise English) + date suffix `YYYY-MM-DD` from `date -u +"%Y-%m-%d"`.

Execution:

1. **ADR pre-check (strict mode)**: Check `knowledge/adr/` for an ADR that covers this feature. If no relevant ADR exists, stop and instruct the user to author one via `/adr:add <slug>`. See `knowledge/conventions/pre-track-adr-authoring.md` §Rules.
2. Create `track/items/<track-id>/` and write `track/items/<track-id>/metadata.json` with:
   - `schema_version: 5`
   - `id: "<track-id>"`
   - `title: "<human-readable title>"`
   - `branch: null` (provisional — updated in step 4)
   - `created_at` / `updated_at` from `date -u +"%Y-%m-%dT%H:%M:%SZ"`
3. Create the track branch from main and switch to it. This command materializes
   `metadata.json.branch` to `track/<track-id>` and commits the activation artifact:
   ```bash
   cargo make track-branch-create '<track-id>'
   ```
4. Confirm that `metadata.json.branch` is now `"track/<track-id>"` (set by the
   wrapper above; do not re-edit `metadata.json` manually).
5. Run identity schema binary verification:
   ```bash
   cargo make verify-track-metadata
   ```
   Stop if verification fails.

Report:

- track id
- track directory path
- branch name
- referenced ADR path(s)
- `verify-track-metadata` result

Behavior:

- No subagent invocation — main is the writer for `/track:init`. It is the only phase command whose writer is main; the other three phase commands delegate to subagents.
- `/track:init` is single-shot: invoke it once and it terminates. It does not loop, retry, or trigger further phase commands.
