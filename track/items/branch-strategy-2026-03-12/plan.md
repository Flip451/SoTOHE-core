<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Feature branch strategy for track workflow

Introduce per-track feature branches to isolate parallel track work from main.
Replace global timestamp-based track resolution with branch-aware resolution.
Distinguish 'current track' (branch-bound, interactive) from 'latest track' (global, CI/reporting).
Add optional worktree support for Agent Teams parallel workers on the same track.
Auto-configure git notes refspec on bootstrap.

## Permission and Wrapper Prerequisites

Add git branch/switch/checkout permissions to .claude/settings.json and .claude/permission-extensions.json, add Makefile.toml wrappers, update hook whitelist and verify_orchestra_guardrails.py before any branch-creating task.

- [ ] Permission and wrapper prerequisites: add git switch/checkout -b/branch to .claude/settings.json allowedCommands, mirror entries in .claude/permission-extensions.json, add Makefile.toml track-branch-create/track-branch-switch wrapper tasks, update block-direct-git-ops.py to whitelist these wrappers, update verify_orchestra_guardrails.py hardcoded allowlists to accept new wrappers

## Schema and Track Resolution

Add branch field to metadata.json (schema v3) with dual v2/v3 validation and rendering.
Replace all latest_track_dir() consumers with branch-aware resolution that distinguishes current track from latest track.

- [ ] metadata.json schema v3: add branch field, dual-read v2/v3 in parse_metadata_v2() and validate_metadata_v2() (accept schema_version 2 or 3), update render_plan()/render_registry()/sync_rendered_views() for v3 compatibility, update verify_latest_track_files.py validation
- [ ] Branch-aware track resolution: replace all latest_track_dir() consumers (verify_latest_track_files.py, external_guides.py) and update all command prompts that reference 'latest track' (.claude/commands/track/commit.md, status.md, revert.md, review.md, implement.md, full-cycle.md, catchup.md) with resolve_track_dir() that distinguishes 'current track' (branch-bound, for interactive ops) from 'latest track' (global, for CI/reporting). Add find_track_by_branch(), current_git_branch() helpers

## Branch Lifecycle CLI

Implement sotp track branch subcommand for creating and switching track branches via Makefile.toml wrappers.

- [ ] sotp track branch CLI subcommand: create and switch track branches via Makefile.toml wrapper tasks (depends on T0 permission setup)

## Guard Policy Extension

Block history-mutating operations (merge, rebase, cherry-pick, reset) in guard hooks.

- [ ] Guard policy update: block merge/rebase/cherry-pick/reset in Rust policy and Python hook

## Workflow Integration

Update /track:plan, /track:commit, and registry rendering for branch-aware operation.

- [ ] /track:plan update: auto-create and switch to track branch after approval using T0 wrappers, write branch field to metadata.json (depends on T0, T1)
- [ ] /track:commit and registry update: branch-context-aware commit flow and registry rendering. Update track_registry.py render_registry() to prefer current-branch track for Current Focus. Ensure /track:status, /track:review, /track:implement, /track:ci also use current-track resolution

## Bootstrap and Migration

Auto-configure notes refspec, migrate legacy tracks, update documentation.

- [ ] Notes refspec bootstrap: auto-configure fetch refspec in cargo make bootstrap
- [ ] Migration and docs: legacy track fallback, workflow docs update, DEVELOPER_AI_WORKFLOW.md update
