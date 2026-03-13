# Verification: Python 依存脱却計画

## Scope Verified

- [x] migration map and phase plan are internally consistent
- [x] security-critical hook migration scope is explicit
- [x] `/track:plan` dependency on Python workflow core is captured
- [x] rollout milestones are defined
- [x] Rust `track views validate/sync` covers metadata decode and rendered view generation
- [x] `cargo make track-sync-views` no longer depends on Python
- [x] `cargo make track-transition` no longer depends on Python and post-syncs rendered views
- [x] track validation gates now execute via Rust CLI instead of Python verify wrappers
- [x] local git workflow wrappers (`add-paths`, `commit-from-file`, `note-from-file`, `switch-main`) are moved from Python wrappers to Rust CLI
- [x] PR status / merge wrappers (`track-pr-status`, `track-pr-merge`) no longer depend on `pr_merge.py`

## Manual Verification Steps

1. Read `migration-map.md`
2. Read `phase1-rust-direct-hooks-diff-plan.md`
3. Verify this track's `metadata.json`, `spec.md`, and `plan.md` align
4. Confirm the current branch matches `track/python-dependency-deprecation-2026-03-13`
5. Run `timeout 600 codex exec review --uncommitted --json --model gpt-5.4 --full-auto` until findings are `0`
6. Run `python3 -m json.tool .claude/settings.json`
7. Run `python3 scripts/verify_orchestra_guardrails.py`
8. Run `pytest -q -o cache_dir=.cache/pytest scripts/test_track_state_machine.py scripts/test_make_wrappers.py`
9. Run `cargo test -p infrastructure -- --nocapture`
10. Run `cargo run --quiet -p cli -- track views validate --project-root .`
11. Run `cargo make track-sync-views`
12. Run `cargo make ci`

## Result

Pass

## Open Issues

`cargo deny` reports an existing duplicate `windows-sys` warning, but the CI task still passes and this track did not change Rust dependencies.
Legacy archive generated views with relaxed schema fields are now normalized by Rust `track-sync-views`; this changed one archived `plan.md` as a consistency fix.
Current Rust migration leaves non-trivial workflow policy in CLI adapters (`apps/cli/src/commands/git.rs`, `apps/cli/src/commands/pr.rs`); next phase should extract stage path policy, branch guard, PR check evaluation, and git/gh execution boundaries into usecase/infrastructure.

## Review Notes

- Final `/track:review` loop closed with `No findings` after fixing wrapper contract drift, strict validation parity gaps, single-track sync partial-write behavior, and post-transition failure semantics.
- Verified that `cargo make track-transition` preserves the `<track_dir>` contract by deriving both `TRACK_ITEMS_DIR` and `TRACK_ID` from the supplied path.
- Verified that `sotp track transition` now treats rendered view sync as warning-only after persistence, so callers are not told the transition failed after `metadata.json` was already updated.
- Latest review evidence: `timeout 600 codex exec --model gpt-5.4 --sandbox read-only --full-auto ...` returned `No findings`.
- Latest CI evidence: `cargo make ci` passed on the final uncommitted diff.
- Current `/track:review` loop for T004 found and fixed two issues before re-verification: repo-root resolution for nested `cwd` calls, and unresolved relative `track-dir.txt` entries weakening branch-guard validation.
- Added nested-`cwd` Rust tests for `add-all`, `add-from-file`, `commit-from-file`, `note-from-file`, `switch-and-pull`, and `track-dir.txt` resolution to cover the new `sotp git` surface.
- Latest focused test evidence: `cargo test -p cli git -- --nocapture` passed with 10 tests after the fixes.
- Latest CI evidence for T004: `cargo make ci` passed after the branch-guard and coverage fixes.
- Added Rust `sotp pr` wrappers for `status` and `wait-and-merge`, and rewired `track-pr-status` / `track-pr-merge` away from `pr_merge.py`.
- Current `/track:review` loop for T005/T004 follow-up found and fixed two P1 issues: archived null-branch claims incorrectly satisfying the legacy branch fallback, and `gh pr checks` failures being downgraded to empty-check results.
- A subsequent `/track:review` pass found one more P1 in the Rust PR wrapper: `gh pr checks` non-zero exit codes for normal pending/failing states were being treated as fatal execution errors. The fix now accepts JSON output even on non-zero exits and prefers the `bucket` field when classifying pass/fail/pending.
- Added a usecase regression test for archived null-branch fallback rejection, and re-verified the Rust PR wrapper behavior with focused CLI tests.
- Latest focused test evidence: `cargo test -p usecase -- --nocapture`, `cargo test -p cli -- --nocapture`, `cargo test -p cli pr -- --nocapture`, `cargo clippy --locked -p cli --all-targets --all-features -- -D warnings`, and `pytest -q -o cache_dir=.cache/pytest scripts/test_make_wrappers.py` passed after the fixes.
- Latest CI evidence for T005/T004 review closeout: `cargo make ci` passed on the final uncommitted diff.

## In Progress

- Added `sotp git` subcommands for `add-all`, `add-from-file`, `commit-from-file`, `note-from-file`, and `switch-and-pull`.
- Added `sotp pr` subcommands for `status` and `wait-and-merge`.
- Switched the local git-oriented cargo-make wrappers away from `scripts/git_ops.py` / `scripts/branch_switch.py` to Rust CLI entry points.
- Switched `track-pr-status` / `track-pr-merge` away from `scripts/pr_merge.py` to Rust CLI entry points.
- Added wrapper assertions in `scripts/test_make_wrappers.py` and Rust unit coverage for git/PR wrapper logic.
- Started `T005` by extracting stage path policy and branch guard decision logic from CLI into `libs/usecase/src/git_workflow.rs`.
- `apps/cli/src/commands/git.rs` now delegates pure stage-path validation and branch-claim evaluation to the usecase layer; filesystem access and subprocess execution remain in CLI for now.
- Focused verification for the `T005` slice: `cargo test -p usecase -- --nocapture` and `cargo test -p cli git -- --nocapture` passed.

## Verified At

2026-03-13
