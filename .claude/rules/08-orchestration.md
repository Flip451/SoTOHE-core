# Orchestration

Claude Code is the orchestrator.

- User-facing interface: `/track:*`
- Context management: `track/`
- Capability routing: `.harness/config/agent-profiles.json`
- Default capability assignment:
  - `orchestrator` / `planner` / `designer` / `implementer`: Claude Code
  - `reviewer`: Codex CLI
  - `researcher`: Gemini CLI
- Parallel execution: Agent Teams

Host orchestration stays in Claude Code.
Specialist capabilities may switch as models evolve, but the public `/track:*` interface should remain stable.

Terms:

- `track`: `metadata.json` (SSoT) / `spec.md` / `plan.md` (read-only rendered view) / `verification.md` / progress management layer

## Source Of Truth

Read these first before planning or implementation:

- `track/tech-stack.md`
- `track/workflow.md`
- `track/registry.md`
- `knowledge/conventions/README.md`
- `track/items/<id>/metadata.json`
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

## Planner Gate (Mandatory)

Always invoke `/track:plan` before implementation, regardless of task difficulty.
`/track:plan` supports Quick/Focused/Full modes — smaller tasks use reduced scope, but
the planning step itself is never skipped. Skipping design entirely causes expensive
downstream review loops (historical lesson: 15+ review rounds from skipped design).

## Planner Briefing Requirements (Provider-Agnostic)

All planner briefings (regardless of provider — Codex, Claude, or future providers) must
reference `.claude/rules/04-coding-principles.md` for type design patterns.
The enum-first / typestate / hybrid decision table in that file is the source of truth.

## Delegation Rules

Use the minimum capable capability first, then resolve it via `.harness/config/agent-profiles.json`.

- Claude Code (`orchestrator` host):
  - normal edits
  - workflow control
  - file synchronization
  - user interaction
- specialist capabilities (6):
  - `orchestrator`: overall coordination (always Claude Code)
  - `planner`: architecture design, trait/module planning, trade-off evaluation
  - `designer`: domain type design (TDDD workflow)
  - `implementer`: difficult Rust implementation, refactoring, performance-oriented edits
  - `reviewer`: code review, correctness analysis, idiomatic Rust checks
  - `researcher`: crate research, codebase-wide analysis, external research
- provider resolution (from `.harness/config/agent-profiles.json`):
  - `orchestrator` / `planner` / `designer` / `implementer` → Claude Code
  - `reviewer` → Codex CLI
  - `researcher` → Gemini CLI
- Agent Teams:
  - `/track:implement`
  - `/track:review`

If unsure:

1. Workflow control or user interaction -> Claude Code
2. Research need -> `researcher`
3. Design, review need -> `planner` / `designer` / `reviewer`
4. Implementation work -> `implementer`
