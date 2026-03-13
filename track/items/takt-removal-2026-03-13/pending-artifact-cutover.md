# Pending Artifact Cutover

## Purpose

This document fixes the scratch-path contract for guarded staging, commit, and git-note flows
before the remaining `takt` runtime is removed. It narrows the "pending artifact" role to an
explicit `tmp/track-commit/` scratch area and treats `.takt/pending-*` only as a migration-era
compatibility input for still-existing legacy wrappers.

## Target Contract

### Primary scratch location

All non-legacy guarded git flows converge on:

- `tmp/track-commit/add-paths.txt`
- `tmp/track-commit/commit-message.txt`
- `tmp/track-commit/note.md`
- `tmp/track-commit/track-dir.txt`

These files are:

- repo-local scratch, not source artifacts
- excluded from `cargo make add-all`
- deleted by their exact wrapper on success
- allowed to be absent on normal clean worktrees

### Legacy compatibility window

The following paths remain readable only while `takt-*` wrappers still exist:

- `.takt/pending-add-paths.txt`
- `.takt/pending-commit-message.txt`
- `.takt/pending-note.md`
- `.takt/handoffs/`
- `.takt/last-failure.log`
- `.takt/debug-report.md`

They are no longer the preferred source for `/track:commit` or normal manual workflow guidance.
They exist only so the repo can continue to execute migration-period wrappers until T004 removes
them.

## Workflow Rules

1. `/track:commit` writes only `tmp/track-commit/commit-message.txt`, `tmp/track-commit/note.md`,
   and `tmp/track-commit/track-dir.txt`.
2. Selective staging for current workflows writes only `tmp/track-commit/add-paths.txt`.
3. `cargo make track-add-paths`, `cargo make track-commit-message`, and `cargo make track-note`
   are the canonical file-based wrappers.
4. `cargo make add-pending-paths`, `cargo make commit-pending-message`, and
   `cargo make note-pending` are migration-only compatibility wrappers and must not be the
   recommended path in user-facing docs.
5. `cargo make add-all`, `sotp git add-all`, and the shared transient-path validators must keep
   excluding both the primary `tmp/track-commit/` scratch files and the remaining `.takt/**`
   legacy scratch paths until T004 lands.

## Test and Implementation Implications

### Rust / Python helper parity

- `libs/usecase/src/git_workflow.rs` defines the shared transient path policy.
- `scripts/git_ops.py` mirrors that policy for remaining compatibility wrappers.
- Regression tests should use `tmp/track-commit/` for the happy path and keep at least one
  compatibility test for legacy `.takt/pending-*` inputs until T004 deletes them.

### Traceability expectations

- A successful guarded commit must be able to complete with only `tmp/track-commit/*`.
- Git note generation is no longer allowed to depend on `.takt/pending-note.md` existing.
- The fallback inline note generation described in `/track:commit` is the normal path; legacy
  pending notes are optional compatibility input only.

## Exit Criteria for T003

- Current docs describe `tmp/track-commit/` as the primary scratch path.
- `/track:commit` no longer prefers `.takt/pending-note.md`.
- Helper tests use `tmp/track-commit/` for the primary success cases.
- The remaining `.takt/pending-*` and `.takt/handoffs` references are explicitly documented as
  migration-only compatibility paths for later removal in T004/T005.
