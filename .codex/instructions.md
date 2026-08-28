# Codex CLI - SoTOHE Orchestrator

This repository supports both Claude Code and Codex CLI as permanent root orchestrator choices.
The active root provider is selected by `.harness/config/agent-profiles.json` at
`capabilities.orchestrator.provider`.

When that provider is `codex`, act as the SoTOHE root orchestrator. When a specialist capability is
assigned to Codex, act only within that specialist boundary.

## Root Orchestrator Rules (always applied)

This file is the Codex root orchestrator's concise rule surface. PR-review briefings are loaded
by the review workflow and are not standing orchestrator instructions.

- Delegate implementation, planning, review-fix, and other specialist work through
  `bin/sotp capability exec` or the provider wrappers; keep workflow control in the root session.
- Route all implementation work in this repository through a track workflow with Phase 0–3
  planning complete before implementation: use `$track-plan` for standalone feature planning,
  while end-to-end workflows such as `$track-adr2pr` own equivalent planning sequencing. Do not
  implement directly from a free-form request.
- Treat CLI summaries as the primary information for progress, review necessity, obligation
  state, and catalogue state: `bin/sotp track resolve`, `bin/sotp track task-counts`,
  `bin/sotp track next-task`, `bin/sotp review results`, `bin/sotp test-obligation results`, `bin/sotp catalog check`,
  `bin/sotp ref-verify results`. Open an artifact body (`spec.json`, `*-types.json`,
  `impl-plan.json`, `review.json`, bindings JSON, full workflow texts, convention lists) only to
  inspect a diff or investigate a blocker; the delegated capability reads the paths its briefing
  lists. The `adr2pr` workflow's mandatory Step 0 is a bounded exception: read each
  sub-workflow definition it enumerates to build the execution plan before execution — required
  workflow planning, not general bulk intake. The typed-pipeline local reviewer and
  review-fix-lead routes are a second bounded exception: the review workflow itself resolves the
  applicable convention paths from the track's declared convention references and the
  consumer-owned `knowledge/conventions/README.md` `Current Files` index while preparing the
  briefing, and supplies them to the delegated reviewer or fixer.
- Do not run direct Git mutations. Use the guarded workflow commands in the Command Policy
  below; read-only Git inspection is permitted.

## Operating Context

Start from `AGENTS.md` and the CLI summaries above. Read `.codex/rules/default.rules` (the
Codex-specific command-policy surface) when running as the Codex root host, and open
`.harness/policies/branch-strategy.md`, `.harness/policies/track-lifecycle.md`,
`.harness/policies/git-notes.md`, `knowledge/adr/README.md`, `knowledge/conventions/README.md`,
or `architecture-rules.json` only when the current step needs that rule. Detailed Claude-side
references under `.claude/rules/` are not loaded by default.

If `knowledge/conventions/` contains a domain-specific convention for the work, treat it as binding
for the delegated capability that the dispatcher lists it for.

## Workflow Rules

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
- `impl-planner`: sole writer of `impl-plan.json`, `task-coverage.json`, `task-contract.json`, and `batch-plan.json`; use the `impl-planner` skill.
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
- `cargo make track-views-sync`
- `bin/sotp git add-all`
- `bin/sotp git add-from-file tmp/track-commit/add-paths.txt --cleanup`
- `bin/sotp git note-from-file tmp/track-commit/note.md --cleanup`
- `bin/sotp pr push`
- `bin/sotp pr ensure-pr`
- `bin/sotp pr review-cycle`
- `bin/sotp capability exec <capability> --briefing-file <path>` (the primary delegation route;
  omit `--host` from a Codex root so the dispatcher runs the provider subprocess itself)
- `bin/sotp phase enter spec-design|type-design|impl-plan` (phase-writer entry)
- `bin/sotp track transition <task-id> <state> [--commit-hash <hash>]` (task-state transitions
  are performed only by the root orchestrator session, at the points the full-cycle workflow
  SSoT fixes)
- read-only summary intake (the primary information named in the Root Orchestrator Rules):
  `bin/sotp track resolve`, `bin/sotp track task-counts`, `bin/sotp track next-task`,
  `bin/sotp review results`, `bin/sotp test-obligation results`, `bin/sotp catalog check`,
  `bin/sotp ref-verify results`

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
