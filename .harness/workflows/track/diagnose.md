# Diagnose Workflow SSoT

> Provider-agnostic workflow SSoT for the `diagnose` track workflow. The Claude adapter
> (`.claude/commands/track/diagnose.md`) references this file. Provider-specific invocation
> framing lives in that adapter; the full workflow contract lives here.

## Mission

Diagnose a phase-rollback target for an impl-phase or later structural inconsistency. The
workflow is a one-shot diagnostic: it dispatches the `rollback-diagnoser` capability, receives
a structured routing decision identifying which phase (`adr` / `spec` / `type` / `impl_plan` /
`impl`) the calling orchestrator should roll back to, and returns that decision verbatim. The
orchestrator owns writer dispatch — this workflow never invokes
adr-editor / spec-designer / type-designer / impl-planner directly, and never edits any SoT
artifact.

See `.harness/capabilities/rollback-diagnoser.md` for the capability's full operational
contract (trigger inputs, mandatory context-file pre-read, LLM-semantic routing taxonomy,
output schema, and boundary with other capabilities).

## Inputs

- **Diagnostic input** — supplied by the caller: a `bin/sotp task-contract check`
  (PreReviewGate) Blocked summary, a `review` workflow finding on any SoT scope
  (`adr` / `spec` / `types` / `impl-plan`), or a free-form reviewer comment. May be passed
  inline or via a `--briefing-file <path>` reference. If empty, ask the user for the
  diagnostic input and stop.

## Trigger scenarios

Invoke from the orchestrator's main loop in one of these scenarios (see also
`.harness/capabilities/rollback-diagnoser.md` §"Trigger inputs"):

1. **PreReviewGate Blocked**: `bin/sotp task-contract check` returns
   `PreReviewGateOutcome::Blocked`. The CLI surfaces a soft prompt suggesting this workflow in
   the Blocked stderr output (emitted by the task-contract driver).
2. **SoT-scope review findings**: the `review` workflow on any of the `adr` / `spec` /
   `types` / `impl-plan` scopes surfaced 🔴 signals or structural mismatch findings
   inconclusive for orchestrator-level classification.
3. **External PR-reviewer comments**: any `pr-review` workflow comment whose routing target is
   not self-evident. Manual passthrough; the orchestrator decides.

## Sequence

**Step 1: Dispatch the rollback-diagnoser capability**

Create a diagnostic briefing and invoke:

```
bin/sotp capability exec rollback-diagnoser --host <current-host> --briefing-file <path>
```

The dispatcher resolves the provider and model internally from
`.harness/config/agent-profiles.json`, validates the provider-native definition, and keeps the
capability read-only. A `delegate-in-host` outcome is an instruction for the current host;
otherwise the dispatcher performs the provider subprocess execution.

**Step 2: Receive the structured routing decision**

```
{
  "routing_target": "adr" | "spec" | "type" | "impl_plan" | "impl",
  "reason": "<japanese diagnostic citing element ids>",
  "recommended_next_action": "<japanese concrete next step>"
}
```

**Step 3: Orchestrator dispatch (outside this workflow)**

The calling orchestrator inspects `routing_target` and dispatches:

- `adr` → author a new ADR (`adr:add`) or invoke the `adr-editor` capability (existing ADR D)
- `spec` → re-invoke the `spec-design` workflow (Phase 1 partial re-entry)
- `type` → re-invoke the `type-design` workflow (Phase 2 partial re-entry)
- `impl_plan` → re-invoke the `impl-plan` workflow (Phase 3 partial re-entry)
- `impl` → apply a source edit task (no writer subagent)

The orchestrator may override the suggested target if it judges `reason` insufficiently
convincing. Diagnose-only outputs are recommendations, not contracts on the orchestrator.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1 | `sotp capability exec rollback-diagnoser` preflight | OK / ERROR (fail-closed) |
| 2 | Capability returns a parseable routing decision | OK / retry (up to 2) / report |

## Constraints

This workflow does NOT:

- Edit any SoT artifact (ADR / spec.json / `<layer>-types.json` / impl-plan.json /
  task-coverage.json / task-contract.json).
- Stage or commit any file.
- Invoke any writer subagent (adr-editor / spec-designer / type-designer / impl-planner).
- Apply source-edit tasks (the orchestrator translates `impl` targets to source edits).
- Run any mutating `bin/sotp` subcommand, including `signal calc-*` refreshes. Signal refresh
  is orchestrator-owned before invocation; the capability may only read persisted signal JSON
  or use true read-only inspection (`ref-verify results`, `task-contract coverage` / `check`,
  `review results`).

## Failure / recovery

- **Empty diagnostic input**: ask the user for the input and stop.
- **Capability execution failure / unparseable output**: retry up to 2 times; if retries also
  fail, report to the user and stop.
- **Unresolvable provider**: fail-closed; do not run the diagnosis with an unknown provider.

## Outputs

- The structured routing decision (returned verbatim to the caller)
- No file modifications, no commits
