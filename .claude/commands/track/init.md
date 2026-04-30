---
description: Initialize a new track directory and its branch (Phase 0).
---

Arguments:

- Use `$ARGUMENTS` as the feature name (or a slug-ready phrase).
- If empty, ask for a feature name and stop.
- Derive `<track-id>` from `$ARGUMENTS`: kebab-case ASCII + date suffix `YYYY-MM-DD` from `date -u +"%Y-%m-%d"`.

Execution:

1. Pre-flight: check current git state and prerequisites:
   ```bash
   git branch --show-current   # expected: `main`
   git status --short          # expected: clean (or only ADR / convention files
                               #   that will be committed inside the new track)
   ```
   - If the current branch is **not `main`**: stop and present the situation to the user. Step 2 (`cargo make track-branch-create`) creates the new branch from `main`, so the only valid options are: switch to main manually, or abort. Do **not** auto-switch.
   - If `git status --short` is non-empty: present the list to the user and classify each item:
     - ADR / convention / other `knowledge/` baseline files staged for the new track (typical case): **already resolved** — they will be committed inside the new track at step 6 below. No user action required.
     - Other in-progress changes unrelated to the new track: ask the user whether to commit, stash, discard, or split into a separate track. Do **not** auto-act.
   - Proceed to step 2 once the current branch is `main` and any unrelated in-progress changes have been resolved by the user. Baseline files left in the working tree do not block step 2.
2. Create the track branch from main and switch to it:
   ```bash
   cargo make track-branch-create '<track-id>'
   ```
3. Create `track/items/<track-id>/metadata.json`:
   - `schema_version`: 5
   - `id`: `<track-id>`
   - `title`: `<human-readable title>`
   - `branch`: `track/<track-id>`
   - `created_at` / `updated_at`: `date -u +"%Y-%m-%dT%H:%M:%SZ"`
4. Render rendered views (`plan.md` + `track/registry.md`) from `metadata.json`:
   ```bash
   cargo make track-sync-views
   ```
   (`verify-track-metadata` in step 5 checks that `track/registry.md` is in sync with `metadata.json`, so this render must run before step 5. A warning about `contract-map.md` skipping the new track because no `domain-types.json` exists yet is expected at this phase — the catalogue is created later in Phase 2 by `/track:type-design`.)
5. Verify identity schema:
   ```bash
   cargo make verify-track-metadata
   ```
6. (Optional, typical) If ADR / convention files prepared for this track are present in the working tree without commit history, the typical flow is to commit them as the first commit of the new track via `/track:review` → `/track:commit` immediately after step 5 (so `/track:plan` later sees a committed ADR and back-and-forth `adr-editor` editing can proceed under the "commit history exists" path defined in `knowledge/conventions/pre-track-adr-authoring.md`). At this point `metadata.json` (step 3) and `plan.md` (step 4) both exist; `spec.md` is created later by `/track:spec-design` in Phase 1 and is not required for this first commit.

Report: track id, track directory, branch name, `verify-track-metadata` result.
