<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# local reviewer の終端制御を Rust wrapper に集約する

local reviewer の child process lifecycle と最終 verdict 判定を repo-owned Rust wrapper へ寄せる。
`/track:review` と Codex reviewer invoke example を同じ wrapper path に揃え、raw `codex exec review --full-auto` 依存を外す。
read-only reviewer 契約と終端挙動を docs/config/tests まで含めて固定し、`cargo make ci` で回帰を防ぐ。

## Phase 1: Rust reviewer wrapper

- [x] Update the repo-owned Rust local reviewer wrapper (`review codex-local`) so `--output-last-message` is parsed as a structured JSON final payload, malformed or ambiguous payloads fail closed, and verdicts are normalized into `zero_findings` / `findings_remain` / `timeout` / `process_failed` / `last_message_missing` without relying on a raw `NO_FINDINGS` sentinel

## Phase 2: Public path and guidance sync

- [x] Route `/track:review` and `.claude/agent-profiles.json` reviewer invoke examples through the Rust wrapper so the public review loop and provider example use the same repo-owned execution path (`cargo make track-local-review -- ...`), and ensure the wrapper-enforced `--output-schema` contract requires a single final JSON object rather than probabilistic free-form success text
- [x] Update Codex delegation guidance and runnable wrapper surfaces (`Makefile.toml`, `.claude/settings.json`, `.claude/rules/02-codex-delegation.md`, `.claude/skills/codex-system/SKILL.md`, and any directly affected reviewer guidance) so local reviewer instructions match the wrapper path, keep reviewer read-only, and document the structured JSON final payload contract instead of the old raw sentinel convention

## Phase 3: Regression coverage and CI

- [x] Add regression coverage for JSON payload parsing and the synced guidance surfaces: Rust integration/unit tests for reviewer lifecycle handling and malformed payload rejection, `.claude/hooks/test_agent_profiles.py`, `scripts/verify_orchestra_guardrails.py`, `scripts/test_verify_scripts.py`, and any necessary orchestration tests; then run `cargo make ci`
