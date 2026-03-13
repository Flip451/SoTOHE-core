# Takt Removal Definition of Done

Date: 2026-03-13
Track: `takt-removal-2026-03-13`

## Global Definition of Done

この repo が `takt` を正式な実行レイヤとして一切前提にせず、以下の必須経路が Claude Code +
Rust CLI + 通常の git/GitHub 操作だけで成立すること。

- `/track:*` の planning / implement / review / commit / merge / done が `takt-*` wrapper なしで閉じる
- `cargo make ci` が `takt` runtime・queue・persona・profile bootstrap を要求しない
- pending artifact / git note / handoff scratch は `tmp/track-commit/*` など非 `takt` scratch 契約で閉じる
- archive/registry/SSoT view 更新が `.takt/**` を参照せずに成立する
- `takt` に関する残存要素がある場合、それは migration cleanup の削除対象として明示されている

## Interpretation Rule

- `takt-touchpoint-inventory.md` は、何が removal scope かと sequencing constraint を定義する。
- `takt-runtime-removal-sequence.md` は、どの surface をどの順番で消すかを定義する。
- この文書は、いつ「`takt` 廃止が完了した」と見なすかの rollout gate を定義する。
- したがって、inventory や removal sequence に載っている surface が一時的に repo に残っていてもよい。ただし、その surface がこの文書の milestone 上でどこまでに消えるか、または migration-only compatibility としてどこまで許容されるかが説明されていることを合格条件とする。

## Required Path Matrix

| Path | Required outcome after `takt` removal | Current owner |
| --- | --- | --- |
| `/track:plan` and track transition/view sync | success without `takt-*` wrapper or `.takt/config.yaml` | `sotp track transition`, `sotp track views validate/sync` |
| `/track:implement` orchestration | success without `cargo make takt-full-cycle` | Claude Code + `.claude/commands/track/*.md` |
| `/track:review` | success without queue/piece runtime or `.takt/handoffs` | `codex exec review`, `verification.md` |
| `/track:commit` | success with `tmp/track-commit/*` scratch and git notes, no `.takt/pending-*` requirement | `sotp git ...`, `cargo make track-note`, `cargo make track-commit-message` |
| PR push / ensure / status / review / merge | success without `takt` wrappers or profile bootstrap | `cargo make track-pr-*`, `sotp pr ...` |
| archive / registry / track closeout | success without `.takt/**` state | `sotp track views validate/sync`, `/track:done` |
| full CI gate | success without `scripts/test_takt_*`, `TAKT_PYTHON`, or `takt` runtime assets | `cargo make ci` |
| docs / guardrails / profile schema | no baseline rule or onboarding path requires `takt` | `.claude/settings.json`, docs, hook/profile helpers |

## M1

Goal:
- public workflow, onboarding, guardrail baseline, and scratch-path guidance stop treating `takt`
  as the normal path

Exit criteria:
- `/track:*` docs and top-level workflow docs describe Claude Code + Rust CLI execution rather than
  `takt` execution
- `.claude/settings.json` and guardrail verifier no longer keep `takt-*` wrapper permissions as
  baseline
- `tmp/track-commit/*` is the primary scratch contract for add/message/note artifacts

Verification procedure:
1. Read `.claude/commands/track/full-cycle.md`, `.claude/commands/track/setup.md`, and `track/workflow.md`
2. Read `DEVELOPER_AI_WORKFLOW.md`, `LOCAL_DEVELOPMENT.md`, and `START_HERE_HUMAN.md`
3. Read `pending-artifact-cutover.md`
4. Run `python3 -m pytest -q .claude/hooks/test_agent_profiles.py .claude/hooks/test_agent_router.py scripts/test_verify_scripts.py scripts/test_takt_profile.py`
5. Run `cargo run --quiet -p cli -- track views validate --project-root .`

Pass condition:
- docs, settings, and scratch guidance all agree that `takt` is migration-only compatibility and
  no longer the primary workflow path

## M2

Goal:
- remaining runtime and wrapper surfaces are either scheduled for deletion or already removed in a
  way that leaves `/track:*` intact

Exit criteria:
- every `takt-*` wrapper, `.takt/**` runtime asset, `scripts/takt_profile.py`, and
  `scripts/takt_failure_report.py` has a fixed delete-or-generalize decision
- no required user flow still depends on `cargo make takt-full-cycle`, queue assets, or persona
  rendering
- the runtime deletion order is explicit enough to remove wrappers/tests without re-deciding
  sequence

Verification procedure:
1. Read `takt-touchpoint-inventory.md`
2. Read `takt-runtime-removal-sequence.md`
3. Inspect `Makefile.toml` for remaining `takt-*` entries and confirm each one maps to a named phase
4. Run `cargo run --quiet -p cli -- track views validate --project-root .`

Pass condition:
- every remaining `takt` runtime surface is accounted for, and no required `/track:*` path depends
  on an undefined compatibility layer

## M3

Goal:
- review, commit, PR, and archive workflows are demonstrably independent from `takt`

Exit criteria:
- `/track:review` closes via reviewer output + `verification.md`, not `.takt/handoffs` or queue recovery
- `/track:commit` uses `tmp/track-commit/*` and git notes without `.takt/pending-*` as a required input
- PR wrappers and merge flow use `sotp git` / `sotp pr` and standard GitHub operations only
- registry/archive updates do not require `.takt/**` state files

Verification procedure:
1. Read `.claude/commands/track/commit.md`
2. Read `track/workflow.md`
3. Run `cargo test -p usecase git_workflow -- --nocapture`
4. Run `cargo test -p cli git -- --nocapture`
5. Run `cargo test -p cli pr -- --nocapture`
6. Run `pytest -q -o cache_dir=.cache/pytest scripts/test_git_ops.py scripts/test_make_wrappers.py`
7. Run `cargo run --quiet -p cli -- track views validate --project-root .`

Pass condition:
- commit/review/PR/archive paths are covered by current docs and tests without requiring any
  `takt` runtime or pending-artifact contract

## M4

Goal:
- final repo gate proves that `takt` can be removed without leaving required CI or docs in an
  inconsistent state

Exit criteria:
- `cargo make ci` no longer depends on `test_takt_*`, `TAKT_PYTHON`, or runtime-specific assets
- any remaining `takt` reference in docs/tests/rules is explicitly migration-only or marked for
  immediate deletion in the same phase
- this document, the inventory, and the removal sequence agree on the final post-`takt` state

Verification procedure:
1. Read `takt-touchpoint-inventory.md`
2. Read `takt-runtime-removal-sequence.md`
3. Read this document
4. Run `cargo make ci`

Pass condition:
- the artifact set agrees on the end state and the full repository gate passes without hidden
  `takt` prerequisites

## Rollout Order

1. M1: docs, guardrails, and scratch contract no longer treat `takt` as normal
2. M2: runtime/wrapper removal order is fixed and required flows are decoupled
3. M3: commit/review/PR/archive paths are proven independent from `takt`
4. M4: full repo gate validates the final post-`takt` state

## Blockers That Still Require Separate Work

- `T004` fixed the deletion order, but the actual removal of `Makefile.toml` `takt-*` wrappers,
  `.takt/**`, and `scripts/test_takt_*` still requires implementation work outside this planning track
- legacy compatibility aliases such as `takt_host_*` remain until the runtime/profile removal phase
  lands
- `scripts/takt_failure_report.py` still needs an explicit delete-vs-generalize decision during the
  runtime cleanup phase
