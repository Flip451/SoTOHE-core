# Orchestration

Claude Code is the orchestrator.

- User-facing interface: `/track:*`
- Context management: `track/`
- Capability routing: `.harness/config/agent-profiles.json`
- Default capability assignment:
  - `orchestrator` / `spec-designer` / `impl-planner` / `type-designer` / `adr-editor` / `implementer`: Claude Code
  - `reviewer`: Codex CLI
  - `researcher`: Gemini CLI
- Parallel execution: Agent Teams

Host orchestration stays in Claude Code. Specialist capabilities may switch as models evolve, but the public `/track:*` interface should remain stable.

Terms:

- `track`: `metadata.json` (identity SSoT) / `spec.json` (Phase 1 behavioral contract SSoT) / `<layer>-types.json` (Phase 2 type-contract SSoT) / `impl-plan.json` + `task-coverage.json` (Phase 3 implementation plan SSoT) / `spec.md` / `plan.md` / `verification.md` (read-only rendered views) / progress management layer

## Source Of Truth

Read these first before planning or implementation:

- `track/tech-stack.md`
- `track/workflow.md`
- `track/registry.md`
- `knowledge/conventions/README.md`
- `track/items/<id>/metadata.json`
- `track/items/<id>/spec.json` (Phase 1 SSoT, if exists)
- `track/items/<id>/<layer>-types.json` (Phase 2 SSoT, if exists)
- `track/items/<id>/impl-plan.json` + `task-coverage.json` (Phase 3 SSoT, if exists)
- `track/items/<id>/spec.md`
- `track/items/<id>/plan.md`
- `track/items/<id>/verification.md`
- `TRACK_TRACEABILITY.md`
- `knowledge/DESIGN.md`
- `.claude/rules/`
- `knowledge/external/POLICY.md`
- `knowledge/external/guides.json`
- `architecture-rules.json`

Operational split:

- `DEVELOPER_AI_WORKFLOW.md`: user-facing operating guide
- `CLAUDE.md`: maintainer/reference guide
- `track/workflow.md`: day-to-day workflow rules
- `knowledge/conventions/`: project-specific engineering rules and implementation policies
- `TRACK_TRACEABILITY.md`: `plan.md` state transitions and registry update rules
- `knowledge/external/guides.json`: registry for long-form external guides cached outside git
- `knowledge/external/POLICY.md`: operating policy for external long-form guides
- `architecture-rules.json`: machine-readable layer dependency source of truth for `deny.toml` and `scripts/check_layers.py`
- `.harness/config/agent-profiles.json`: capability-to-provider mapping source of truth

## Planning Gate (Mandatory)

Always invoke `/track:plan` before implementation, regardless of task difficulty. `/track:plan` orchestrates Phase 0-3 (init → spec → design → impl-plan) and back-and-forth escalation when downstream signals fail. Skipping design entirely causes expensive downstream review loops (historical lesson: 15+ review rounds from skipped design).

## Briefing Requirements (Provider-Agnostic)

All capability briefings (regardless of provider — Codex, Claude, or future providers) must reference `.claude/rules/04-coding-principles.md` for type design patterns. The enum-first / typestate / hybrid decision table in that file is the source of truth.

## Delegation Rules

Use the minimum capable capability first, then resolve it via `.harness/config/agent-profiles.json`.

- Claude Code (`orchestrator` host):
  - normal edits
  - workflow control
  - file synchronization
  - user interaction
- specialist capabilities:
  - `orchestrator`: overall coordination (always Claude Code)
  - `spec-designer`: behavioral contract authoring (Phase 1 spec.json writer)
  - `type-designer`: type-level contract authoring (Phase 2 `<layer>-types.json` writer, TDDD workflow)
  - `impl-planner`: implementation plan authoring (Phase 3 impl-plan.json + task-coverage.json writer)
  - `adr-editor`: ADR back-and-forth modification (invoked by `/track:plan` when spec → ADR signal turns 🔴)
  - `implementer`: difficult Rust implementation, refactoring, performance-oriented edits
  - `reviewer`: code review, correctness analysis, idiomatic Rust checks
  - `researcher`: crate research, codebase-wide analysis, external research
- provider resolution (from `.harness/config/agent-profiles.json`):
  - `orchestrator` / `spec-designer` / `type-designer` / `impl-planner` / `adr-editor` / `implementer` → Claude Code
  - `reviewer` → Codex CLI
  - `researcher` → Gemini CLI
- Agent Teams:
  - `/track:implement`
  - `/track:review`

If unsure:

1. Workflow control or user interaction -> Claude Code
2. Research need -> `researcher`
3. Behavioral spec authoring -> `spec-designer`
4. Type catalogue authoring -> `type-designer`
5. Implementation plan authoring -> `impl-planner`
6. ADR back-and-forth modification -> `adr-editor`
7. Review need -> `reviewer`
8. Implementation work -> `implementer`
