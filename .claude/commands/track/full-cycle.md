---
description: Run per-task implement → DRY check → review → commit loop for the current track.
---

Canonical command for autonomous per-task implementation in the track workflow.

Requires being on a `track/<id>` branch. If on any other branch, stop and suggest switching to the correct track branch.

## Step 0 (required before any execution step): Build an execution plan

Read **every** sub-command definition referenced below (`/track:implement`,
`/track:dry-check`, `/track:review`, `/track:commit`) and extract their decision points into a concrete
execution plan. Do NOT treat them as informational background — treat them as a state
machine to execute.

For each sub-command, identify:
- Trigger conditions ("when X happens → do Y immediately")
- Parallelism rules ("launch N agents in parallel, escalate each independently")
- Completion criteria ("full model zero_findings, not just fast model")
- Error/branch handling ("if step fails → stop and report")

Skim-reading produces missed steps and user corrections. Reading the sub-command
definitions and building this plan is the first action; no execution step may begin
until the plan is complete.

## Execution

For each task in `metadata.json` `tasks` array (in order),
skip `done` with non-null `commit_hash` and `skipped` tasks:

- **`todo` or `in_progress`**: run all execution steps (implement → DFP → review/DFP fixpoint → commit).
- **`done` with null `commit_hash`**: implementation is complete but not yet committed.
  Skip only Step 1; run Step 1b and Steps 2-3 (DFP → review/DFP fixpoint → commit).

Steps:

1. **Implement**: execute `/track:implement` scoped to this single task.
1b. **DRY fix phase (DFP)**: execute `/track:dry-check` for this track. This runs the
   whole-codebase DRY gate (single scope, D13) via the `dry-fix-lead` (dfl) agent —
   `sotp dry write` → fix DRY violations → `sotp dry check-approved` until the gate passes.
   DFP runs **before** Review (RFP) and is **loosely coupled** to it (D1/OS-01): `/track:dry-check`
   never invokes `/track:review`; full-cycle sequences the two phases here.
   Branch on the dfl terminal state (three **mutually-exclusive** outcomes — never collapse
   `blocked` and `failed` into one branch):
   - **`completed`** — the DRY gate is Approved. Proceed to Review (Step 2).
   - **`blocked`** — DRY violations remain that dfl could not resolve autonomously (the loop
     exhausted its fix attempts). This is a **DRY-gate outcome, NOT a tooling error**. Halt the
     per-task loop immediately, surface the unresolved DRY violation pairs (`bin/sotp dry results
     --track-id <id> --filter violation`), and do **NOT** proceed to Review or Commit. Escalate
     for manual resolution.
   - **`failed`** — an execution / tooling error prevented the loop from running. Stop the loop
     and report the error. Do **NOT** proceed.
2. **Review**: execute `/track:review`. Must reach full model `zero_findings`.
   **Back-edge (RFP → DFP fixpoint)**: review fixes (RFP) edit code, which can reintroduce
   duplication. After Review reaches `zero_findings`, **re-run Step 1b (DFP)**. Iterate
   DFP ⇄ RFP until **both** gates are clean in the same pass (the DRY gate stays Approved and
   review stays `zero_findings` with no new edits) — that fixpoint is the precondition for Commit.
3. **Commit**: stage **after** the final review round (`cargo make add-all` or selective `track-add-paths`), then execute `/track:commit` with a commit message generated from the task description. Staging before review omits the `review.json` delta from the commit — see `/track:commit` Step 1.
   The commit message gate (`cargo make track-commit-message`) enforces the DRY gate as a hard
   precondition: it runs `sotp dry check-approved` after the review gate and refuses to emit a
   commit message while the DRY gate is Blocked — so a `blocked` DFP cannot be committed past.
   After commit, record the hash: `cargo make track-transition -- <task_id> done --commit-hash <hash>`. The active track is resolved from the current branch; pass `--track-id <id>` explicitly only when targeting a different track.

If any step fails, stop the loop and report the failure.

## Post-loop

After all tasks are committed, create or append to `track/items/<id>/observations.md` **only** when one of the following holds:

- (a) any task produced machine-non-verifiable observations (wall-time measurements, UX confirmation, dogfooding results) worth recording, or
- (b) `spec.json`'s `acceptance_criteria` explicitly mandates recording to `observations.md`.

The file is free-form markdown (no scaffold). Otherwise, skip this step (file absence = no observations).

## Behavior

After execution, summarize:

1. Tasks completed (count and IDs)
2. Tasks remaining (if stopped early)
3. Failure details (if any)
4. Recommended next command: `/track:dry-check` → `/track:review` (repeat DFP/RFP to fixpoint) → `/track:commit` (for verification changes) or `/track:pr` (all done)
