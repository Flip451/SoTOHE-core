# Verification: takt 廃止計画

## Scope Verified

- [x] current takt touchpoints were inspected before planning
- [x] takt removal scope is separated from general Python deprecation work
- [x] public `/track:*` compatibility remains an explicit constraint
- [x] docs / wrapper / guardrail / CI migration surfaces are captured
- [x] `.claude/commands/**`, `.claude/rules/**`, `.claude/docs/WORKFLOW.md`, and `START_HERE_HUMAN.md` are recognized as in-scope migration surfaces
- [x] agent routing/profile files and guarded git transient path rules are recognized as in-scope migration surfaces
- [x] `takt-touchpoint-inventory.md` fixes runtime, docs, routing, guardrail, and scratch-path cutover principles in one artifact
- [x] `/track:full-cycle` is now documented as a Claude Code-native autonomous workflow rather than a `takt` wrapper
- [x] setup, onboarding, and agent-router workflow hints no longer direct users to `cargo make takt-*`
- [x] `pending-artifact-cutover.md` fixes `tmp/track-commit/` as the primary scratch contract and demotes `.takt/pending-*` to migration-only compatibility inputs
- [x] `/track:commit` and `track/workflow.md` now describe generated `tmp/track-commit/note.md` as the normal git-note path rather than preferring `.takt/pending-note.md`
- [x] Rust/Python guarded git helper tests use `tmp/track-commit/*` as the happy path while preserving one legacy `.takt/pending-note.md` compatibility case
- [x] `takt-runtime-removal-sequence.md` fixes deletion phases for `takt-*` wrappers, `.takt/**` runtime assets, `scripts/takt_profile.py`, and the related test/CI fallout
- [x] `.claude/settings.json` and `verify_orchestra_guardrails.py` no longer treat `takt-*` wrappers or `.cache/takt-uv/**` as baseline guardrail surfaces
- [x] `.claude/agent-profiles.json` now uses `workflow_host_provider` / `workflow_host_model` as the canonical host schema, while hooks keep `takt_host_*` as migration compatibility aliases
- [x] public docs and traceability rules now describe `takt-*` and `.takt/pending-*` as legacy migration compatibility only, not as the normal execution path
- [x] `takt-removal-definition-of-done.md` fixes the post-`takt` required-path matrix, milestone gates, rollout order, and final blockers in one artifact

## Manual Verification Steps

1. Read `track/workflow.md`, `TAKT_TRACK_TRACEABILITY.md`, and `DEVELOPER_AI_WORKFLOW.md`
2. Inspect `Makefile.toml` for `takt-*` and `takt-failure-report` wrappers
3. Inspect `scripts/takt_profile.py` and `scripts/test_takt_profile.py`
4. Inspect `.takt/` runtime definitions and confirm they are in scope for removal
5. Inspect `.claude/commands/track/full-cycle.md`, `.claude/commands/track/setup.md`, `.claude/commands/track/commit.md`, `.claude/docs/WORKFLOW.md`, `.claude/rules/07-dev-environment.md`, and `START_HERE_HUMAN.md`
6. Inspect `.claude/hooks/agent-router.py`, `.claude/hooks/_agent_profiles.py`, and `.claude/agent-profiles.json`
7. Inspect `libs/usecase/src/git_workflow.rs`, `scripts/git_ops.py`, and `scripts/test_git_ops.py`
8. Read `takt-touchpoint-inventory.md` and confirm each remaining task maps to one or more inventory sections
9. Verify this track's `metadata.json`, `spec.md`, and `plan.md` align
10. Confirm `track/registry.md` lists this track in `Completed Tracks` and shows no remaining active track when this completion slice is rendered
11. Read `.claude/commands/track/full-cycle.md`, `.claude/commands/track/setup.md`, `.claude/docs/WORKFLOW.md`, `START_HERE_HUMAN.md`, and `.claude/hooks/agent-router.py` and confirm they describe Claude Code + Agent Teams + Rust CLI orchestration rather than `takt` execution
12. Read `track/items/takt-removal-2026-03-13/pending-artifact-cutover.md` and confirm it defines `tmp/track-commit/*` as the primary scratch area and `.takt/pending-*` as migration-only compatibility input
13. Read `.claude/commands/track/commit.md` and `track/workflow.md` and confirm they prefer `cargo make track-note` / `cargo make track-commit-message` over legacy `.takt/pending-*` wrappers
14. Run `cargo test -p usecase git_workflow -- --nocapture`
15. Run `cargo test -p cli git -- --nocapture`
16. Run `pytest -q -o cache_dir=.cache/pytest scripts/test_git_ops.py scripts/test_make_wrappers.py`
17. Run `cargo run --quiet -p cli -- track views validate --project-root .`
18. Read `track/items/takt-removal-2026-03-13/takt-runtime-removal-sequence.md` and confirm each remaining wrapper/runtime/test surface is assigned to a named deletion phase
19. Read `track/items/takt-removal-2026-03-13/takt-removal-definition-of-done.md` and confirm it aligns with the inventory, removal sequence, `/track:*` flow, PR workflow, and CI gate expectations
20. Run `cargo make ci`

## Result

Pass

## Open Issues

This is a planning track only; no code, docs, or workflow behavior has been removed yet.
The repo still contains `takt` wrappers, `.takt/` runtime assets, and traceability rules that assume takt-generated pending files.
The repo still contains residual `takt` wrappers, `.takt/**` runtime assets, and legacy queue/profile tests because T004 only fixed the deletion order, not the removal itself.
T001 fixes inventory scope only; no runtime/doc/guardrail deletion has started yet.
Legacy `.takt/pending-*` and `.takt/handoffs` compatibility paths still exist by design until T004 removes the remaining wrappers and runtime assets.
The runtime/wrapper removal plan is fixed, but the actual deletion is intentionally deferred to the next implementation slice so Makefile tasks, `.takt/**`, and their Python/runtime tests can move in lockstep.
This track now fixes the rollout gate and end-state definition, but it does not itself delete `takt` runtime assets.

## Progress Notes

- Added `takt-touchpoint-inventory.md` to freeze the current `takt` dependency surface and the sequencing constraints for removing it.
- The inventory explicitly covers `.takt/` runtime assets, `cargo make takt-*`, `scripts/takt_profile.py`, public `/track:*` command docs, `.claude` routing/rules, guardrail verifier/test surfaces, and guarded git transient-path contracts.
- Rewrote `.claude/commands/track/full-cycle.md` to describe a Claude Code-native autonomous implementation loop instead of shelling into `cargo make takt-full-cycle`.
- Rewrote `.claude/commands/track/setup.md`, `.claude/docs/WORKFLOW.md`, `START_HERE_HUMAN.md`, and the agent-router workflow hint so user-facing guidance no longer treats `takt` as the execution layer.
- Current `/track:review` loop for T002 found and fixed two doc/runtime mismatches: `.claude/docs/WORKFLOW.md` incorrectly implied `cargo make track-pr-review` only works after a PR already exists, and the agent-router stopped injecting external-guide context for still-supported `cargo make takt-*` commands during the migration window.
- Follow-up cleanup removed a duplicate external-guide bullet from `.claude/commands/track/full-cycle.md` and added regression coverage in `.claude/hooks/test_agent_router.py`.
- Latest verification for the T002 review closeout: `python3 -m pytest -q .claude/hooks/test_agent_router.py`, `cargo make ci-rust`, and `cargo make ci` passed on the final uncommitted diff.
- Added `pending-artifact-cutover.md` to freeze the cutover from `.takt/pending-*` to `tmp/track-commit/*` and to separate primary scratch from migration-only compatibility paths.
- Updated `.claude/commands/track/commit.md` and `track/workflow.md` so `/track:commit` and git-note guidance now treat generated `tmp/track-commit/note.md` as the normal path and legacy `.takt/pending-note.md` only as a compatibility fallback.
- Rebased Rust/Python guarded git helper tests onto `tmp/track-commit/*` for their success paths while keeping a targeted legacy `.takt/pending-note.md` compatibility test.
- Latest verification for T003 implementation: `cargo test -p usecase git_workflow -- --nocapture`, `cargo test -p cli git -- --nocapture`, `pytest -q -o cache_dir=.cache/pytest scripts/test_git_ops.py scripts/test_make_wrappers.py`, and `cargo run --quiet -p cli -- track views validate --project-root .` passed on the final uncommitted diff.
- Added `takt-runtime-removal-sequence.md` to lock the removal order for `TAKT_PYTHON`, `takt-*` wrappers, `.takt/pieces/**`, `.takt/personas/**`, queue assets, and the corresponding `test_takt_*` suites.
- The removal plan now separates four phases: doc/rule cleanup, runtime execution deletion, failure-report delete-or-generalize, and CI/test cleanup.
- Latest verification for T004 planning slice: read `Makefile.toml`, `scripts/takt_profile.py`, and the current wrapper/test inventory, then ran `cargo run --quiet -p cli -- track views validate --project-root .`.
- T005 removed baseline `takt-*` permissions and `.cache/takt-uv/**` deny rules from `.claude/settings.json`, then aligned `scripts/verify_orchestra_guardrails.py` and its selftests with the new baseline.
- T005 changed `.claude/agent-profiles.json` to canonical `workflow_host_provider` / `workflow_host_model` keys, while `.claude/hooks/_agent_profiles.py` now accepts legacy `takt_host_*` keys as compatibility aliases for the remaining migration-era runtime.
- T005 narrowed `.claude/hooks/agent-router.py` so external-guide injection is keyed to `/track:*` commands only, and updated the related hook regression tests.
- T005 rewrote `LOCAL_DEVELOPMENT.md`, `DEVELOPER_AI_WORKFLOW.md`, `track/workflow.md`, `TAKT_TRACK_TRACEABILITY.md`, `.claude/rules/07-dev-environment.md`, and `.claude/commands/track/commit.md` so `takt` is documented as legacy compatibility rather than the normal execution layer.
- Latest verification for T005 implementation: `python3 -m pytest -q .claude/hooks/test_agent_profiles.py .claude/hooks/test_agent_router.py scripts/test_verify_scripts.py scripts/test_takt_profile.py`, `cargo run --quiet -p cli -- track views validate --project-root .`, and `cargo make ci` passed on the final uncommitted diff.
- T006 added `takt-removal-definition-of-done.md` to define the global post-`takt` completion gate, interpretation rule, required-path matrix, M1-M4 rollout gates, and remaining blockers.
- The DoD explicitly fixes success criteria for `/track:implement`, `/track:review`, `/track:commit`, PR flow, archive/registry updates, and the full `cargo make ci` gate without `takt` as a required execution layer.
- Latest verification for T006 implementation: read `takt-touchpoint-inventory.md`, `takt-runtime-removal-sequence.md`, and `takt-removal-definition-of-done.md`, then ran `cargo run --quiet -p cli -- track views validate --project-root .` and `cargo make ci`.

## Verified At

2026-03-13
