# Impl-Plan Workflow SSoT

> Provider-agnostic workflow SSoT for the `impl-plan` track workflow. Both the Claude adapter
> (`.claude/commands/track/impl-plan.md`) and the Codex skill adapter
> (`.agents/skills/track-impl-plan/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Author the implementation plan for the current track — `track/items/<id>/impl-plan.json` and
`task-coverage.json` — via the `impl-planner` capability (Phase 3). The workflow is
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
- **ADR path(s)** and **related conventions** — paths under `knowledge/adr/` and
  `knowledge/conventions/` for the feature domain.

## Sequence

**Step 1: Pre-check**

Confirm `track/items/<track-id>/spec.json` exists (Phase 1 output). If not, stop and
instruct the caller to run the `spec-design` workflow first.

Confirm at least one `track/items/<track-id>/<layer>-types.json` exists for every TDDD-enabled
layer (Phase 2 output). If not, stop and instruct the caller to run the `type-design` workflow
(`.harness/workflows/track/type-design.md`) first.

**Step 2: Invoke impl-planner capability**

Invoke the `impl-planner` capability (see `.harness/capabilities/impl-planner.md` for the full
internal pipeline). The briefing must include:

- Track id and paths to `track/items/<track-id>/spec.json` and each `<layer>-types.json`
- Path(s) to the referenced ADR(s) under `knowledge/adr/`
- Paths to related conventions under `knowledge/conventions/`

The capability owns writing `track/items/<track-id>/impl-plan.json` and
`track/items/<track-id>/task-coverage.json`, and evaluating the task-coverage binary gate
(OK / ERROR). The workflow does not duplicate these steps.

**Step 3: Receive and surface the gate verdict**

Receive the binary gate verdict (OK / ERROR) from the capability output. Surface the verdict,
task count, and any gate error details to the caller without re-reading the output files.

## Gates

| Gate | Verdict |
|------|---------|
| `spec.json` exists | ERROR if absent |
| At least one `<layer>-types.json` exists per TDDD-enabled layer | ERROR if absent |
| Capability task-coverage binary gate | OK / ERROR |

## Failure / recovery

- **Missing spec.json**: stop and instruct the caller to run the `spec-design` workflow first.
- **Missing type catalogues**: stop and instruct the caller to run the `type-design` workflow first.
- **Capability execution failure**: retry up to 2 times (transient tooling errors). If retries
  also fail, report to the caller and stop.
- **Capability returns ERROR (task-coverage gate)**: surface the gate error details to the
  caller. The caller (`plan` workflow) applies the loop rule (re-invoke `impl-planner` in the
  same phase). The `max_retry` guard is enforced by the caller.

## Outputs

- `track/items/<id>/impl-plan.json` (written by the capability)
- `track/items/<id>/task-coverage.json` (written by the capability)
- Binary gate verdict: **OK** or **ERROR** + error details
- Task count (surfaced to caller from capability output)
- No commit is created by this workflow
