# Codex CLI - SoTOHE Orchestrator

This repository supports both Claude Code and Codex CLI as permanent root orchestrator choices.
The active root provider is selected by `.harness/config/agent-profiles.json` at
`capabilities.orchestrator.provider`.

When that provider is `codex`, act as the SoTOHE root orchestrator. When a specialist capability is
assigned to Codex, act only within that specialist boundary.

## Operating Context

Read these first:

- `AGENTS.md`
- `.harness/policies/branch-strategy.md`
- `.harness/policies/track-lifecycle.md`
- `.harness/policies/git-notes.md`
- `knowledge/adr/README.md` (pre-track ADR index — tech stack / product-policy decisions)
- `track/registry.md`
- `knowledge/conventions/README.md`
- current `track/items/<id>/metadata.json`
- current `track/items/<id>/spec.json`, if present
- current `track/items/<id>/<layer>-types.json`, if present
- current `track/items/<id>/impl-plan.json`, `track/items/<id>/task-coverage.json`, and `track/items/<id>/task-contract.json`, if present
- current `track/items/<id>/spec.md` and `track/items/<id>/plan.md`, if present
- `.claude/rules/`
- `.codex/rules/default.rules` (the Codex-specific command-policy surface — read it when running as the Codex root host)
- `architecture-rules.json`

If `knowledge/conventions/` contains a domain-specific convention for the work, treat it as binding.

## Root Orchestrator Rules

- Keep the public `/track:*` workflow stable regardless of whether Claude Code or Codex is the root host.
- Use the existing SoTOHE phase commands and `cargo make` wrappers.
- Do not introduce a second profile layer. Provider routing stays in `capabilities.<name>.provider`.
- Keep Phase 1, Phase 2, Phase 3, ADR edit, review-fix, and dry-fix ownership separate.
- Prefer Codex custom agents plus `.agents/skills` when the corresponding capability is assigned to Codex.
- Do not persist references to scratch / runtime / cache files (e.g. under `tmp/`) as architectural authority. The tracked repo-local surfaces intentionally provided here — `.codex/*`, `.agents/skills`, `.harness/capabilities` — ARE authoritative.

## Specialist Routing

Capability mapping comes from `.harness/config/agent-profiles.json`. Each specialist capability's full
operational contract lives in a single provider-agnostic SSoT at `.harness/capabilities/<name>.md`;
the Codex skill (`.agents/skills/<name>/SKILL.md`) and the Claude subagent (`.claude/agents/<name>.md`)
are thin wrappers that reference it. Read that SSoT when acting as a specialist.

- `orchestrator`: overall workflow coordination.
- `spec-designer`: writes `spec.json`; use the `spec-designer` skill.
- `type-designer`: writes per-layer type catalogues; use the `type-designer` skill.
- `impl-planner`: writes `impl-plan.json`, `task-coverage.json`, and `task-contract.json`; use the `impl-planner` skill.
- `adr-editor`: edits target ADRs during back-and-forth planning; use the `adr-editor` skill.
- `implementer`: edits source code within the current task.
- `reviewer`: reviews correctness and safety only.
- `review-fix-lead`: fixes actionable review findings; use the existing `review-fix-lead` skill.
- `dry-fix-lead`: fixes DRY findings; use the existing `dry-fix-lead` skill.
- `rollback-diagnoser`: diagnose-only specialist invoked by `/track:diagnose` when an impl-phase or later finding (PreReviewGate Blocked, SoT-scope review finding on adr/spec/types/impl-plan, external PR-reviewer comment) needs phase-rollback routing; returns a structured `{routing_target, reason, recommended_next_action}` decision the orchestrator dispatches. Never edits any SoT artifact; the dispatch belongs to the orchestrator. Use the `rollback-diagnoser` skill.
- `researcher`: follows the provider assigned in the capability map.

## Command Policy

Use the gate aggregates via `cargo make` and single workflow operations via guarded `bin/sotp` commands:

- `cargo make ci`
- `cargo make ci-rust`
- `cargo make track-commit-message`
- `cargo make track-local-review-fix`
- `cargo make track-local-review`
- `cargo make track-local-dry-fix`
- `bin/sotp git add-all`
- `bin/sotp git add-from-file tmp/track-commit/add-paths.txt --cleanup`
- `bin/sotp git note-from-file tmp/track-commit/note.md --cleanup`
- `bin/sotp pr push`
- `bin/sotp pr ensure-pr`
- `bin/sotp pr review-cycle`

Allowed direct Git usage is read-only inspection such as `git status`, `git diff`, `git log`,
`git show`, `git rev-parse`, `git ls-files`, and `git notes show/list`.

Do not run direct Git mutation commands. Do not run direct Codex review commands for SoTOHE review
gates. Use the project wrappers so review state, commit gates, and traceability remain under the
repository workflow.

## Hook And Trust Requirements

Project-local `.codex` config, rules, hooks, agents, and repo-scoped skills are intended for trusted
project checkouts. In an untrusted checkout, user/system Codex settings may be the only active layer.
When onboarding a clone, make the project trusted before relying on these repo-local guardrails.

Codex hooks must call `.codex/hooks/sotp-hook.sh`, which delegates to `bin/sotp hook dispatch`.
Policy belongs in SoTOHE hook dispatch, not in the shell adapter.

## Rust Guidelines

- No panics in production library code.
- Prefer validated domain types over raw primitives for domain concepts.
- Propagate errors with `Result` and `?`.
- Keep infrastructure behind trait boundaries.
- Preserve hexagonal layer dependencies from `architecture-rules.json`.
- Add focused tests for public behavior and failure cases.

## Session resume

Session resume is orchestrator opt-in (`sotp capability exec --resume`; reviewer rounds resume
automatically for same-scope same-round re-entries). On both resumed and fresh dispatches every
execution flag (model, sandbox, effort) is explicitly re-specified; a failed or expired resume
falls back to a fresh session. When you run as a resumed session, check whether the upstream
artifacts of your assignment changed since the prior session and re-read any that did before
working (see `.harness/prompts/capability-exec-discipline.md`).

## Output

For user-facing replies, be concise and direct. For task work, report:

- files changed
- verification commands run
- remaining risks or skipped checks
