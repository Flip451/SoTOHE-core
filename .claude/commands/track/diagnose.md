---
description: Diagnose a phase-rollback target for an impl-phase or later structural inconsistency, returning a structured routing decision the orchestrator dispatches.
---

Canonical command for **phase rollback diagnosis**. `/track:diagnose` is a one-shot diagnostic
skill that runs when the impl-phase or later surfaces a structural inconsistency the internal
signal pipeline cannot localize on its own. It returns a structured routing decision identifying
which phase (`adr` / `spec` / `type` / `impl_plan` / `impl`) the calling orchestrator should
roll back to. The orchestrator owns writer dispatch — this command never invokes
adr-editor / spec-designer / type-designer / impl-planner directly, and never edits any SoT
artifact.

Provider routing is resolved via `.harness/config/agent-profiles.json`
(`capabilities.rollback-diagnoser.provider` / `model`). The full operational contract — trigger
inputs, mandatory context-file pre-read, LLM-semantic routing taxonomy, output schema, and
boundary with other capabilities — lives in
`.harness/capabilities/rollback-diagnoser.md` (provider-agnostic SSoT). Both the Claude subagent
(`.claude/agents/rollback-diagnoser.md`) and the Codex skill
(`.agents/skills/rollback-diagnoser/SKILL.md`) reference that file.

## Arguments

- `$ARGUMENTS`: the diagnostic input — a `bin/sotp task-contract check` (PreReviewGate) Blocked
  summary, a `/track:review` plan-artifacts finding, or a free-form reviewer comment. May be
  passed inline or via a `--briefing-file <path>` reference. If empty, ask the user for the
  diagnostic input and stop.

## When to invoke

Invoke from the orchestrator's main loop in one of these scenarios (see also
`.harness/capabilities/rollback-diagnoser.md` §"Trigger inputs"):

1. **PreReviewGate Blocked**: `bin/sotp task-contract check` returns
   `PreReviewGateOutcome::Blocked`. The CLI surfaces a soft prompt suggesting this command in
   the Blocked stderr output (`apps/cli-driver/src/task_contract.rs` `render_check_violations`).
2. **plan-artifacts review findings**: `/track:review` on `plan-artifacts` scope surfaced 🔴
   signals or structural mismatch findings inconclusive for orchestrator-level classification.
3. **External PR-reviewer comments**: any `/track:pr-review` (Codex Cloud) comment whose
   routing target is not self-evident. Manual passthrough; the orchestrator decides.

## Execution

1. Resolve the active capability provider from `.harness/config/agent-profiles.json`
   (`capabilities.rollback-diagnoser`). Both Claude and Codex hosts route through this single
   capability resolution.
2. Invoke the resolved provider via the appropriate adapter:
   - **provider: claude** — invoke the Claude subagent via the Agent tool with
     `subagent_type: "rollback-diagnoser"`. The subagent reads the operational SSoT
     (`.harness/capabilities/rollback-diagnoser.md`) and executes the routing judgment.
   - **provider: codex** — invoke the Codex specialist agent
     (`.codex/agents/rollback-diagnoser.toml`) through a repo-owned wrapper that forces a
     read-only sandbox. If a direct CLI fallback is unavoidable, it must use
     `codex exec --sandbox read-only`; never use `--full-auto`, `--sandbox workspace-write`, or
     any invocation path that allows writes for this diagnose-only capability. The codex skill
     (`.agents/skills/rollback-diagnoser/SKILL.md`) reads the same operational SSoT.
3. Receive the structured routing decision from the capability:
   ```
   {
     "routing_target": "adr" | "spec" | "type" | "impl_plan" | "impl",
     "reason": "<japanese diagnostic citing element ids>",
     "recommended_next_action": "<japanese concrete next step>"
   }
   ```
4. The orchestrator inspects `routing_target` and dispatches:
   - `adr` → `/adr:add <slug>` (new ADR) or `adr-editor` subagent (existing ADR D)
   - `spec` → re-invoke `/track:spec-design` (Phase 1 partial re-entry)
   - `type` → re-invoke `/track:type-design` (Phase 2 partial re-entry)
   - `impl_plan` → re-invoke `/track:impl-plan` (Phase 3 partial re-entry)
   - `impl` → apply a source edit task (no writer subagent)
5. The orchestrator may override the suggested target if it judges `reason` insufficiently
   convincing. Diagnose-only outputs are recommendations, not contracts on the orchestrator.

## Behavior

After execution, the command returns the structured routing decision verbatim to the
orchestrator. It does NOT:

- Edit any SoT artifact (ADR / spec.json / `<layer>-types.json` / impl-plan.json /
  task-coverage.json / task-contract.json).
- Stage or commit any file.
- Invoke any writer subagent (adr-editor / spec-designer / type-designer / impl-planner).
- Apply source-edit tasks (the orchestrator translates `impl` targets to source edits).
- Run any mutating `bin/sotp` subcommand, including `signal calc-*` refreshes. Signal refresh is
  orchestrator-owned before invocation; this command may only read persisted signal JSON or use
  true read-only inspection (`ref-verify results`, `task-contract coverage` / `check`,
  `review results`).

## References

- `.harness/capabilities/rollback-diagnoser.md` — provider-agnostic operational SSoT (routing
  taxonomy, mandatory context-file pre-read, output contract)
- `.claude/agents/rollback-diagnoser.md` — Claude subagent wrapper
- `.agents/skills/rollback-diagnoser/SKILL.md` — Codex skill wrapper
- `.codex/agents/rollback-diagnoser.toml` — Codex agent TOML
- `.harness/config/agent-profiles.json` — `capabilities.rollback-diagnoser` provider routing
- `knowledge/adr/2026-06-26-0503-adr2pr-back-and-forth-skill-definition.md` D1-D8 — original
  ADR decisions for this skill and capability
