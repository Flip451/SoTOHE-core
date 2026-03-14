# Verification: reviewer process termination

## Scope Verified

- [x] scratch proposal under `tmp/reviewer-process-termination-design-2026-03-14/` was reviewed before creating this track
- [x] current repo state has no active track on `main`, so this planning track starts from a clean registry state
- [x] existing reviewer guidance surfaces were inspected: `.claude/commands/track/review.md`, `.claude/agent-profiles.json`, `.claude/rules/02-codex-delegation.md`, `.claude/skills/codex-system/SKILL.md`
- [x] command permission surfaces were inspected: `Makefile.toml` and `.claude/settings.json`
- [x] `codex exec --help` and `codex exec review --help` were inspected locally to confirm the current CLI contract around `--output-last-message`, `--full-auto`, and reviewer subcommand options
- [x] `track/tech-stack.md` contains no unresolved work-item markers that would block planning
- [x] PR review posting flow and Python review scripts are explicitly outside this MVP
- [x] reviewer final payload contract was updated from a raw sentinel to shape-enforced JSON via `--output-schema`, with wrapper-side semantic validation for verdict/findings consistency
- [x] Rust targeted tests passed for `usecase::review_workflow` and `commands::review::tests`
- [x] verifier regression tests passed for `scripts/test_verify_scripts.py -k verify_orchestra_guardrails`
- [x] `cargo make ci` passed after the JSON-schema reviewer changes

## Manual Verification Steps

1. Read `track/items/reviewer-process-termination-2026-03-14/design.md`
2. Read `tmp/reviewer-process-termination-design-2026-03-14/reviewer-process-termination-proposal-2026-03-14.md`
3. Verify `metadata.json`, `spec.md`, `design.md`, and rendered `plan.md` align on the same wrapper-based approach
4. After implementation starts, verify `apps/cli` exposes the new `review codex-local` path and that `/track:review` uses it when the active reviewer provider is `codex`
5. Verify the wrapper runs the reviewer path in read-only mode, uses `--output-schema` for the final payload, and does not use `--full-auto` in the canonical local reviewer execution path
6. Verify timeout kills the child process and returns a deterministic failure result
7. Verify final message handling parses the final JSON payload, maps `{"verdict":"zero_findings","findings":[]}` to success, and sends malformed / ambiguous payloads to explicit failure buckets
8. Verify `Makefile.toml`, `.claude/settings.json`, `.claude/agent-profiles.json`, `.claude/commands/track/review.md`, `.claude/rules/02-codex-delegation.md`, and `.claude/skills/codex-system/SKILL.md` all point at the same wrapper contract
9. Verify regression tests cover both the Rust wrapper lifecycle and stale-guidance detection
10. Run `cargo make ci`

## Result / Open Issues

Pass.

Implemented and verified:
- `review codex-local` now launches `codex exec` in read-only mode with `--output-schema` and `--output-last-message`
- final reviewer verdict is derived from validated JSON payloads instead of a raw `NO_FINDINGS` sentinel
- malformed / ambiguous JSON or verdict/findings inconsistency fails closed as `process_failed`
- public guidance and verifier guardrails were updated to describe the schema-enforced wrapper contract

Open issues:
- none in this MVP scope

## Verified At

2026-03-14
