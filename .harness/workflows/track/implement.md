# Implement Workflow SSoT

> Provider-agnostic workflow SSoT for the `implement` track workflow. Both the Claude adapter
> (`.claude/commands/track/implement.md`) and the Codex skill adapter
> (`.agents/skills/track-implement/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Run parallel interactive implementation for the current track. The orchestrator consumes the
approved plan through CLI summaries and selected task briefings, marks selected tasks
`in_progress`, delegates implementation using the available parallelism of the execution
environment, and runs CI to verify correctness.
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
- **Track context** — CLI summaries for the active track, the selected task briefing, and the
  exact artifact paths needed by the delegated implementer. The implementer reads the relevant
  spec, plan, metadata, catalogue entries, and task contract from those paths; the orchestrator
  does not bulk-read them. Rendered views provide context but do not replace the capability's
  machine-readable inputs.
- **Upstream design consistency** — the approved spec/plan and phase summaries are the design
  input. Do not perform a separate ADR pre-check, read ADR bodies during normal intake, or edit
  `metadata.json` to repair a discrepancy. The delegated implementer receives the exact paths
  from its briefing and owns implementation-side verification. If a briefing, summary, or
  blocker exposes a design inconsistency, stop implementation and route it to the owning
  spec/plan phase (and, where applicable, the ADR-edit lane) before writing code.
- **Optional scope notes** — caller-supplied hints (target module, constraints, priority) that
  narrow the set of tasks to implement.

## Summary-first context intake

Before selecting or dispatching tasks, collect these summaries:

- `bin/sotp track resolve`, `bin/sotp track task-counts`, and `bin/sotp track next-task` for
  phase and progress;
- `bin/sotp review results` for the scopes that need review;
- `bin/sotp test-obligation results` for enrollment and fulfillment state when the track is
  enrolled; and
- `bin/sotp catalog check` plus `bin/sotp ref-verify results --chain 2 --filter all` for catalogue
  completion and catalogue-to-specification state.

Treat the command output and the task briefing as primary. Do not open `*-types.json`,
`review.json`, bindings JSON, full sub-workflow texts, or a `Related Conventions` list during
intake. Open only a targeted diff or the artifact body named by a blocker. The dispatcher lists
resolved convention paths alongside the implementer briefing (possibly none); the delegated
capability reads those paths, while the orchestrator does not enumerate or read conventions
itself.

## Sequence

**Step 1: Resolve track and validate context**

1. Resolve the current track:
   - Require the current git branch to match `track/<id>` and resolve the track id from that
     branch.
   - If the branch does not match this pattern or the matching track is not materialized, stop
     immediately and report the situation. Do not fall back to another active track, transition
     tasks, or write implementation code.
2. Use the summaries above to identify the target task(s) from the approved plan. If scope notes
   are provided, map them to the relevant plan scope.
3. Prepare each implementer briefing with the selected task ids, the summary output, exact
   artifact paths, and the convention paths supplied by the dispatcher. The delegated capability
   reads those paths and reports any upstream or scope conflict; the orchestrator opens an
   artifact body only for a diff or blocker investigation.

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
materialized by the `type-design` workflow's mandatory terminal derive step, and
`bin/sotp test-obligation check` applies the artifact-presence policy. Use
`bin/sotp test-obligation results` as the obligation-state summary before entering this step.
This step is limited to incremental binding authoring
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

**Step 6: Record observations (conditional)**

After CI passes, create or append to `track/items/<id>/observations.md` only when one of the
following holds:

- (a) The task produced machine-non-verifiable observations (wall-time measurements, UX
  confirmation, dogfooding results) worth recording.
- (b) `spec.json`'s `acceptance_criteria` explicitly mandates recording to `observations.md`.

The file is free-form markdown with no required scaffold. Otherwise, skip this step
(file absence = no observations).

**Step 7: Report implementation handoff**

The implementer reports the implemented task ids, changed areas, and verification results to the
orchestrator. The orchestrator keeps successful tasks `in_progress` until the enclosing batch's
DRY fix phase closes; after that phase and CI pass, it marks them `done` before review. It
backfills the commit hash only after the batch commit. If work remains blocked, keep tasks in
`in_progress` and report why.

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
- Optional `track/items/<id>/observations.md` (appended if conditions are met)
- `plan.md` and `registry.md` regenerated as side effects of task state transitions
- Implemented scope summary and remaining tasks (reported to caller)
- No commit is created by this workflow
