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
- [x] `git` / `gh` / repo-root adapter boundaries are explicit infrastructure modules instead of CLI-local subprocess helpers
- [x] track metadata branch-claim discovery now lives in infrastructure instead of CLI-local filesystem scans
- [x] `pr_review.py` and verification scripts are classified by required-path status in `verification-boundary-classification.md`
- [x] `.venv`-independent Definition of Done and M1-M4 rollout verification are defined in `rollout-definition-of-done.md`

## Manual Verification Steps

1. Read `migration-map.md`
2. Read `phase1-rust-direct-hooks-diff-plan.md`
3. Read `verification-boundary-classification.md`
4. Read `rollout-definition-of-done.md`
5. Verify this track's `metadata.json`, `spec.md`, and `plan.md` align
6. Confirm the current branch matches `track/python-dependency-deprecation-2026-03-13`
7. Run `timeout 600 codex exec review --uncommitted --json --model gpt-5.4 --full-auto` until findings are `0`
8. Run `python3 -m json.tool .claude/settings.json`
9. Run `python3 scripts/verify_orchestra_guardrails.py`
10. Run `pytest -q -o cache_dir=.cache/pytest scripts/test_track_state_machine.py scripts/test_make_wrappers.py`
11. Run `cargo test -p infrastructure -- --nocapture`
12. Run `cargo run --quiet -p cli -- track views validate --project-root .`
13. Run `cargo make track-sync-views`
14. Run `cargo make ci`

## Result

Pass

## Open Issues

`cargo deny` reports an existing duplicate `windows-sys` warning, but the CI task still passes and this track did not change Rust dependencies.
Legacy archive generated views with relaxed schema fields are now normalized by Rust `track-sync-views`; this changed one archived `plan.md` as a consistency fix.
`rollout-definition-of-done.md` fixes the rollout gate definition, but it is still a track artifact rather than an enforced machine-readable policy.
`migration-map.md` records eventual migration targets, while `verification-boundary-classification.md` records the temporary current-state Python boundary; `rollout-definition-of-done.md` now makes that distinction explicit.

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
- Current `/track:review` loop for T006 started with three P1 findings: `pr_workflow` still depended on GitHub transport strings, the CLI adapter lost meaningful wrapper coverage after extraction, and `metadata.json` had `updated_at` earlier than `created_at`.
- Fixed T006 by normalizing GitHub check state in CLI before crossing into `usecase`, adding injectable seams for `gh` execution and sleep behavior in `pr.rs`, adding CLI regression tests for non-zero JSON handling, stderr propagation, merge invocation, and timeout behavior, and correcting the track timestamp ordering.
- Latest reviewer verdict for T006: `No P0/P1 findings in the remaining diff.`
- Latest focused verification for T006: `cargo test -p usecase pr_workflow -- --nocapture`, `cargo test -p cli pr -- --nocapture`, `cargo clippy --locked -p cli --all-targets --all-features -- -D warnings`, `cargo make track-sync-views`, and `cargo make ci-rust` passed after the fixes.
- Latest full CI evidence for T006 review closeout: `cargo make ci` passed on the final uncommitted diff.
- Follow-up implementation moved `gh` execution / PR JSON decode into `libs/infrastructure/src/gh_cli.rs` and `git` execution / repo-root resolution into `libs/infrastructure/src/git_cli.rs`, leaving `apps/cli/src/commands/pr.rs` and `apps/cli/src/commands/git.rs` as thinner adapters around usecase + infrastructure.
- Latest focused verification for the adapter extraction slice: `cargo test -p infrastructure -- --nocapture`, `cargo test -p cli git -- --nocapture`, `cargo test -p cli pr -- --nocapture`, `cargo clippy --locked -p cli --all-targets --all-features -- -D warnings`, `cargo make track-sync-views`, and `cargo make ci` passed.
- A subsequent `/track:review` pass found one P1 coverage gap after the extraction: `libs/infrastructure/src/gh_cli.rs` owned fail-closed `gh` transport behavior without direct regression tests for non-zero JSON handling, stderr surfacing, and merge failure propagation.
- Fixed that gap by extracting testable helper seams inside `gh_cli.rs` and adding infrastructure regression tests for all three transport contracts; the re-review verdict was `No findings.` and `cargo make ci-rust` plus `cargo make ci` passed afterward.
- `T007` moved track branch-claim discovery from `apps/cli/src/commands/git.rs` into `libs/infrastructure/src/git_cli.rs`, so CLI no longer scans `track/items` / `track/archive` directly.
- Added `verification-boundary-classification.md` to classify `pr_review.py`, `check_layers.py`, `verify_orchestra_guardrails.py`, and the remaining `verify_*` scripts into required Rust path, deferred-until-SSoT-redesign, and optional Python utility buckets.
- Focused verification for the `T007` slice: `cargo test -p infrastructure git_cli -- --nocapture`, `cargo test -p cli git -- --nocapture`, `pytest -q -o cache_dir=.cache/pytest scripts/test_make_wrappers.py`, `cargo clippy --locked -p cli --all-targets --all-features -- -D warnings`, `cargo make track-sync-views`, and `cargo make ci` passed.
- A follow-up `/track:review` pass for T007 found one P1 in `libs/infrastructure/src/git_cli.rs`: `collect_track_branch_claims()` swallowed malformed `metadata.json` files and could misreport branch ownership instead of failing closed.
- Fixed that review finding by propagating `read_metadata()` errors from `collect_track_branch_claims()` and adding a regression test for invalid track metadata.
- Latest reviewer verdict for the T007 fix was `No findings.` after re-checking `apps/cli/src/commands/git.rs` and `libs/infrastructure/src/git_cli.rs`.
- Latest focused verification for the review fix: `cargo test -p infrastructure git_cli -- --nocapture`, `cargo test -p usecase git_workflow`, `cargo make ci-rust`, and `cargo make ci` passed.
- `T008` added `rollout-definition-of-done.md` to define the `.venv`-independent required-path matrix, exact M1-M4 exit criteria, verification procedures, and rollout order.
- Verification for the T008 documentation slice: `cargo run --quiet -p cli -- track views validate --project-root .`, `cargo make track-sync-views`, and `cargo make ci` passed after the new rollout artifact was added.
- A follow-up `/track:review` pass for T008 found one P1 inconsistency: `rollout-definition-of-done.md` described migration-map, verification-boundary classification, and DoD as if they shared the same current-state bucket model, even though `migration-map.md` records eventual targets while the others describe temporary Python boundaries.
- Fixed that review finding by adding an explicit interpretation rule to `rollout-definition-of-done.md` and recording the distinction in `verification.md`.
- Latest reviewer verdict for the T008 fix was `No findings.`
- Latest focused verification for the review fix: `cargo run --quiet -p cli -- track views validate --project-root .` and `cargo make ci-rust` passed.

## In Progress

- Added `sotp git` subcommands for `add-all`, `add-from-file`, `commit-from-file`, `note-from-file`, and `switch-and-pull`.
- Added `sotp pr` subcommands for `status` and `wait-and-merge`.
- Switched the local git-oriented cargo-make wrappers away from `scripts/git_ops.py` / `scripts/branch_switch.py` to Rust CLI entry points.
- Switched `track-pr-status` / `track-pr-merge` away from `scripts/pr_merge.py` to Rust CLI entry points.
- Added wrapper assertions in `scripts/test_make_wrappers.py` and Rust unit coverage for git/PR wrapper logic.
- Completed `T005` by extracting stage path policy and branch guard decision logic into `libs/usecase/src/git_workflow.rs`, and moving `git` execution plus repo-root resolution into `libs/infrastructure/src/git_cli.rs`.
- `apps/cli/src/commands/git.rs` now delegates pure validation to usecase and subprocess/path resolution to infrastructure; CLI keeps file cleanup and user-facing exit code translation.
- Completed `T006` by keeping PR check summarization and wait/timeout decisions in `libs/usecase/src/pr_workflow.rs`, while moving `gh` execution and PR JSON decode into `libs/infrastructure/src/gh_cli.rs`.
- `apps/cli/src/commands/pr.rs` now normalizes infrastructure records into usecase statuses and keeps only presentation / polling glue.

## Verified At

2026-03-13
