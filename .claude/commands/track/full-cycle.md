---
description: Run per-task implement → review → commit loop for the current track.
---

Canonical command for autonomous per-task implementation in the track workflow.

Requires being on a `track/<id>` branch. If on `plan/<id>`, stop and suggest `/track:activate <id>`.
If on any other branch, stop and suggest switching to the track branch.

## Step 0 (required before any execution step): Build an execution plan

Read **every** sub-command definition referenced below (`/track:implement`,
`/track:review`, `/track:commit`) and extract their decision points into a concrete
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

- **`todo` or `in_progress`**: run all three steps (implement → review → commit).
- **`done` with null `commit_hash`**: implementation is complete but not yet committed.
  Skip step 1 and run steps 2-3 only (review → commit).

Steps:

1. **Implement**: execute `/track:implement` scoped to this single task.
2. **Review**: execute `/track:review`. Must reach full model `zero_findings`.
3. **Commit**: stage **after** the final review round (`cargo make add-all` or selective `track-add-paths`), then execute `/track:commit` with a commit message generated from the task description. Staging before review omits the `review.json` delta from the commit — see `/track:commit` Step 1.
   After commit, record the hash: `cargo make track-transition <track_dir> <task_id> done --commit-hash <hash>`.

If any step fails, stop the loop and report the failure.

## Post-loop

After all tasks are committed, update `verification.md` with overall results and `verified_at`.

## Behavior

After execution, summarize:
1. Tasks completed (count and IDs)
2. Tasks remaining (if stopped early)
3. Failure details (if any)
4. Recommended next command: `/track:review` → `/track:commit` (for verification changes) or `/track:pr` (all done)
