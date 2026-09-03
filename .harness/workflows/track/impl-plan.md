# Impl-Plan Workflow SSoT

> Provider-agnostic workflow SSoT for the `impl-plan` track workflow. Both the Claude adapter
> (`.claude/commands/track/impl-plan.md`) and the Codex skill adapter
> (`.agents/skills/track-impl-plan/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Author the implementation plan for the current track — `track/items/<id>/impl-plan.json`,
`task-coverage.json`, `task-contract.json`, and `batch-plan.json` — via the `impl-planner`
capability (Phase 3; that capability is the sole writer of all four). The workflow is
single-shot: invoke the capability once, receive its binary gate verdict, and return. Re-invocation
on ERROR is the caller's responsibility (`plan` workflow). The `impl-planner` capability owns
all file writes and gate evaluation internally.

See `.harness/capabilities/impl-planner.md` for the capability's full operational contract.

## Inputs

- **Current branch** — must match `track/<id>`. The track id is resolved from this branch.
- **`track/items/<track-id>/spec.json`** — must exist (Phase 1 completed). If absent, stop
  and instruct the caller to run the `spec-design` workflow first.
- **`track/items/<track-id>/<layer>-types.json`** — at least one must exist for every
  TDDD-enabled layer (Phase 2 completed). If none exist, stop and instruct the caller to run
  the `type-design` workflow first.
- **ADR path(s)** — paths under `knowledge/adr/` for the feature domain.

The workflow does not read or select conventions. The capability dispatcher supplies the
resolved convention paths in the delegated briefing (possibly an empty set), and the
`impl-planner` capability reads those paths as its complete convention input.

## Summary-first context intake

Before opening a catalogue or plan artifact, use `bin/sotp track resolve` and
`bin/sotp track task-counts` for progress, `bin/sotp review results` for review necessity,
`bin/sotp test-obligation results` for obligation state when enrolled, and
`bin/sotp catalog check` plus `bin/sotp ref-verify results --chain 2 --filter all` for catalogue
and catalogue-to-specification state. These CLI summaries are primary. Do not bulk-read
`*-types.json`, `review.json`, bindings JSON, full sub-workflow texts, or a `Related Conventions`
list. Open only a targeted diff or the body named by a blocker; the delegated capability receives
and reads exact spec, catalogue, ADR, and convention paths from its briefing.

## Sequence

**Step 1: Pre-check**

Confirm `track/items/<track-id>/spec.json` exists (Phase 1 output). If not, stop and
instruct the caller to run the `spec-design` workflow first.

Run `bin/sotp catalog check` and use its summary to confirm that every TDDD-enabled layer has a
complete catalogue (Phase 2 output). If it reports a missing or incomplete catalogue, stop and
instruct the caller to run the `type-design` workflow first. Do not open catalogue JSON merely to
perform this presence check.

**Step 2: Review the types scope**

1. Use `bin/sotp review results` to determine whether the `types` scope needs attention, then
   invoke the `review` workflow's single-scope re-entry round for `types`
   (`.harness/workflows/track/review.md` §Single-scope re-entry round) to `zero_findings`.

**Step 3: Enter the impl-plan phase**

Prepare the configured writer briefing at `tmp/impl-planner-briefing.md`. It must include:

- Track id and paths to `track/items/<track-id>/spec.json` and each `<layer>-types.json`
- Path(s) to the referenced ADR(s) under `knowledge/adr/`

Do not add hand-picked convention paths to the workflow-generated file. The dispatcher supplies
the resolved paths alongside the delegated briefing, and an empty resolved set remains
authoritative.

Then run `bin/sotp phase enter impl-plan`. The phase engine runs the declared pre-entry checks
and, only when they all succeed, invokes the configured `impl-planner` writer. The workflow
must not dispatch that writer directly. The writer owns `track/items/<track-id>/impl-plan.json`,
`track/items/<track-id>/task-coverage.json`, `track/items/<track-id>/task-contract.json`, and
`track/items/<track-id>/batch-plan.json`, and evaluating both binary gates — the
task-coverage gate (`bin/sotp verify plan-artifact-refs`) and the batch-plan structural gate
(`bin/sotp batch-plan check`). Phase 3 passes only when both are OK.

**Step 4: Receive and surface the gate verdict**

Receive the binary gate verdict (OK / ERROR) from the capability output. Surface the verdict,
task count, and any gate error details to the caller without re-reading the output files.

## Gates

| Gate | Verdict |
|------|---------|
| `spec.json` exists | ERROR if absent |
| At least one `<layer>-types.json` exists per TDDD-enabled layer | ERROR if absent |
| Types-scope fast and final reviews | `zero_findings` / ERROR |
| Capability task-coverage binary gate | OK / ERROR |
| Capability batch-plan structural gate (`bin/sotp batch-plan check`) | OK / ERROR |

## Failure / recovery

- **Missing spec.json**: stop and instruct the caller to run the `spec-design` workflow first.
- **Missing type catalogues**: stop and instruct the caller to run the `type-design` workflow first.
- **Types-scope review findings**: do not enter the phase; return to the `type-design` workflow
  for catalogue repair. After regeneration, return through the `plan` workflow's Phase 2 🔵 path
  (including `bin/sotp ref-verify run`) to refresh Chain 2 approval before re-running the
  types-scope single-scope review or entering `impl-plan`.
- **Types-scope review blocked_cross_scope**: return to the caller for the direct-upstream
  rollback route; use the `diagnose` workflow when the routing target is unclear.
- **Phase-entry failure**: retry up to 2 times for transient execution failures. A failed
  pre-entry check does not launch the writer; report it to the caller and stop.
- **Capability returns ERROR (task-coverage or batch-plan gate)**: surface the gate error
  details to the caller. The caller (`plan` workflow) applies the loop rule (re-invoke the
  `impl-plan` workflow in the same phase). The `max_retry` guard is enforced by the caller.

## Outputs

- `track/items/<id>/impl-plan.json` (written by the capability)
- `track/items/<id>/task-coverage.json` (written by the capability)
- `track/items/<id>/task-contract.json` (written by the capability)
- `track/items/<id>/batch-plan.json` (written by the capability — its sole writer)
- Binary gate verdict: **OK** or **ERROR** + error details
- Task count (surfaced to caller from capability output)
- No commit is created by this workflow
