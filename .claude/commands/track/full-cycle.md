---
description: Run per-task implement → review → commit loop for the current track.
---

Canonical command for autonomous per-task implementation in the track workflow.

Requires being on a `track/<id>` branch. If on `plan/<id>`, stop and suggest `/track:activate <id>`.
If on any other branch, stop and suggest switching to the track branch.

## Execution

For each task in `metadata.json` `tasks` array (in order), skip `done` with non-null `commit_hash` and `skipped` tasks:

- **`todo` or `in_progress` tasks** — full cycle:
  1. **Implement**: execute `/track:implement` scoped to this single task.
  2. **Review**: execute `/track:review`. Must reach full model `zero_findings`.
  3. **Commit**: execute `/track:commit` with a commit message generated from the task description.
     After commit, record the hash: `cargo make track-transition <track_dir> <task_id> done --commit-hash <hash>`.

- **`done` with `commit_hash` null** — hash backfill only:
  The task was implemented and committed but the hash was not recorded. Find the commit via `git log` and run:
  `cargo make track-transition <track_dir> <task_id> done --commit-hash <hash>`.

If any step fails, stop the loop and report the failure.
Rerun `/track:full-cycle` resumes correctly because only tasks with a committed hash are skipped.

## Post-loop

After all tasks are committed, update `verification.md` with overall results and `verified_at`.
These bookkeeping changes are uncommitted and will be included in the next review+commit cycle
or picked up by `/track:pr`.

## Behavior

After execution, summarize:
1. Tasks completed (count and IDs)
2. Tasks remaining (if stopped early)
3. Failure details (if any)
4. Recommended next command: `/track:pr` (all done) or targeted fix (stopped)
