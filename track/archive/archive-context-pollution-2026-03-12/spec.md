# WF-07: Archived Track Context Pollution Prevention

## Goal

Prevent archived (completed) track directories from appearing in Claude Code's AI search results, reducing context noise while maintaining full CI validation coverage.

## Background

When tracks reach `done` status and are archived via `/track:archive`, their files remain in `track/items/`. This causes AI tools (Read, Grep, Glob) to return archived content alongside active work, polluting search context and reducing signal-to-noise ratio.

## Scope

### In Scope

- Physical move of archived track directories from `track/items/<id>/` to `track/archive/<id>/`
- Shared helper function for scripts to scan both locations
- Update 5 verify/registry Python scripts to scan `track/archive/` in addition to `track/items/`
- Update `/track:archive` command to perform physical move
- Add Claude Code deny rules to block AI search on `track/archive/`
- Update CLAUDE.md workspace map
- Unit and integration tests for new behavior

### Out of Scope

- Changes to Rust source code
- Changes to `track_resolution.py`, `git_ops.py`, `track_state_machine.py`, `pr_review.py` (these operate only on active tracks)
- Changes to the track branch guard logic
- Deletion or compression of archived track content

## Constraints

- Archived tracks must continue to pass all verify scripts (`verify-plan-progress`, `verify-track-metadata`, `verify-track-registry`)
- `verify-latest-track` must continue to skip archived tracks (already implemented via `_SKIP_STATUSES`)
- Registry rendering must continue to show archived tracks in the "Archived Tracks" section
- `cargo make ci` must pass after migration
- Git history should be preserved — use `git mv` for moves so git detects renames natively (do not rely on `shutil.move` + similarity-based rename detection)

## Acceptance Criteria

1. All archived track directories reside under `track/archive/<id>/`, not `track/items/<id>/`
2. `cargo make ci` passes with tracks in the new location
3. Claude Code deny rules prevent Read/Grep/Glob on `track/archive/**`
4. `track/registry.md` correctly renders archived tracks from the new location
5. New track creation and archiving workflow works end-to-end
6. `all_track_directories()` helper returns directories from both locations, sorted by name
