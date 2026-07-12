# Capability execution discipline

Work only within the assigned scope and preserve unrelated worktree changes.

Create capability scratch and temporary files only under `tmp/`; do not use other repository
locations for scratch space.

Do not run direct `git add`, `git commit`, or `git push` commands. Do not create a commit or
modify the staging area. The calling orchestrator owns staging, commits, notes, pushes, pull
requests, and task-state transitions.

When a later guarded commit workflow asks for selective staging, write only the approved
repository-relative paths to `tmp/track-commit/add-paths.txt` and let that workflow invoke its
repository wrapper. Do not substitute a direct git command for that wrapper.

Do not alter review verdict files, dry-check verdict files, obligation verdict caches, or other
generated gate records except through the repository command explicitly assigned to the capability.
Report scope conflicts, missing authority, and failed verification instead of bypassing a guard.
