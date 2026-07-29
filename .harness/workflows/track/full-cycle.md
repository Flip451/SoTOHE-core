# Full-Cycle Workflow SSoT

> Provider-agnostic workflow SSoT for the `full-cycle` track workflow. Both the Claude adapter
> (`.claude/commands/track/full-cycle.md`) and the Codex skill adapter
> (`.agents/skills/track-full-cycle/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Run the autonomous feature-batch implement → DRY check → task completion → review → commit loop for the current
track. The consumption unit is the **declared batch**: the batches declared in
`track/items/<id>/batch-plan.json` `batches[]` are consumed in declaration order — each
batch's member tasks are implemented in dependency order into the same working tree without
intermediate commits, then a single DFP + review pass + commit close the batch. This workflow
performs no batch composition of its own: sizing, ceilings, and admission are planning- and
admission-domain concerns, not execution concerns. Requires being on a `track/<id>` branch.

Sub-workflows used:

- `.harness/workflows/track/implement.md`
- `.harness/workflows/track/dry-check.md`
- `.harness/workflows/track/review.md`
- `.harness/workflows/track/commit.md`

## Inputs

- **Current branch** — must match `track/<id>`. If not, stop and suggest switching.
- **`impl-plan.json`** — task list with status and declared dependencies.
- **`batch-plan.json`** — the declared ordered `batches[]` (read-only here; impl-planner is its
  sole writer). For a track planned after this artifact's introduction, absence on the active
  track is an error (fail-closed). The track that introduced the mechanism predates its own
  Phase 3 output and completes under orchestrator-driven batching — that is the only
  declaration-less lane, per the non-retroactivity constraint.
- **`spec.md`, `plan.md`, `metadata.json`** — task context for the implement sub-workflow.

## Sequence

### Step 0: Build an execution plan (required before any execution)

Read every sub-workflow definition referenced in this workflow and extract their decision points
into a concrete execution plan. Treat them as a state machine to execute, not background reading.

**Step 0a: Load the declared batches**

Read `track/items/<id>/batch-plan.json` and take `batches[]` as the execution sequence,
in declaration order. This workflow composes no batches and re-checks no ceilings: the
declared composition already passed the Phase 3 structural check (`bin/sotp batch-plan
check`), and execution never re-judges admission.

When `batch-plan.json` is absent: for a track planned after the artifact's introduction this
is an error — stop and route to Phase 3 (impl-planner) instead of composing batches here. For
the introducing track only (its Phase 3 predates its own output), fall back to
orchestrator-driven batching: the orchestrator selects the remaining `todo` / `in_progress`
tasks into batches at its own judgment, honouring `depends_on` order; every other step of this
workflow applies unchanged.

Select the current batch: the first batch in declaration order that still has a member task
not yet `done`-with-`commit_hash` (skip `skipped` members). Within it, carry a `done` member
with null `commit_hash` forward as **DonePending** (implementation complete, but still
participates in DFP, Review, Commit, and post-commit task hash backfill).

**Step 0b: Order tasks inside the batch by implementation dependencies**

Within the batch, run `implement` in dependency order for `todo` / `in_progress` tasks only,
honouring the `depends_on` edges declared in `impl-plan.json` (lower-layer first where no
edge dictates otherwise). DonePending tasks keep their position for downstream gates.

### Execution (per batch)

**Step 1: Implement (batch-scoped)**

Invoke the `implement` workflow (`.harness/workflows/track/implement.md`) over every `todo`
task in this batch in Step 0b order. Each task enters implementation through its
`todo → in_progress` transition; if the transition's admission judgment **rejects** a later
member (e.g. the ceiling would be exceeded by its estimate on top of the prior contribution),
apply the runtime split: close the current execution unit at that task boundary — the
already-admitted prefix proceeds through DFP → Review → Commit as this unit — and the
rejected member starts the next execution unit (which re-attempts its transition against the
post-commit baseline). Runtime may only split a declared batch this way, never merge batches.
From the split onward, every later per-batch step (Step 1d done-marking, Step 3 commit and
hash backfill) operates on the **admitted execution unit's task set**, not the full declared
batch; the rejected member stays `todo` and is untouched until its own unit.

For an `in_progress` task, first check whether its
working-tree implementation already exists (a prior standalone `/track:implement` hands tasks
off `in_progress` without transitioning them): if the task's implementation is already present
and CI passes, skip re-implementation and carry it forward like a DonePending task; only
(re-)implement an `in_progress` task whose work is absent or incomplete. For DonePending tasks,
skip implementation only — keep the task in the batch so its working-tree changes flow through
DFP, Review, Commit, and post-commit task hash recording. Do NOT commit between tasks in the
same batch.

Test-obligation handoff: a catalogue-bearing track enters this loop already enrolled
(`obligations.json` + `test-bindings.json` from the `type-design` workflow's terminal derive
step, ADR 2026-07-23-0240 D1). The `implement` workflow's Step 4 authors binding increments
against those obligations per batch; this workflow adds no enrollment decision of its own,
and the commit gate's `sotp test-obligation check` fail-closes if the artifacts are missing.

No actual-diff measurement happens in this workflow: the ceiling concept exists only in the
planning and admission domains. Once implementation has started, diff growth — including
growth from review fixes and DRY fixes — is never measured against or treated as exceeding a
ceiling; actual measurement is read only as an admission baseline, never here.

**Step 1c: DRY fix phase (DFP, once per batch)**

Invoke the `dry-check` workflow (`.harness/workflows/track/dry-check.md`) once for the
accumulated batch diff. DFP runs **before** Review (RFP) and is loosely coupled to it.

Branch on the dfl terminal state (four mutually-exclusive outcomes):

- **`skipped`**: treat as equivalent to `completed`; proceed to Review (Step 2).
- **`completed`**: DRY gate Approved; proceed to Review (Step 2).
- **`blocked`**: halt the batch loop immediately. Surface unresolved DRY violation pairs
  (`bin/sotp dry results --track-id <id> --filter violation`). Do NOT proceed to Review or
  Commit. Escalate for manual resolution.
- **`failed`**: stop the loop and report the error. Do NOT proceed.

**Post-DFP R2 repeat.** On the `completed` path, dfl edited the working tree after the
`implement` workflow's Step 5 verification ran, so before proceeding to Review repeat the
pre-review verification required by
`.harness/policies/implementation-delegation.md#R2. review 起動前に配置を検証する`. That policy
owns the verification's steps; this workflow owns repeating it at every boundary where source is
mutated between that verification and Review. The `skipped` path launches no dfl and mutates
nothing, so the Step 5 verification still holds and no repeat is required there.

**Step 1d: Orchestrator marks completed tasks done**

After CI passes and DFP reaches `skipped` or `completed`, the orchestrator marks each successfully
implemented `todo` / `in_progress` task in the batch `done` before Review. DonePending tasks are
already `done` and require no state transition at this point. The orchestrator does not record a
commit hash yet; it backfills that hash only after the batch commit in Step 3.

**Step 2: Review (single round per batch)**

Invoke the `review` workflow (`.harness/workflows/track/review.md`) once. Required scopes come
from `bin/sotp review results`, which auto-classifies the accumulated batch diff. Review must
reach full-model `zero_findings` in every required scope.

**Back-edge (RFP → DFP fixpoint)**: review fixes can reintroduce duplication. After Review
reaches `zero_findings`, re-run Step 1c (DFP) before returning to Review or Commit. On this
back-edge the review fixers themselves mutated source, so the Step 1c R2 repeat applies
unconditionally — perform it before returning to Review, whatever the back-edge DFP's terminal
state. Iterate until the DRY gate stays Approved and review stays `zero_findings` with no new
edits. Diff growth introduced by review fixes or DRY fixes is never treated as a ceiling
overrun (the ceiling concept does not exist in this domain).

**Step 3: Commit (single commit per batch)**

Stage **after** the final review round (`bin/sotp git add-all` or selective
`bin/sotp git add-from-file tmp/track-commit/add-paths.txt --cleanup`),
then invoke the `commit` workflow (`.harness/workflows/track/commit.md`) once with a commit
message naming the batch (e.g., "Batch A: T002-T004 …").

The `commit` workflow enforces the track-aware gates as hard preconditions via
`cargo make track-commit-message` (including `cargo make ci-track`,
`sotp test-obligation check`, and `sotp dry check-approved` before committing).
A `blocked` DFP or failing test-obligation gate cannot be committed past.

**Orchestrator post-commit task hash recording**: after the commit succeeds, the orchestrator
backfills the single commit hash on every task in this execution unit — the admitted task set
when a runtime split occurred, otherwise the declared batch — including DonePending tasks, with:

```
bin/sotp track transition <task_id> done --commit-hash <hash>
```

`TaskStatus::Done` has no `commit_hash` uniqueness constraint; the same hash on multiple
tasks is the canonical representation of a batch commit.

### Step 4: Lifecycle tail commit (after all batches)

Post-commit task hash recording (Step 3) writes the commit hash to `impl-plan.json` *after*
the batch commit — the hash cannot exist before the commit. For the last batch, no successor
batch captures these writes.

Procedure (after Step 3 of the **last** batch):

1. Inspect the working tree with `git status --short`. Expect modifications limited to
   `track/items/<track-id>/impl-plan.json` and `track/items/<track-id>/plan.md` only.
2. If those (and only those) files are modified, run a tail review refresh before committing:
   - Invoke the `review` workflow. Expected required scope: `impl-plan` (the tail diff is
     only the task hash backfill in `impl-plan.json`; the rendered `plan.md` is
     review-operational and does not affect scope hashes).
   - Continue only after `bin/sotp review check-approved` succeeds and:
     `bin/sotp review results --track-id <track-id> --scope impl-plan --round-type final --limit 1`
     shows a recorded final `zero_findings` round for the tail diff.
   - This review refresh is mandatory: `cargo make track-commit-message` runs
     `bin/sotp review check-approved` before committing, and after Step 3 mutates
     `impl-plan.json` / `plan.md`, the previous `impl-plan` review hash is stale.
3. After the tail review refresh succeeds, stage and commit the lifecycle diff:
   1. Run `bin/sotp git add-all` to stage the task hash backfill (plus any review-operational artifacts produced by Step 2's review refresh, e.g. `review.json` / `<layer>-type-signals.json`).
   2. Write the lifecycle tail commit message to `tmp/track-commit/commit-message.txt`. The wrapper in the next step reads this exact path (`bin/sotp git commit-from-file tmp/track-commit/commit-message.txt --cleanup`), so the file must exist before invoking it. A typical message is:

      ```
      ops(track): backfill task commit hashes for batch <name>
      ```
   3. Run `cargo make track-commit-message`. The wrapper runs CI + `cargo make ci-track` + `bin/sotp review check-approved` + the test-obligation gate + the DRY-gate precondition, then commits from the file and deletes it on success.
   4. (Optional, recommended) Attach a git note via `bin/sotp git note-from-file tmp/track-commit/note.md --cleanup`.
4. If no `impl-plan.json` / `plan.md` modifications were present in Step 1, skip this step.
5. After Step 2, dirty files may include the two plan artifacts plus review-operational
   artifacts produced by the refresh and staged by Step 3. If `git status --short` shows any
   other files before the commit, stop and report. After `cargo make track-commit-message`
   succeeds, `git status --short` must be empty; any remaining dirty file is unexpected and
   must be investigated before declaring the loop complete.

The workflow completes only when `git status --short` is empty after this step.

### Post-loop

After all batches are committed and the optional lifecycle tail commit is recorded, create or
append to `track/items/<id>/observations.md` only when:

- (a) Any task produced machine-non-verifiable observations worth recording, or
- (b) `spec.json`'s `acceptance_criteria` explicitly mandates recording to `observations.md`.

Otherwise, skip (file absence = no observations).

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1 | Track branch and active tasks found | OK / stop |
| 1c | DFP terminal state | skipped/completed → Step 1d; blocked/failed → halt |
| 1c | R2 placement verification repeated after a `completed` DFP | pass / fail |
| 1d | Successful batch tasks marked `done` before review | OK / stop |
| 2 | Review `zero_findings` all required scopes | completed / blocked / failed |
| 2 | R2 placement verification repeated on each back-edge pass | pass / fail |
| 3 | `cargo make track-commit-message` (CI + track-aware gates + DRY check) | OK / ERROR |
| 4 | `git status --short` empty | OK / unexpected dirty state |

## Failure / recovery

- **Wrong branch**: stop and suggest switching to `track/<id>`.
- **DFP `blocked`**: halt the loop. Surface violation pairs. Do not proceed to review.
- **DFP `failed`**: stop and report tooling error.
- **Review `blocked_cross_scope`**: fix cross-scope dependencies, then relaunch the affected scope.
- **Review `failed` / timeout**: relaunch (up to 2 retries per fixer), then report.
- **Commit failure**: fix CI or staging issue. Do not re-stage until the issue is resolved.
- **Unexpected dirty state in Step 4**: stop and investigate before declaring completion.

## Outputs

- Commits on the current `track/<id>` branch, one per batch + optional lifecycle tail
- Commit hashes recorded by the orchestrator on all batch tasks via
  `bin/sotp track transition done --commit-hash`
- Optional `track/items/<id>/observations.md`
- Summary: batches executed (task IDs, commit hash per batch), tasks completed, tasks remaining,
  any failures, recommended next command (`pr-review` workflow)
