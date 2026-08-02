# Implement Workflow SSoT

> Provider-agnostic workflow SSoT for the `implement` track workflow. Both the Claude adapter
> (`.claude/commands/track/implement.md`) and the Codex skill adapter
> (`.agents/skills/track-implement/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Run parallel interactive implementation for the current track. The orchestrator reads the
approved implementation plan, marks selected tasks `in_progress`, delegates implementation using
the available parallelism of the execution environment, and runs CI to verify correctness.
Implementer runs report completion; only the orchestrator performs task-state transitions. It
marks tasks `done` once CI passes and the DRY fix phase closes — before review, so the review
sees the final task state and the transition diff does not invalidate an approved round — and
backfills the commit hash after the batch commit.
Implementation requires being on a `track/<id>` branch. No commit is created by this workflow —
the enclosing lifecycle proceeds through the DRY fix phase, the orchestrator’s pre-review `done`
transitions, review, then the `commit` workflow.

## Inputs

- **Current branch** — must match `track/<id>`. The track id is resolved from this branch.
  If the branch does not match this pattern, stop immediately and report the situation.
- **Track context** — `spec.md`, `plan.md`, `metadata.json`, and all conventions listed in
  `## Related Conventions (Required Reading)` in `spec.md` (or `plan.md` for legacy tracks).
  Use the corresponding `<layer>-types.json` catalogue entries for exact type signatures,
  trait definitions, and module trees; use `impl-plan.json` for task execution detail. Rendered
  views provide context but do not replace those machine-readable sources.
- **ADR pre-check** — if `spec.md` or `plan.md` references an ADR under `knowledge/adr/`,
  read the ADR and verify that the target task's description is consistent with the ADR's
  design (layer placement, error types, behavioral contracts). Fix `metadata.json` (then
  `bin/sotp track views sync`) before writing code if discrepancies are found. The ADR is the
  source of truth for design decisions.
- **Optional scope notes** — caller-supplied hints (target module, constraints, priority) that
  narrow the set of tasks to implement.

## Sequence

**Step 1: Resolve track and validate context**

1. Resolve the current track:
   - If the current git branch matches `track/<id>`, use that track.
   - Otherwise, use the latest materialized active track (non-archived, non-done, `branch != null`).
   - If no materialized active track is found on a `track/<id>` branch, stop immediately and
     report the situation. Do not transition tasks or write implementation code.
2. Read `spec.md`, `plan.md`, and `metadata.json`. Read every convention file listed in
   `## Related Conventions (Required Reading)`.
3. Identify the target task(s) from the approved plan. If scope notes are provided, map them
   to the relevant plan scope.

**Step 2: Orchestrator marks tasks in_progress**

Before dispatching implementer runs, the orchestrator uses `bin/sotp track transition <task_id>
in_progress` to mark selected tasks as `in_progress` in `impl-plan.json` (the task-state SSoT;
`metadata.json` carries track identity only). This auto-renders
`plan.md` + `registry.md`. The active track is resolved from the current branch; pass
`--track-id <id>` explicitly only when targeting a different track. Do NOT edit `plan.md`
directly — it is a read-only view rendered from the track artifacts.

**Step 3: Parallel implementation**

Implement the selected tasks in dependency order (lower-layer first, then upper layers that
consume the new lower-layer surface). The order is encoded in the impl-plan sections.

Every dispatch to a source-editing capability must carry the `## Architecture Constraints`
section required by
`.harness/policies/implementation-delegation.md#R1. 委譲時に architecture 制約を注入する`. That
policy owns what the section must state; this workflow owns injecting it into each dispatch.

Parallelism rules:

- Tasks touching independent files may be implemented in parallel.
- Serialize `cargo add`, `cargo update`, and any `Cargo.lock`-changing step through a single
  worker to avoid lock contention, then resume parallel work.
- Parallel workers should prefer `cargo make test` for test validation. Reserve full-suite
  commands and full CI for the integration phase or a single worker to avoid build lock contention.
  To isolate a single test: `cargo nextest run <test_name>` inside the tools container.

**Step 4: Test-obligation binding increments**

Enrollment is not decided here: `obligations.json` and `test-bindings.json` are
materialized by the `type-design` workflow's mandatory terminal derive step (ADR
2026-07-23-0240 D1), and `bin/sotp test-obligation check` applies the artifact-presence
policy in ADR 2026-07-23-0240 D2. This step is limited to incremental binding authoring
against those already-enrolled obligations: run the `obligation-fulfillment` workflow
(`.harness/workflows/track/obligation-fulfillment.md`) — it owns the author → totality →
evaluate → repair loop, including the split between implementer-side authoring and
orchestrator-side `evaluate`. Per-record authoring discipline lives in the `implementer`
capability contract (`.harness/capabilities/implementer.md`). The gate must be passing
before implementation is reported complete.

**Step 5: CI validation**

Before reporting completion, require `cargo make ci` equivalent validation.

Before review is launched on this work, also perform the pre-review verification required by
`.harness/policies/implementation-delegation.md#R2. review 起動前に配置を検証する`. The
`cargo make ci` run above satisfies that rule's `cargo make check-layers` step; its remaining
confirmations are the delegator's own and are covered by no gate.

**Step 6: Prepare or record observations (conditional)**

When the acceptance criterion is AC-05 or an equivalent criterion requiring one completion-time
record for a limited rollout, a standalone `implement` run must **not** create or claim that
track-completion record. Instead, hand the enclosing `full-cycle` workflow the assignment-level
outcome data it has observed: assignment id and lane; provider and effort; completion-condition
or quality result; each applicable gate result; incomplete-output, timeout, and gate-failure
flags; provider-reported credits (or `unavailable`); elapsed time from assignment start to its
recorded completion result or verdict; execution count; and any Terra retry result and
model-regression-candidate determination. Preserve the source references with the handoff and
mark unavailable measurements explicitly; do not infer or omit them. The full-cycle workflow
writes the single auditable record only after the limited rollout has finished and every
assignment outcome is available.

For all other tracks or observation mandates, create or append to
`track/items/<id>/observations.md` only when either the task produced machine-non-verifiable
observations (wall-time measurements, UX confirmation, dogfooding results) worth recording or
`spec.json`'s `acceptance_criteria` explicitly mandates recording to `observations.md`. The file
remains free-form markdown with no required scaffold. Otherwise, skip this step (file absence =
no observations).

**Step 7: Report implementation handoff**

The implementer reports the implemented task ids, changed areas, and verification results to the
orchestrator. The orchestrator keeps successful tasks `in_progress` until the enclosing batch's
DRY fix phase closes; after that phase and CI pass, it marks them `done` before review. It
backfills the commit hash only after the batch commit. If work remains blocked, keep tasks in
`in_progress` and report why.

For an AC-05/equivalent standalone run, this caller-facing report is also the required assignment
handoff to the later `full-cycle` invocation. It must include the complete Step 6 payload and its
auditable evidence references: assignment id/lane; provider/effort; completion-condition or
quality result; applicable gate results; incomplete-output, timeout, and gate-failure flags;
credits or `unavailable`; elapsed time; execution count; Terra retry result; and
model-regression-candidate determination. State `unavailable` for every unavailable measurement.
The report must identify its receiving full-cycle invocation; it is handoff data only, never a
track-completion claim or an early `observations.md` completion record.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1 | Active `track/<id>` branch found | OK / stop |
| 3 | Each source-editing dispatch carries `## Architecture Constraints` | pass / fail |
| 4 | `bin/sotp test-obligation check` exits 0 | pass / fail |
| 5 | `cargo make ci` exits 0 | pass / fail |
| 5 | R2 pre-review placement verification performed | pass / fail |

## Failure / recovery

- **No track branch**: stop immediately. Do not transition tasks or write code. Report the
  situation to the caller.
- **CI failure**: fix the failing gate (fmt, clippy, test, deny, layers, verify-*), re-run
  `cargo make ci`, and continue. The orchestrator must not mark tasks done until every
  post-implementation completion condition has passed.
- **Blocked task**: keep the task in `in_progress`. Report the blocker and the remaining work.
  The `review` + `commit` cycle may proceed for other tasks once the orchestrator has completed
  their pre-review `done` transition after CI and the DRY fix phase.
- **Cargo.lock contention** (parallel workers): serialize the lockfile-changing step through
  one worker, then resume parallel work.

## Outputs

- Source code changes in the working tree (not committed)
- Orchestrator-updated `impl-plan.json` task states (`todo` → `in_progress`; successful tasks
  become `done` after CI and the DRY fix phase, before review; commit hashes are backfilled
  after the batch commit)
- Optional `track/items/<id>/observations.md` (ordinary observations appended when conditions
  are met; an AC-05/equivalent rollout-completion record is written once by `full-cycle` after
  all assignment outcomes exist)
- `plan.md` and `registry.md` regenerated as side effects of task state transitions
- Implemented scope summary and remaining tasks (reported to caller)
- For AC-05/equivalent standalone runs, a complete assignment-level handoff payload with
  evidence references for the receiving `full-cycle` invocation (not a completion record)
- No commit is created by this workflow
