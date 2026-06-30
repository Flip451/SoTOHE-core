# Full-Cycle Workflow SSoT

> Provider-agnostic workflow SSoT for the `full-cycle` track workflow. Both the Claude adapter
> (`.claude/commands/track/full-cycle.md`) and the Codex skill adapter
> (`.agents/skills/track-full-cycle/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Run the autonomous feature-batch implement → DRY check → review → commit loop for the current
track. The default consumption unit is the **feature batch**: all `todo` / `in_progress` tasks
are implemented in dependency order into the same working tree without intermediate commits,
then a single DFP + review pass + commit close the batch. The batch is split only when adding
the next task would cause some layer's cumulative diff to exceed its per-scope ceiling
(configured in `.harness/config/review-scope.json`). Requires being on a `track/<id>` branch.

Sub-workflows used:

- `.harness/workflows/track/implement.md`
- `.harness/workflows/track/dry-check.md`
- `.harness/workflows/track/review.md`
- `.harness/workflows/track/commit.md`

## Inputs

- **Current branch** — must match `track/<id>`. If not, stop and suggest switching.
- **`impl-plan.json`** — task list with status and per-task scope hints.
- **`.harness/config/review-scope.json`** — per-scope diff ceilings SSoT:
  - `default_diff_ceiling_lines: Option<u32>` — global default.
  - Per-group `diff_ceiling_lines: Option<u32>` — per-scope override.
  When both are absent for a scope, treat that scope as unconstrained (no ceiling).
  `ScopeName::Other` never has a ceiling.
- **`spec.md`, `plan.md`, `metadata.json`** — task context for the implement sub-workflow.

## Sequence

### Step 0: Build an execution plan (required before any execution)

Read every sub-workflow definition referenced in this workflow and extract their decision points
into a concrete execution plan. Treat them as a state machine to execute, not background reading.

**Step 0a: Load per-scope diff ceilings**

Read `.harness/config/review-scope.json`. Load `default_diff_ceiling_lines` and per-group
`diff_ceiling_lines` values. When both are absent for a scope, treat it as unconstrained.

**Step 0b: Plan the batches**

Walk the impl-plan `tasks` array in declared order, skipping tasks with `done` + non-null
`commit_hash`, or `skipped`. Carry `done` with null `commit_hash` forward as **DonePending**
(implementation complete, but still participates in DFP, Review, Commit, and D4 hash backfill).
Group the remaining `todo` / `in_progress` / DonePending tasks into batches by greedy
accumulation:

1. Start a new (empty) batch.
2. For the next task in order, estimate its per-scope diff contribution by classifying the
   task description's listed files through `.harness/config/review-scope.json` patterns.
   Do not infer scope from file extension alone: markdown under `.claude/**` / `.harness/**`
   is `harness-policy`; `knowledge/adr/**` and `track/items/<track-id>/**` are `plan-artifacts`.
   For DonePending tasks, use the already-accumulated working-tree diff.
3. If this task's own contribution would exceed a configured ceiling for a layer whose current
   batch cumulative diff is still **zero**, and the current batch is non-empty, close the current
   batch and re-evaluate the task in a fresh batch. If the task still exceeds the ceiling as a
   singleton, emit it as an over-ceiling singleton batch and log the overflow (advisory, not
   a hard halt).
4. If adding this task would cause a layer's **already-non-zero** cumulative diff in the current
   batch to exceed its ceiling, close the current batch and start a new one.
5. **CN-01 continuation rule**: if the next task only touches layers whose cumulative diff in
   the current batch is still zero, and the task's own contribution stays within those layers'
   ceilings, the ceiling of any other already-touched layer is irrelevant — append the task.
6. Repeat until all remaining tasks are placed.

The planner is a heuristic, not a binary gate. When sizing is uncertain, bias toward fewer /
larger batches.

**Step 0c: Order tasks inside a batch by implementation dependencies**

Within a batch, run `implement` in dependency order for `todo` / `in_progress` tasks only
(lower-layer first, then upper layers). DonePending tasks keep their position for downstream
gates. The order is encoded in the impl-plan sections.

### Execution (per batch)

**Step 1: Implement (batch-scoped)**

Invoke the `implement` workflow (`.harness/workflows/track/implement.md`) over every
`todo` / `in_progress` task in this batch in Step 0c order. For DonePending tasks, skip
implementation only — keep the task in the batch so its working-tree changes flow through DFP,
Review, Commit, and D4 hash recording. Do NOT commit between tasks in the same batch.

**Step 1b: Actual-diff guard (advisory ceiling visibility)**

Measure the **actual** per-scope diff against the ceilings loaded in Step 0a. This is run at
two points:

- After implementation finishes and before DFP.
- After DFP returns `skipped` / `completed` and before Review.

Procedure:

1. Compute `additions + deletions` for each configured scope by intersecting the scope's file
   list (`bin/sotp review files --scope <scope>`) with the union of:
   - `git diff --numstat <batch-base> --` (tracked committed, staged, and unstaged changes
     relative to the batch-base commit — the HEAD at which the current batch started).
   - Untracked additions from `git ls-files --others --exclude-standard` (counted as additions
     for their full file line count with zero deletions).
2. Compare each scope's actual line count to its `diff_ceiling_for_scope` value. Skip
   comparisons for scopes with no ceiling.
3. If any scope's actual diff exceeds its ceiling: **log the overflow (scope name, actual count,
   ceiling value) and continue**. The ceiling is advisory; do not halt, revert, or require user
   judgment. Record the overflow for future impl-plan refinement if useful.

**Step 1c: DRY fix phase (DFP, once per batch)**

Invoke the `dry-check` workflow (`.harness/workflows/track/dry-check.md`) once for the
accumulated batch diff. DFP runs **before** Review (RFP) and is loosely coupled to it.

Branch on the dfl terminal state (four mutually-exclusive outcomes):

- **`skipped`**: treat as equivalent to `completed`. Re-run Step 1b as the post-DFP/pre-review
  guard before proceeding to Review (Step 2).
- **`completed`**: DRY gate Approved. Re-run Step 1b, then proceed to Review (Step 2).
- **`blocked`**: halt the batch loop immediately. Surface unresolved DRY violation pairs
  (`bin/sotp dry results --track-id <id> --filter violation`). Do NOT proceed to Review or
  Commit. Escalate for manual resolution.
- **`failed`**: stop the loop and report the error. Do NOT proceed.

**Step 2: Review (single round per batch)**

Invoke the `review` workflow (`.harness/workflows/track/review.md`) once. Required scopes come
from `bin/sotp review results`, which auto-classifies the accumulated batch diff. Review must
reach full-model `zero_findings` in every required scope.

**Back-edge (RFP → DFP fixpoint)**: review fixes can reintroduce duplication and shift
per-scope diff totals. After Review reaches `zero_findings`, re-run Step 1b, Step 1c (DFP),
and the post-DFP Step 1b guard before returning to Review or Commit. Iterate until the same
pass has Step 1b measurements recorded at both mutation boundaries, the DRY gate stays
Approved, and review stays `zero_findings` with no new edits. Ceiling overflow remains
advisory and does not block convergence.

**Step 3: Commit (single commit per batch)**

Stage **after** the final review round (`cargo make add-all` or selective `track-add-paths`),
then invoke the `commit` workflow (`.harness/workflows/track/commit.md`) once with a commit
message naming the batch (e.g., "Batch A: T002-T004 …").

The `commit` workflow enforces the DRY gate as a hard precondition via
`cargo make track-commit-message` (which runs `sotp dry check-approved` before committing).
A `blocked` DFP cannot be committed past.

**D4 same-hash recording**: after the commit succeeds, record the single commit hash on every
task in this batch (including DonePending tasks) with:

```
bin/sotp track transition <task_id> done --commit-hash <hash>
```

`TaskStatus::Done` has no `commit_hash` uniqueness constraint; the same hash on multiple
tasks is the canonical D4 representation of a batch commit.

### Step 4: Lifecycle tail commit (after all batches)

D4 same-hash recording (Step 3) writes the commit hash to `impl-plan.json` *after* the batch
commit — the hash cannot exist before the commit. For the last batch, no successor batch
captures these writes.

Procedure (after Step 3 of the **last** batch):

1. Inspect the working tree with `git status --short`. Expect modifications limited to
   `track/items/<track-id>/impl-plan.json` and `track/items/<track-id>/plan.md` only.
2. If those (and only those) files are modified, run a tail review refresh before committing:
   - Invoke the `review` workflow. Expected required scope: `plan-artifacts` (the tail diff is
     only the D4 backfill in the current track's plan artifacts).
   - Continue only after `bin/sotp review check-approved` succeeds and:
     `bin/sotp review results --track-id <track-id> --scope plan-artifacts --round-type final --limit 1`
     shows a recorded final `zero_findings` round for the tail diff.
   - This review refresh is mandatory: `cargo make track-commit-message` runs
     `bin/sotp review check-approved` before committing, and after Step 3 mutates
     `impl-plan.json` / `plan.md`, the previous `plan-artifacts` review hash is stale.
3. After the tail review refresh succeeds, stage and commit the lifecycle diff:
   1. Run `cargo make add-all` to stage the D4 backfill (plus any review-operational artifacts produced by Step 2's review refresh, e.g. `review.json` / `<layer>-type-signals.json`).
   2. Write the lifecycle tail commit message to `tmp/track-commit/commit-message.txt`. The wrapper in the next step reads this exact path (`bin/sotp git commit-from-file tmp/track-commit/commit-message.txt --cleanup`), so the file must exist before invoking it. A typical message is:

      ```
      ops(track): D4 hash backfill for batch <name> (post-commit lifecycle)
      ```
   3. Run `cargo make track-commit-message`. The wrapper runs CI + `bin/sotp review check-approved` + the DRY-gate precondition, then commits from the file and deletes it on success.
   4. (Optional, recommended) Attach a git note via `cargo make track-note` (write `tmp/track-commit/note.md` first; the wrapper consumes that path).
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
| 1c | DFP terminal state | skipped/completed → proceed; blocked/failed → halt |
| 2 | Review `zero_findings` all required scopes | completed / blocked / failed |
| 3 | `cargo make track-commit-message` (CI + DRY check) | OK / ERROR |
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
- Commit hashes recorded on all batch tasks via `bin/sotp track transition done --commit-hash`
- Optional `track/items/<id>/observations.md`
- Summary: batches executed (task IDs, commit hash per batch), tasks completed, tasks remaining,
  any failures, recommended next command (`pr-review` workflow)
