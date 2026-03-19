<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Block PR push when track has unresolved tasks

Add a guard to sotp pr push that blocks when the track has unresolved tasks (not done/skipped).
Prevents the workflow mistake of pushing before task state transitions, which forces main-branch direct commits post-merge.
Enforces by mechanism (domain guard), not by memory or prompt instructions.

## Domain layer: task completion check

Add pub fn all_tasks_resolved(&self) -> bool to TrackMetadata
Returns true if every task has status done or skipped
Pure domain logic, no I/O

- [x] Domain: add all_tasks_resolved() method to TrackMetadata

## CLI guard in pr push

In apps/cli/src/commands/pr.rs push(), after resolve_branch_context() and before repo.push_branch()
Read metadata.json via FsTrackStore or direct decode
Call track.all_tasks_resolved()
If false: print [BLOCKED] with list of unresolved task IDs and statuses, return ExitCode::FAILURE
If true: proceed with push
Skip guard entirely when branch starts with plan/ (planning-only branches have no code tasks)

- [x] CLI: add task completion guard to sotp pr push before git push

## Tests

Domain unit test: all_tasks_resolved returns false with mixed states, true with all done/skipped
CLI integration test: push() returns FAILURE when tasks are unresolved
CLI integration test: push() skips guard on plan/ branches (AC3)

- [x] Tests: guard blocks on unresolved tasks, passes on all done/skipped
