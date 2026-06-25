---
description: Run feature-batch implement → DRY check → review → commit loop for the current track (per-task split only when a per-scope diff ceiling is about to be exceeded).
---

Canonical command for autonomous **feature-batch** implementation in the track workflow.

The default consumption unit is the **feature batch**: all `todo` / `in_progress` tasks of the
active track are implemented in dependency order into the **same working tree** without
intermediate commits, then a single review pass and a single commit close the batch. The batch
is split only when adding the next task would cause some layer's cumulative diff to exceed its
per-scope ceiling (configured in `.harness/config/review-scope.json`).

Requires being on a `track/<id>` branch. If on any other branch, stop and suggest switching to
the correct track branch.

## Step 0 (required before any execution step): Build an execution plan

Read **every** sub-command definition referenced below (`/track:implement`,
`/track:dry-check`, `/track:review`, `/track:commit`) and extract their decision points into a
concrete execution plan. Do NOT treat them as informational background — treat them as a state
machine to execute.

For each sub-command, identify:
- Trigger conditions ("when X happens → do Y immediately")
- Parallelism rules ("launch N agents in parallel, escalate each independently")
- Completion criteria ("full model zero_findings, not just fast model")
- Error/branch handling ("if step fails → stop and report")

Skim-reading produces missed steps and user corrections. Reading the sub-command
definitions and building this plan is the first action; no execution step may begin
until the plan is complete.

## Step 0a: Load per-scope diff ceilings

Read `.harness/config/review-scope.json` (the SSoT for per-scope ceilings, ADR
`2026-06-22-1327-feature-batch-default-inversion` §D3):

- Top-level `default_diff_ceiling_lines: Option<u32>` — global default applied to any scope
  that has no per-group override.
- Per-group `diff_ceiling_lines: Option<u32>` (inside each entry under `groups.<scope>`) —
  per-scope override.

When both are absent for a scope, treat that scope as **unconstrained** (no ceiling). The
ceiling values are loaded by `ReviewScopeConfig::diff_ceiling_for_scope(scope)` at runtime;
this command consults the same values via the JSON config so the planner stays consistent with
the actual classifier behavior. `ScopeName::Other` never has a ceiling (the implicit catch-all
is not a configured review scope).

## Step 0b: Plan the batches

Walk the impl-plan `tasks` array in declared order, skipping only tasks that need no further
work (`done` with non-null `commit_hash`, or `skipped`). Carry `done` with null `commit_hash`
forward as **DonePending**: implementation is already complete, but the task still participates
in DFP, Review, Commit, and D4 hash backfill. Group the remaining `todo` / `in_progress` /
DonePending tasks into one or more **batches** by greedy accumulation:

1. Start a new (empty) batch.
2. For the next task in order, estimate its per-scope diff contribution by classifying the
   task description's listed files through `.harness/config/review-scope.json` patterns; use the
   impl-plan section hint only when the file list is absent or incomplete. Do not infer scope
   from file extension: markdown under `.claude/**` / `.harness/**` is `harness-policy`, while
   `knowledge/adr/**` and `track/items/<track-id>/**` are `plan-artifacts`. For DonePending
   tasks, use the already accumulated working-tree diff/files and do not schedule another
   implementation pass.
3. If this task's own contribution would exceed a configured ceiling for a layer whose current
   batch cumulative diff is still **zero**, a batch boundary cannot make that task compliant.
   If the current batch is non-empty, close the current batch first and re-evaluate the task in a
   fresh batch. If the task still exceeds the ceiling as a singleton, stop before implementation
   and require the task to be split/refined in `impl-plan.json`; do **not** emit an
   over-ceiling singleton batch.
4. If adding this task would cause some layer's **already-non-zero** cumulative diff in the
   current batch to exceed its `diff_ceiling_for_scope`, **close the current batch** (commit
   it as below) and start a new one. The next iteration places this task into the fresh batch.
5. **CN-01 continuation rule**: if the next task only touches layers whose cumulative diff in
   the current batch is still **zero**, and the task's own contribution stays within those
   layers' ceilings, the per-scope ceiling of any other (already-touched) layer is irrelevant —
   append it to the current batch and continue. A batch boundary fires only when the offending
   layer is *both* already in the batch *and* about to exceed; a fresh-layer singleton overflow
   is handled by Rule 3.
6. Repeat until all remaining tasks are placed.

The planner is a heuristic, not a binary gate. When sizing is uncertain (estimates are rough
or the task list is short), bias toward fewer / larger batches and let the per-scope ceiling
serve as the hard upper bound at split time. A single feature with no over-ceiling layer
collapses to one batch.

## Step 0c: Order tasks inside a batch by implementation dependencies

Within a batch, run `/track:implement` in dependency order for `todo` / `in_progress` tasks
only (lower-layer first, then upper layers that consume the new lower-layer surface).
DonePending tasks keep their position in the batch for downstream gates and hash recording, but
skip implementation. The order is encoded in the impl-plan sections (e.g. domain →
infrastructure → cli) and the orchestrator follows it without introducing fresh ordering
judgments.

## Execution (per batch)

For each batch produced by Step 0b, in order:

1. **Implement (batch-scoped)**: invoke `/track:implement` over every `todo` / `in_progress`
   task in this batch. For any DonePending task (`done` with null `commit_hash`), skip
   implementation only; keep the task in the batch so the existing working-tree changes flow
   through DFP, Review, Commit, and D4 same-hash recording. `/track:implement` may parallelise
   inside a batch when tasks touch independent files; otherwise it runs them sequentially in the
   Step 0c order. Do NOT commit between tasks in the same batch — accumulate all changes in the
   working tree.

1b. **Actual-diff guard (hard cap on Step 0b estimates)**: measure the **actual** per-scope
    diff against the ceilings loaded in Step 0a. Step 0b is a planning heuristic over per-task
    *estimates*; this step is the hard cap that prevents an underestimated batch from silently
    bypassing the configured ceilings. Run it at both pre-review mutation boundaries:
    - after implementation finishes and before DFP, to catch underestimated implementation
      batches early; and
    - after DFP returns `skipped` / `completed` and before Review, because DFP can edit files
      and push the same batch over a scope ceiling.

    Procedure:

    1. Compute the actual diff (`additions + deletions`) for each configured scope by
       intersecting the scope's file list (`bin/sotp review files --scope <scope>`) with the
       union of:
       - `git diff --numstat <batch-base> --`, which covers tracked committed, staged, and
         unstaged worktree changes relative to the batch base; and
       - untracked additions from `git ls-files --others --exclude-standard`, counted as
         additions for their full file line count with zero deletions.

       Do this for every scope listed in `.harness/config/review-scope.json` `groups`.
       `<batch-base>` is the HEAD commit at which the current batch started (the prior
       batch's commit, or the track's first commit if this is the first batch). Use the same
       `<batch-base>` for the post-DFP rerun so implementation and DFP mutations are measured
       as one batch diff. The implicit `ScopeName::Other` is exempt (no ceiling, by Step 0a).
    2. Compare each scope's actual line count to its `diff_ceiling_for_scope` value. When the
       ceiling is `None` for a scope, skip the comparison for that scope.
    3. If any scope's actual diff exceeds its ceiling, **halt the batch loop immediately**:
       - Do NOT proceed to DFP, Review, or Commit for this batch.
       - Report the overflowing scopes with their actual line counts and ceiling values.
       - The estimator in Step 0b underestimated this batch; the correct response is one of:
         (a) refine the offending task's description in `impl-plan.json` and re-split via a
         smaller follow-up `impl-plan` revision, (b) raise the ceiling explicitly in
         `.harness/config/review-scope.json` with a justification, or (c) revert / shelf the
         offending edits and re-implement them as a separate batch.
       - This is a hard cap that protects the per-scope review-cost ceiling property of D3 /
         AC-04. Skipping it defeats the workflow's main guarantee.
    4. If every scope is within its ceiling, continue to the next phase: Step 1c (DFP) for the
       post-implementation run, or Step 2 (Review) for the post-DFP/pre-review run.

1c. **DRY fix phase (DFP, once per batch)**: execute `/track:dry-check` once for the
    accumulated batch diff. This runs the whole-codebase DRY gate (single scope, D13) via the
    `dry-fix-lead` (dfl) agent — `sotp dry write` → fix DRY violations → `sotp dry
    check-approved` until the gate passes. DFP runs **before** Review (RFP) and is **loosely
    coupled** to it (D1/OS-01): `/track:dry-check` never invokes `/track:review`; full-cycle
    sequences the two phases here.

    Branch on the dfl terminal state (four **mutually-exclusive** outcomes — never collapse
    `skipped`, `blocked`, and `failed` into one branch):
    - **`skipped`** — `/track:dry-check` Step 0a detected
      `.harness/config/dry-check.json.enabled: false` (or file missing) and did not run dfl.
      Treat as a pass-through equivalent to `completed`, then re-run Step 1b as the
      post-DFP/pre-review guard before proceeding to Review (Step 2). The single SSoT for the
      opt-out lives in `/track:dry-check`; do NOT duplicate the config probe here.
    - **`completed`** — the DRY gate is Approved. Re-run Step 1b as the post-DFP/pre-review
      guard, then proceed to Review (Step 2) only if the batch still fits its ceilings.
    - **`blocked`** — DRY violations remain that dfl could not resolve autonomously (the loop
      exhausted its fix attempts). This is a **DRY-gate outcome, NOT a tooling error**. Halt
      the batch loop immediately, surface the unresolved DRY violation pairs (`bin/sotp dry
      results --track-id <id> --filter violation`), and do **NOT** proceed to Review or
      Commit. Escalate for manual resolution.
    - **`failed`** — an execution / tooling error prevented the loop from running. Stop the
      loop and report the error. Do **NOT** proceed.

2. **Review (single round per batch)**: execute `/track:review` once. The required scopes
   come from `bin/sotp review results`, which auto-classifies the accumulated batch diff
   across every affected layer. Because the diff already spans every layer the batch touched,
   `/track:review`'s scope-independent parallel reviewers run with full parallelism in a
   single round — exactly the property D2 protects.

   Review must reach full-model `zero_findings` in every required scope.

   **Back-edge (RFP → DFP fixpoint)**: review fixes (RFP) edit code, which can reintroduce
   duplication AND can grow the per-scope diff past its ceiling. After Review reaches
   `zero_findings`, **re-run Step 1b (Actual-diff guard), Step 1c (DFP), and the post-DFP
   Step 1b guard before returning to Review or Commit**. Iterate `Actual-diff guard` → DFP →
   `Actual-diff guard` ⇄ RFP until **all three** gates are clean in the same pass (the actual
   diff stays within ceiling after both RFP and DFP mutations, the DRY gate stays Approved, and
   review stays `zero_findings` with no new edits) — that fixpoint is the precondition for
   Commit.

3. **Commit (single commit per batch, same hash for all batch tasks)**: stage **after** the
   final review round (`cargo make add-all` or selective `track-add-paths`), then execute
   `/track:commit` once with a commit message that names the batch (e.g. "Batch A: T002-T004
   …"). Staging before review omits the `review.json` delta from the commit — see
   `/track:commit` Step 1.

   The commit message gate (`cargo make track-commit-message`) enforces the DRY gate as a
   hard precondition: it runs `sotp dry check-approved` after the review gate and refuses to
   emit a commit message while the DRY gate is Blocked — so a `blocked` DFP cannot be
   committed past.

   **D4 same-hash recording**: after the commit succeeds, record the **single** commit hash
   on **every** task in this batch, including DonePending tasks, with
   `bin/sotp track transition <task_id> done --commit-hash <hash>`. The active track is resolved
   from the current branch; pass `--track-id <id>` explicitly only when targeting a different
   track. `TaskStatus::Done` has no `commit_hash` uniqueness constraint; the same hash on
   multiple tasks is the canonical D4 representation of a batch commit.

If any step fails, stop the loop and report the failure.

## Post-loop

After all batches are committed, create or append to `track/items/<id>/observations.md`
**only** when one of the following holds:

- (a) any task produced machine-non-verifiable observations (wall-time measurements, UX
  confirmation, dogfooding results) worth recording, or
- (b) `spec.json`'s `acceptance_criteria` explicitly mandates recording to `observations.md`.

The file is free-form markdown (no scaffold). Otherwise, skip this step (file absence = no
observations).

## Behavior

After execution, summarize:

1. Batches executed (count and the task IDs in each), with per-batch commit hash.
2. Tasks completed (count and IDs).
3. Tasks remaining (if stopped early).
4. Failure details (if any).
5. Recommended next command: `/track:dry-check` → `/track:review` (repeat DFP/RFP to
   fixpoint) → `/track:commit` (for verification changes) or `/track:pr` (all done).
