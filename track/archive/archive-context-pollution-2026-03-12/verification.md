# Verification: WF-07 Archive Context Pollution Prevention

## Scope Verified

- Physical move of 15 archived tracks from `track/items/` to `track/archive/`
- 5 verify/registry scripts updated to scan both directories
- `/track:archive` command updated with physical move step
- Claude Code deny rules added for `track/archive/**`
- CLAUDE.md workspace map updated

## Manual Verification Steps

### 1. Archive Directory Structure
- Confirm `track/archive/` contains all 15 previously archived track directories
- Confirm `track/items/` contains only active (non-archived) tracks
- Confirm each archived track has `metadata.json`, `plan.md`, `spec.md`, `verification.md`

### 2. CI Validation
- Run `cargo make ci` and confirm all checks pass
- Run `cargo make verify-plan-progress` — archived tracks in `track/archive/` are validated
- Run `cargo make verify-track-metadata` — archived tracks in `track/archive/` are validated
- Run `cargo make verify-track-registry` — registry.md includes archived tracks from new location
- Run `cargo make verify-latest-track` — skips archived tracks as before

### 3. Search Exclusion
- Confirm `.claude/settings.json` deny rules block Read/Grep/Glob on `track/archive/**`
- Verify that Bash-based verify scripts (running outside Claude Code) are unaffected

### 4. End-to-End Archive Flow
- Create a test track, complete it, run `/track:archive`
- Confirm track is moved to `track/archive/` and registry updated

## Result / Open Issues

(To be filled after implementation)

## Verified At

(To be filled after verification)
