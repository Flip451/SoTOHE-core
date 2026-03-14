# Spec: local reviewer の終端制御を Rust wrapper に集約する

## Goal

`/track:review` の local reviewer loop を repo-owned Rust wrapper 経由に統一し、
reviewer child process の timeout / 終端 / verdict 判定を repo 側で fail-closed に制御する。
同時に、reviewer は read-only contract のまま動くことを docs / config / tests まで含めて揃える。

## Scope

- `apps/cli` に local reviewer 用 Rust subcommand を追加する
- `Makefile.toml` と `.claude/settings.json` に reviewer wrapper の runnable path を追加する
- reviewer local path では raw `codex exec review --full-auto` を canonical path から外す
- `/track:review` と `.claude/agent-profiles.json` reviewer invoke example を同じ wrapper path に揃える
- `.claude/rules/02-codex-delegation.md` と `.claude/skills/codex-system/SKILL.md` を reviewer wrapper 契約へ同期する
- reviewer lifecycle / final verdict / docs/config sync の regression tests を追加する

## Non-Goals

- `@codex review` を使う PR review cycle の再設計
- `scripts/pr_review.py` の structured PR review flow の変更
- non-Codex reviewer provider support
- reviewer findings schema の全面再設計
- planner / implementer / debugger capability 全体の wrapper 化

## Constraints

- user-facing の primary command は引き続き `/track:review` とする
- local reviewer path では Python を増やさない
- local reviewer path は read-only contract を維持し、`--full-auto` を canonical path に含めない
- wrapper は repo 側で timeout を管理し、child process の hanging を放置しない
- wrapper は `--output-schema` を使って final reviewer payload の shape を固定する
- verdict は少なくとも `zero_findings` / `findings_remain` / `timeout` / `process_failed` / `last_message_missing` に正規化する
- final message は single JSON object を正本とし、MVP の top-level `verdict` は `zero_findings` / `findings_remain` のみを許可する
- `--output-schema` は JSON shape を固定し、wrapper は `zero_findings => empty findings` / `findings_remain => non-empty findings` の semantic validation も行う
- malformed / ambiguous JSON や verdict/findings の不整合は fail-closed で `process_failed` 側に倒す
- `codex exec review` は MVP の実行基盤に必須としない。明示 read-only 制御を優先して generic `codex exec` path を採用してよい
- `track/tech-stack.md` に未解決の作業メモ marker がない状態を維持する

## Canonical Design

- `track/items/reviewer-process-termination-2026-03-14/design.md`
- `tmp/reviewer-process-termination-design-2026-03-14/reviewer-process-termination-proposal-2026-03-14.md`

## Acceptance Criteria

- [ ] `apps/cli` に repo-owned local reviewer wrapper (`review codex-local`) が追加され、CLI command tree に組み込まれている
- [ ] wrapper は reviewer child process を Rust 側で spawn し、timeout を管理し、timeout 時に child を終了させて non-zero を返す
- [ ] wrapper は `--output-last-message` を使って final message を回収し、single JSON object を parse し、last stdout line として同じ JSON payload を返せる
- [ ] wrapper は malformed / ambiguous JSON final payload を fail-closed で弾き、final message と process result を `zero_findings` / `findings_remain` / `timeout` / `process_failed` / `last_message_missing` に正規化する
- [ ] local reviewer path の canonical execution は read-only contract を保ち、`--full-auto` を canonical path に含めない
- [ ] `.claude/agent-profiles.json` の Codex reviewer invoke example と `.claude/commands/track/review.md` が同じ wrapper path (`cargo make track-local-review -- ...`) を指し、reviewer final output schema が JSON contract で明文化されている
- [ ] `Makefile.toml` と `.claude/settings.json` が reviewer wrapper の runnable path を expose し、Claude Code の command allowlist と矛盾しない
- [ ] `.claude/rules/02-codex-delegation.md` と `.claude/skills/codex-system/SKILL.md` が reviewer wrapper path と read-only contract に同期している
- [ ] Rust test と docs/config verification (`.claude/hooks/test_agent_profiles.py`, `scripts/verify_orchestra_guardrails.py`, `scripts/test_verify_scripts.py` を含む) が reviewer wrapper path と stale guidance を回帰防止する
- [ ] `cargo make ci` が通る
