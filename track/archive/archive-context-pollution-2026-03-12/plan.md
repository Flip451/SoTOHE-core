<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# WF-07: Archived track context pollution prevention — physical move to track/archive/ with script and deny rule updates

Prevent archived track directories from polluting AI search context by physically moving them to track/archive/ and adding Claude Code deny rules.
All verify scripts and registry rendering updated to scan both track/items/ and track/archive/ via a shared helper.
No Rust code changes. Python scripts + command definitions + settings only.
Backward compatible: existing CI validation continues to cover archived tracks.

## Core Infrastructure

Add TRACK_ARCHIVE_DIR constant and all_track_directories() helper to track_schema.py.
This shared helper returns sorted track directories from both track/items/ and track/archive/.

- [x] Add TRACK_ARCHIVE_DIR constant and all_track_directories() helper to track_schema.py

## Script Updates

Update 5 verify/registry scripts to scan both directories via the new helper.
verify_plan_progress, verify_track_metadata, verify_latest_track_files, track_registry, verify_tech_stack_ready.

- [x] Update verify_plan_progress.py track_dirs() to use all_track_directories()
- [x] Update verify_track_metadata.py main() to use all_track_directories()
- [x] Update verify_latest_track_files.py track_dirs() to use all_track_directories()
- [x] Update track_registry.py collect_track_metadata() to scan both directories
- [x] Update verify_tech_stack_ready.py has_track_dirs() and all_tracks_planned() for both directories

## Archive Command Update

Update /track:archive command definition to add physical move step.
After metadata update, mkdir -p track/archive/ and git mv track/items/<id>/ to track/archive/<id>/.

- [x] Update /track:archive command to mkdir -p track/archive/ and git mv track dir to track/archive/<id>/

## Search Exclusion

Add deny rules to .claude/settings.json to block Read/Grep/Glob on track/archive/**.
Verify scripts run via Bash and are unaffected by deny rules.

- [x] Add deny rules for track/archive/** in .claude/settings.json (Read, Grep, Glob)

## Documentation and Tests

Update CLAUDE.md Workspace Map to include track/archive/.
Update archive command Behavior section.
Add unit tests for all_track_directories() and integration tests for archive directory scanning.

- [x] Update CLAUDE.md Workspace Map and archive command Behavior section for track/archive/
- [x] Update test_track_schema.py: TRACK_ARCHIVE_DIR consistency test for scripts referencing track/archive, and all_track_directories() unit tests
- [x] Add archive directory tests in test_track_registry.py and test_verify_scripts.py

## Integration Validation

Physically move all 15 existing archived tracks to track/archive/.
Run cargo make ci to validate all scripts, views, and tests pass.

- [x] Move all existing archived tracks to track/archive/ and run cargo make ci for full validation
