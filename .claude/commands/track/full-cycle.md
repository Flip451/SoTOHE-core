---
description: Run per-task implement → review → commit loop for the current track.
---

Canonical command for autonomous per-task implementation in the track workflow.

Requires being on a `track/<id>` branch. If on `plan/<id>`, stop and suggest `/track:activate <id>`.
If on any other branch, stop and suggest switching to the track branch.

## Execution

For each task in `metadata.json` `tasks` array (in order) where `status` is `todo` or `in_progress`:

1. **Implement**: execute `/track:implement` scoped to this single task.
   `/track:implement` handles implementation, CI, and verification update.
   Note: `/track:implement` normally marks the task `done` and suggests `/track:commit`,
   but within the full-cycle loop the orchestrator proceeds to review before committing.
2. **Review**: execute `/track:review`. Reviews the implementation including all changes.
   Must reach full model `zero_findings`.
3. **Commit**: execute `/track:commit` with a commit message generated from the task description.

If any step fails, stop the loop and report the failure.
Rerun `/track:full-cycle` resumes from the first `todo` or `in_progress` task.

## Post-loop

After all tasks complete, update `verification.md` with overall results and `verified_at`.
Post-loop changes are included in the next `/track:commit` or `/track:pr` — no separate commit is needed here.

## Behavior

After execution, summarize:
1. Tasks completed (count and IDs)
2. Tasks remaining (if stopped early)
3. Failure details (if any)
4. Recommended next command: `/track:pr` (all done) or targeted fix (stopped)
