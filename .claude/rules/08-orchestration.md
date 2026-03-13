# Orchestration

Claude Code is the orchestrator.

- User-facing interface: `/track:*`
- Context management: `track/`
- Execution workflows: `takt`
- Capability routing: `.claude/agent-profiles.json`
- Default specialist profile:
  - `planner` / `reviewer` / `debugger`: Codex CLI
  - `researcher` / `multimodal_reader`: Gemini CLI
  - `implementer`: Claude Code
- Parallel execution: Agent Teams

Host orchestration stays in Claude Code.
Specialist capabilities may switch as models evolve, but the public `/track:*` interface should remain stable.

Terms:

- `track`: `metadata.json` (SSoT) / `spec.md` / `plan.md` (read-only rendered view) / `verification.md` / progress management layer
- `takt`: execution workflow layer for implementation and review

## Source Of Truth

Read these first before planning or implementation:

- `track/tech-stack.md`
- `track/workflow.md`
- `track/registry.md`
- `project-docs/conventions/README.md`
- `track/items/<id>/metadata.json`
- `track/items/<id>/spec.md`
- `track/items/<id>/plan.md`
- `track/items/<id>/verification.md`
- `TAKT_TRACK_TRACEABILITY.md`
- `.claude/docs/DESIGN.md`
- `.claude/rules/`
- `docs/EXTERNAL_GUIDES.md`
- `docs/external-guides.json`
- `docs/architecture-rules.json`

Operational split:

- `DEVELOPER_AI_WORKFLOW.md`: user-facing operating guide
- `CLAUDE.md`: maintainer/reference guide
- `track/workflow.md`: day-to-day workflow rules
- `project-docs/conventions/`: project-specific engineering rules and implementation policies
- `TAKT_TRACK_TRACEABILITY.md`: `plan.md` state transitions and registry update rules
- `docs/external-guides.json`: registry for long-form external guides cached outside git
- `docs/EXTERNAL_GUIDES.md`: operating policy for external long-form guides
- `docs/architecture-rules.json`: machine-readable layer dependency source of truth for `deny.toml` and `scripts/check_layers.py`
- `.claude/agent-profiles.json`: capability-to-provider mapping source of truth

## Delegation Rules

Use the minimum capable capability first, then resolve it via `.claude/agent-profiles.json`.

- Claude Code (`orchestrator` host):
  - normal edits
  - workflow control
  - file synchronization
  - user interaction
- specialist capabilities:
  - `planner`: architecture design, trait/module planning, trade-off evaluation
  - `researcher`: crate research, codebase-wide analysis, external research
  - `implementer`: difficult Rust implementation, refactoring, performance-oriented edits
  - `reviewer`: code review, correctness analysis, idiomatic Rust checks
  - `debugger`: compile-error diagnosis, failing test analysis
  - `multimodal_reader`: PDF / image / audio / video understanding
- provider resolution:
  - default profile maps `planner` / `reviewer` / `debugger` to Codex CLI
  - default profile maps `researcher` / `multimodal_reader` to Gemini CLI
  - default profile maps `implementer` to Claude Code
- Agent Teams:
  - `/track:implement`
  - `/track:review`
- takt:
  - autonomous implementation / review workflows driven by `.takt/pieces/`

If unsure:

1. Workflow control or user interaction -> Claude Code
2. Research or multimodal need -> `researcher` / `multimodal_reader`
3. Design, review, or debugging need -> `planner` / `reviewer` / `debugger`
4. Deterministic workflow execution -> takt
5. Implementation work -> `implementer`
