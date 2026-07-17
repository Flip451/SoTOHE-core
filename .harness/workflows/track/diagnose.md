# Diagnose Workflow SSoT

> Provider-agnostic workflow SSoT for the `diagnose` track workflow. The Claude adapter
> (`.claude/commands/track/diagnose.md`) references this file. Provider-specific invocation
> framing lives in that adapter; the full workflow contract lives here.

## Mission

Diagnose recovery for either an ADR-baseline mismatch or an impl-phase-or-later structural
inconsistency. A baseline mismatch takes the dedicated `adr-diagnoser` route below and returns
its verdict; all other diagnostic inputs dispatch `rollback-diagnoser` and return a phase-routing
decision (`adr` / `spec` / `type` / `impl_plan` / `impl`). The orchestrator owns all writer
dispatch and recovery writes — this workflow never invokes adr-editor / spec-designer /
type-designer / impl-planner directly, and never edits any SoT artifact.

See `.harness/capabilities/adr-diagnoser.md` for baseline-triage rules and
`.harness/capabilities/rollback-diagnoser.md` for the phase-routing taxonomy, context-file
pre-read, output schema, and boundaries.

## ADR-baseline mismatch diagnosis route

When `adr-baseline check-review`, `adr-baseline check-commit`, or the track-aware CI path blocks
on a byte mismatch, the orchestrator enters this route before the ordinary phase-rollback
diagnosis. The binary check remains purely byte-based; semantic classification occurs only here,
after the block.

1. The calling orchestrator prepares and supplies a briefing containing the triggering check output, direct
   ADR filename, active track id, current-versus-latest-baseline diff, and originating capability
   when known. Preparing that scratch briefing is outside this read-only workflow.
2. Dispatch the read-only capability:

   ```
   bin/sotp capability exec adr-diagnoser --host <current-host> --briefing-file <path>
   ```

3. Return its structured `{verdict, reason, recommended_next_action}` output verbatim to the
   caller. The diagnoser returns a verdict only. An uncertain semantic effect is `deviation`,
   never a restamp.

### Post-diagnosis recovery (orchestrator-only, outside this workflow)

After the caller has consumed a parseable ADR-diagnoser verdict, the **orchestrator**, not this
workflow or either diagnoser capability, performs the recovery write:

- `non-semantic-restamp` → run `bin/sotp adr-baseline snapshot --source <file> --kind non-semantic-fix`, then retry the triggering check.
- `deviation` → run `bin/sotp adr-baseline restore --source <file>`, inject the mismatch history into the originating capability briefing, and require an amendment proposal rather than an in-place ADR edit.
- `unknown-editor` → run `bin/sotp adr-baseline restore --source <file>`, record the history in the optional `observations.md`, then retry or continue from the restored state.

Snapshot, restore, briefing injection, and observation recording are all orchestrator actions
outside the diagnose workflow.

## Inputs

- **Diagnostic briefing** — a non-empty caller-prepared `--briefing-file <path>` containing a
  `bin/sotp task-contract check` (PreReviewGate) Blocked summary, a `review` workflow finding on
  any SoT scope (`adr` / `spec` / `types` / `impl-plan`), or a free-form reviewer comment.
  Inline diagnostic input is not supported because both diagnosers require the same briefing-file
  dispatch contract.
- **ADR-baseline mismatch** — a byte-mismatch output from `check-review`, `check-commit`, or
  CI, plus the source filename, latest-baseline diff, and originating capability when known.

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
4. **ADR-baseline mismatch**: use the ADR-baseline mismatch recovery route above, not the
   `rollback-diagnoser` phase-routing taxonomy.

## Sequence

**Step 1: Dispatch the selected read-only diagnoser capability**

For an ADR-baseline byte mismatch, follow the dedicated route above and invoke
`adr-diagnoser`. For every other diagnostic input, create no files in this workflow and invoke
`rollback-diagnoser`:

Use the caller-provided diagnostic briefing and invoke:

```
bin/sotp capability exec rollback-diagnoser --host <current-host> --briefing-file <path>
```

The dispatcher resolves the provider and model internally from
`.harness/config/agent-profiles.json`, validates the provider-native definition, and keeps the
capability read-only. A `delegate-in-host` outcome is an instruction for the current host;
otherwise the dispatcher performs the provider subprocess execution.

**Step 2: Receive the corresponding structured verdict**

ADR-baseline mismatch:

```
{
  "verdict": "non-semantic-restamp" | "deviation" | "unknown-editor",
  "reason": "<Japanese explanation>",
  "recommended_next_action": "<Japanese orchestrator action>"
}
```

Other diagnostic input:

```
{
  "routing_target": "adr" | "spec" | "type" | "impl_plan" | "impl",
  "reason": "<japanese diagnostic citing element ids>",
  "recommended_next_action": "<japanese concrete next step>"
}
```

**Step 3: Orchestrator dispatch (outside this workflow)**

For a `rollback-diagnoser` routing decision, the calling orchestrator inspects `routing_target`
and dispatches:

- `adr` → author a new ADR (`adr:add`) or invoke the `adr-editor` capability (existing ADR D)
- `spec` → re-invoke the `spec-design` workflow (Phase 1 partial re-entry)
- `type` → re-invoke the `type-design` workflow (Phase 2 partial re-entry)
- `impl_plan` → re-invoke the `impl-plan` workflow (Phase 3 partial re-entry)
- `impl` → apply a source edit task (no writer subagent)

For an `adr-diagnoser` verdict, it performs only the post-diagnosis recovery actions documented
above. The orchestrator may override a rollback target if it judges `reason` insufficiently
convincing. Diagnose-only outputs are recommendations, not contracts on the orchestrator.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1 | Selected diagnoser preflight | OK / ERROR (fail-closed) |
| 2 | Capability returns the matching parseable verdict schema | OK / retry (up to 2) / report |

## Constraints

This workflow does NOT:

- Create or modify a briefing, ADR, baseline copy, ledger, `observations.md`, or any other file.
- Run `bin/sotp adr-baseline snapshot` or `bin/sotp adr-baseline restore`; those mutating
  recovery commands are exclusively orchestrator actions after this workflow returns a verdict.
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

- **Missing or empty diagnostic briefing**: ask the caller for a complete `--briefing-file` and
  stop.
- **Capability execution failure / unparseable output**: retry up to 2 times; if retries also
  fail, report to the user and stop.
- **Unresolvable provider**: fail-closed; do not run the diagnosis with an unknown provider.
- **ADR-baseline mismatch**: if `adr-diagnoser` fails or returns an unparseable verdict, retry
  up to 2 times; if it still fails, keep the baseline gate blocked and report to the user. Do
  not snapshot, restore, or bypass the check without a verdict.

## Outputs

- The structured `adr-diagnoser` verdict **or** `rollback-diagnoser` routing decision, returned
  verbatim to the caller
- No file modifications, snapshots, restores, commits, or writer dispatches
