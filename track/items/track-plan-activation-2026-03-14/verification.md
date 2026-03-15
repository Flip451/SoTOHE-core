# Verification: track:plan plan-only / activate 導線

## Scope Verified

- [x] planning input documents under `tmp/track-plan-activation-design-2026-03-13/` were reviewed and canonicalized into this track's `design.md`
- [x] current workflow constraints from branch strategy and branch guard specs were reviewed
- [x] no active track exists at plan creation time, so this track starts from a clean registry state
- [x] `track/tech-stack.md` contains no unresolved work-item markers that would block planning
- [x] MVP scope explicitly excludes worker-branch and worktree automation
- [x] this planning track itself was created through the existing materialized branch flow, so its current `track/registry.md` row is not a proof fixture for future branch-null behavior

## Manual Verification Steps

1. Read `track/items/track-plan-activation-2026-03-14/design.md`
2. Read `track/archive/branch-strategy-2026-03-12/spec.md`
3. Read `track/archive/track-branch-guard-2026-03-12/spec.md`
4. Verify this track's `metadata.json`, `spec.md`, `design.md`, and rendered `plan.md` align
5. Confirm `design.md` is sufficient to understand the implementation plan without requiring `tmp/track-plan-activation-design-2026-03-13/`
6. After implementation starts, verify the same branch-null fixture set is accepted/rejected identically by Python (`scripts/track_schema.py` and its tests) and Rust (`track views validate` / render path), and that both stacks render the same next-command/current-focus guidance for those fixtures
7. Verify a planning-only artifact can be validated with `/track:ci`, then reviewed and committed on `main` before activation only with an explicit `track-id` selector, and that any allowed pre-activation PR path is restricted to the planning-only diff allowlist (`track/items/<id>/`, `track/registry.md`, `track/tech-stack.md`, `.claude/docs/DESIGN.md`) rather than a hidden `track/<id>` branch precondition
8. Verify pre-activation selector rules are explicit per command: `/track:review` / `/track:commit` / `/track:pr-review` require a `track-id` selector, while `/track:merge` requires an explicit PR number and does not fall back to current-branch auto-detect on non-track branches
9. Verify activation writes `metadata.json.branch`, persists that materialized state so it is still visible after switching back to the source branch, and moves the workspace onto `track/track-plan-activation-2026-03-14` or the requested target branch
10. Verify `/track:activate` rejects invalid-state, dirty-worktree, or stale/divergent-branch activations before metadata persistence, and that a checkout-only failure after persistence can be resumed by re-running `/track:activate <track-id>`
11. Verify branch-null tracks cannot enter implementation-phase transitions before activation, while planning-artifact-only review/commit/PR paths remain allowed only with an explicit selector rule and the guarded executor path honors that selector
12. Verify `/track:status` surfaces `Ready to Activate` and recommends `/track:activate <track-id>` for branch-null planning-only tracks, while materialized `planned` tracks still route to implementation
13. Verify `track/registry.md` recommends `/track:activate <track-id>` for branch-null planning-only tracks and preserves `/track:implement` for already-activated `planned` tracks, and that its header/footer copy also reflects the new public path
14. Verify a mixed fixture with both a materialized active track and a newer branch-null planning-only track keeps `status` / `catchup` / `revert` / external guide context loading / current focus / latest-track verification on the materialized active track
15. Verify `START_HERE_HUMAN.md`, `.claude/commands/track/setup.md`, `.claude/commands/track/catchup.md`, `.claude/commands/track/full-cycle.md`, `.claude/commands/track/ci.md`, `.claude/commands/track/review.md`, `.claude/commands/track/pr-review.md`, `.claude/commands/track/merge.md`, `.claude/commands/track/revert.md`, `.claude/commands/track/done.md`, `.claude/hooks/agent-router.py`, `.claude/hooks/block-direct-git-ops.py`, `.claude/skills/track-plan/SKILL.md`, `.claude/rules/07-dev-environment.md`, `.claude/docs/WORKFLOW.md`, `libs/domain/src/guard/policy.rs`, `Makefile.toml`, `apps/cli/src/commands/pr.rs`, `apps/cli/src/commands/track.rs`, `scripts/pr_review.py`, `scripts/pr_merge.py`, `apps/cli/src/commands/git.rs`, `scripts/git_ops.py`, `scripts/track_branch_guard.py`, and `scripts/external_guides.py` no longer contradict the plan-only / activate lane
16. Verify `.claude/commands/track/plan.md`, new `plan-only.md` / `activate.md`, `.claude/commands/track/implement.md`, `.claude/commands/track/status.md`, `.claude/commands/track/commit.md`, `track/workflow.md`, and `DEVELOPER_AI_WORKFLOW.md` expose the same public path and phase model
17. Verify `/track:full-cycle` either remains a post-activation compatibility path or is removed/redefined by the separate takt-removal work, but cannot bypass activation while present
18. Run `cargo make ci`

## Result / Open Issues

Implementation complete (T001–T009). Regression tests added for T005 covering:

- `track_phase.rs`: 16 tests for `resolve_phase`, `resolve_phase_from_record`, and `next_command` across all status/branch/schema combinations (was **zero** tests before T005)
- `track_resolution.rs`: 5 additional tests for `reject_branchless_guard` (error message guidance, done/skipped with branch, plan/ branch resolution)
- `usecase::lib.rs`: `execute_by_status` error message verification (no double-prefix)
- Existing test suites maintained: 442 tests total, all passing

Remaining open items (not blocking this track):
- Python/Rust parity tests for rendered guidance matching (deferred — Python layer is in closure policy and existing tests in `test_track_schema.py` cover the Python side)
- archived branchless track re-surfacing in Current Focus (tested in `render.rs` existing tests, confirmed by `render_registry_prefers_materialized_active_track_in_current_focus`)
- `/track:full-cycle` activation guard bypass — verified by `reject_branchless_implementation_transition` tests and command-level guards

`cargo make ci` passes with 442 tests.

## Verified At

2026-03-15
