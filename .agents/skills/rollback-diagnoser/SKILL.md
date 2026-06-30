---
name: rollback-diagnoser
description: Use when Codex is assigned the SoTOHE `rollback-diagnoser` capability. Receives a diagnostic input (PreReviewGate Blocked summary / plan-artifacts review finding / external PR-reviewer comment), reads the SoT chain top-down, and returns a structured `{routing_target, reason, recommended_next_action}` routing decision the calling orchestrator dispatches. Diagnose-only — never edits SoT artifacts, never invokes writer agents.
---

# Rollback-Diagnoser (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/rollback-diagnoser.md` — the
provider-agnostic contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `rollback-diagnoser` capability
  (`.codex/agents/rollback-diagnoser.toml`).
- Triggered from `/track:diagnose` (`.claude/commands/track/diagnose.md`) via the same
  capability-resolution path used by the Claude subagent (the orchestrator host resolves
  `capabilities.rollback-diagnoser.provider` from `.harness/config/agent-profiles.json` and
  dispatches accordingly).
- This skill is **diagnose-only**: it must not write to any SoT artifact, must not invoke any
  writer agent, and must not run any mutating `bin/sotp` subcommand, including `signal calc-*`
  refreshes. Signal refresh is orchestrator-owned before invocation; this skill may only read
  persisted signal JSON or use true read-only inspection (`ref-verify results`,
  `task-contract coverage` / `check`, `review results`).
- The structured output is the skill's terminal text — not a human-facing summary. The
  orchestrator parses the three fields (`routing_target` / `reason` / `recommended_next_action`)
  and dispatches the corresponding writer or applies a source-edit task for `impl`.
- See the operational SSoT for the 5-class routing taxonomy, mandatory context-file pre-read,
  and the routing procedure.
