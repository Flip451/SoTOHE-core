# Rollout Definition of Done

Date: 2026-03-13
Track: `python-dependency-deprecation-2026-03-13`

## Global Definition of Done

`.venv` 未構築環境で、以下の必須経路が成功または既定どおり fail-closed / warn+exit0 になること。

- security-critical hooks が `python3` launcher なしで動く
- track workflow の必須操作が Rust CLI 経由で成立する
- CI の必須ゲートが `.venv` を前提にしない
- Python が残る要素は optional utility か、SSoT 再設計待ちの repository-policy check として明示分類されている

## Interpretation Rule

- `migration-map.md` は各 entrypoint の最終的な移行先を示す。
- `verification-boundary-classification.md` は現在時点で required path から外せているか、まだ Python に残す理由が何かを示す。
- この文書は rollout gate と required-path 判定を示す。
- したがって、`Rustへ移行` と書かれた entrypoint が当面 Python に残っていてもよい。ただし、その場合は `verification-boundary-classification.md` とこの文書で「なぜ今は Python に残すのか」が説明されていることを合格条件とする。

## Required Path Matrix

| Path | Required outcome without `.venv` | Current owner |
| --- | --- | --- |
| direct git block hook | fail-closed (`exit 2`) | `sotp hook dispatch block-direct-git-ops` |
| file-lock acquire hook | fail-closed (`exit 2`) | `sotp hook dispatch file-lock-acquire` |
| file-lock release hook | warn + `exit 0` | `sotp hook dispatch file-lock-release` |
| track transition / view sync | success | `sotp track transition`, `sotp track views validate/sync` |
| commit / note / branch wrappers | success | `sotp git ...` |
| PR status / merge wrappers | success | `sotp pr ...` |
| metadata / plan / registry validation | success | `sotp track views validate` |
| full CI gate | success | `cargo make ci` |

## M1

Goal:
- hook fail-closed semantics are preserved without Python launcher bootstrap

Exit criteria:
- `.claude/settings.json` の security-critical hook 3 本に `python3` 依存がない
- malformed / blocked invocation でも direct git block と file-lock acquire は `exit 2`
- file-lock release は launcher failure を warning に落として `exit 0`

Verification procedure:
1. `python3 -m json.tool .claude/settings.json`
2. `python3 scripts/verify_orchestra_guardrails.py`
3. `pytest -q -o cache_dir=.cache/pytest scripts/test_verify_scripts.py`
4. `cargo make ci`

Pass condition:
- hook command strings and regression testsがすべて通り、CI 全体でも hook contract の回帰がない

## M2

Goal:
- track workflow core と主要 wrapper が `.venv` なしで成立する

Exit criteria:
- `track transition`, `track views validate/sync`, `commit-from-file`, `note-from-file`, `switch-and-pull`, `pr status`, `pr wait-and-merge` が Rust CLI 経由
- branch guard と repo-root / metadata discovery が CLI local scan に依存しない

Verification procedure:
1. `cargo run --quiet -p cli -- track views validate --project-root .`
2. `cargo make track-sync-views`
3. `cargo test -p cli git -- --nocapture`
4. `cargo test -p cli pr -- --nocapture`
5. `cargo test -p infrastructure git_cli -- --nocapture`
6. `cargo test -p infrastructure gh_cli -- --nocapture`

Pass condition:
- workflow-critical wrapper tests が通り、track validation と adapter tests が Rust 側だけで成立する

## M3

Goal:
- CI の必須 gate が `.venv` に依存しないことを確認し、残留 Python を optional / deferred と切り分ける

Exit criteria:
- metadata / plan / registry validation は Rust CLI のみ
- verify script 群の required/deferred/optional 分類が文書化されている
- full CI が現在の required path で通る

Verification procedure:
1. Read `verification-boundary-classification.md`
2. `cargo test -p usecase git_workflow`
3. `cargo make ci-rust`
4. `cargo make ci`

Pass condition:
- required-path classification に矛盾がなく、full CI が green

## M4

Goal:
- `.venv` は optional utility 専用であり、track workflow の必須条件ではない状態を固定する

Exit criteria:
- remaining Python utilities are only repository-policy checks or advisory hooks
- `/track:review`, `/track:commit`, `/track:implement` の必須経路に `.venv` bootstrap 手順が不要
- rollout order と fallback policy が文書化されている

Verification procedure:
1. Read `migration-map.md`
2. Read `verification-boundary-classification.md`
3. Read this document
4. `cargo make ci`

Pass condition:
- migration map, verification classification, and DoD document agree on the eventual target and the current temporary Python boundary rationale

## Rollout Order

1. M1: security-critical hooks
2. M2: track workflow core and critical wrappers
3. M3: required CI gates and verification boundary classification
4. M4: residual Python demotion to optional utility

## Blockers That Still Require Separate Work

- `scripts/pr_review.py` remains a Python orchestration utility until async state persistence is redesigned
- `scripts/verify_orchestra_guardrails.py`, `scripts/verify_latest_track_files.py`, and `scripts/verify_tech_stack_ready.py` still depend on file-oriented SSoT outside the Rust track aggregate
- repository-policy utilities such as `scripts/check_layers.py` and `scripts/verify_architecture_docs.py` are not yet modeled as Rust domain/usecase checks
