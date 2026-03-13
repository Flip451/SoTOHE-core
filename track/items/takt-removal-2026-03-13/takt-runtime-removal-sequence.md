# Takt Runtime Removal Sequence

## Purpose

This document fixes the deletion order and compatibility boundaries for the remaining `takt`
runtime surface. It covers the `cargo make takt-*` wrappers, `.takt/` runtime assets,
`scripts/takt_profile.py`, `scripts/takt_failure_report.py`, and the test/CI/doc surfaces that
still depend on them.

## Removal Goal

The repo must stop requiring:

- `Makefile.toml` `takt-*` wrappers
- `TAKT_PYTHON`
- `.takt/config.yaml`
- `.takt/pieces/**`
- `.takt/personas/**`
- `.takt/runtime/personas/**`
- `.takt/tasks.yaml` and `.takt/tasks/**`
- `scripts/takt_profile.py`
- `scripts/takt_failure_report.py`
- `scripts/test_takt_profile.py`
- `scripts/test_takt_failure_report.py`
- `scripts/test_takt_personas.py`

The final state is `/track:*` + Rust CLI + Claude Code / Agent Teams only, with no queue runtime
or persona-rendering path left in the repo.

## Compatibility Boundary

### Still allowed during T004 planning

These surfaces may remain temporarily because later tasks still need to rewrite docs, settings,
guardrails, and profile schema:

- `cargo make takt-*` wrappers in `Makefile.toml`
- `.claude/settings.json` permissions for `cargo make takt-*`
- `.claude/hooks/agent-router.py` external-guide injection for legacy `takt-*` commands
- `.claude/rules/07-dev-environment.md`, `track/workflow.md`, and other docs that mention legacy
  compatibility paths
- `scripts/test_make_wrappers.py` checks that assert wrappers still exist

### Not allowed after T004 implementation lands

- user-facing workflow docs that still recommend `takt` as the primary path
- CI jobs that still require `scripts/test_takt_*` after the runtime is deleted
- profile schema fields that exist only to support `takt_host_provider` / `takt_host_model`
- `.takt/pending-*` as the normal path for staging, commit, or git notes

## Deletion Order

### Phase A: freeze direct replacements

Preconditions:

- `/track:full-cycle`, `/track:commit`, `/track:review`, PR flow, and guarded git scratch already
  work without `takt`
- `tmp/track-commit/*` is the primary scratch contract

Actions:

1. Remove any remaining public docs that present `cargo make takt-*` as normal workflow.
2. Replace `TAKT_TRACK_TRACEABILITY.md` references with neutral track-traceability guidance or an
   in-place rewrite.
3. Rewrite `.claude/rules/**`, `DEVELOPER_AI_WORKFLOW.md`, `LOCAL_DEVELOPMENT.md`, and
   `track/workflow.md` so any remaining `takt` mention is explicitly "migration-only".

Exit condition:

- no user-facing doc recommends `takt` as a primary command path

### Phase B: remove runtime execution layer

Preconditions:

- `/track:full-cycle` no longer shells into `cargo make takt-full-cycle`
- profile/routing code no longer requires `takt_host_*` fields to exist

Actions:

1. Delete `scripts/takt_profile.py`.
2. Delete `.takt/pieces/**`, `.takt/personas/**`, `.takt/runtime/personas/**`,
   `.takt/tasks.yaml`, and `.takt/tasks/**`.
3. Delete `cargo make takt-add`, `takt-run`, `takt-render-personas`, `takt-full-cycle`,
   `takt-spec-to-impl`, `takt-impl-review`, `takt-tdd-cycle`, and `takt-clean-queue`.
4. Remove `TAKT_PYTHON` from `Makefile.toml`.

Exit condition:

- no executable path in the repo invokes `scripts/takt_profile.py`

### Phase C: remove failure-report helper or generalize it

Decision:

- preferred path is deletion, not rename, unless a generic non-`takt` workflow failure report is
  actively needed by another track

Actions:

1. If no generic consumer exists, delete `scripts/takt_failure_report.py`,
   `cargo make takt-failure-report`, and `scripts/test_takt_failure_report.py`.
2. Otherwise, rename it to a non-`takt` helper and update all docs/tests in the same change.

Exit condition:

- no helper name or output path is `takt`-specific unless explicitly kept as compatibility-only

### Phase D: remove runtime-specific tests from CI

Actions:

1. Remove `scripts/test_takt_profile.py`, `scripts/test_takt_failure_report.py`, and
   `scripts/test_takt_personas.py`.
2. Remove their entries from `scripts-selftest-local` in `Makefile.toml`.
3. Rewrite `scripts/test_make_wrappers.py` and `scripts/test_verify_scripts.py` so they assert the
   post-`takt` state instead of the current wrapper presence.

Exit condition:

- `cargo make ci` passes without any `test_takt_*` suite or `takt-*` wrapper assertions

## Concrete File Groups

### Delete together

- `scripts/takt_profile.py`
- `.takt/pieces/**`
- `.takt/personas/**`
- `.takt/runtime/personas/**`
- `.takt/tasks.yaml`
- `.takt/tasks/**`
- `scripts/test_takt_profile.py`
- `scripts/test_takt_personas.py`

### Delete or rename together

- `scripts/takt_failure_report.py`
- `scripts/test_takt_failure_report.py`
- `[tasks.takt-failure-report]` in `Makefile.toml`
- any docs that mention `.takt/debug-report.md` or `.takt/last-failure.log`

### Update before deleting wrappers

- `.claude/settings.json`
- `scripts/verify_orchestra_guardrails.py`
- `scripts/test_verify_scripts.py`
- `scripts/test_make_wrappers.py`
- `.claude/hooks/agent-router.py`
- `.claude/hooks/test_agent_router.py`
- `.claude/rules/07-dev-environment.md`
- `track/workflow.md`
- `DEVELOPER_AI_WORKFLOW.md`
- `LOCAL_DEVELOPMENT.md`
- `TAKT_TRACK_TRACEABILITY.md`

## Risk Notes

### Highest-risk breakages

- deleting `takt-full-cycle` before `/track:full-cycle` is fully decoupled
- deleting `scripts/test_takt_*` before CI/verifier expectations are rewritten
- deleting profile schema fields before `.claude/hooks/_agent_profiles.py` and
  `.claude/agent-profiles.json` are migrated
- deleting `.takt/pending-*` wrappers before all remaining docs and fallback paths are narrowed to
  migration-only usage

### Lower-risk cleanups

- deleting rendered runtime personas after no command regenerates them
- removing queue cleanup commands once `takt-run` / `takt-add` are gone
- removing `TAKT_PYTHON` once no task reads it

## Exit Criteria for T004

- the removal order is explicit enough that T005 can update guardrails/docs/profile schema without
  re-deciding runtime deletion order
- every remaining `takt-*` wrapper and `.takt/**` runtime asset belongs to a named phase above
- CI/test fallout for each deletion phase is listed before implementation starts
