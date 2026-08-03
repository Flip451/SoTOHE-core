---
name: track-recover
description: Use when Codex is asked to run the recover workflow for an active track after a conflicted base merge.
sandbox: workspace-write
---

# Track Recover

The canonical recover workflow is `.harness/workflows/track/recover.md`. This skill is the
Codex adapter for that provider-neutral workflow and must not duplicate its state machine.

When invoked, resolve the active track from the current `track/<id>` branch, read the workflow
and track context, and delegate recovery through the guarded CLI surface. Do not invoke git or
filesystem recovery operations directly; recovery may modify the worktree and stage files only
through the guarded surfaces described by the workflow. Adapter rule: do not create commits, merge branches, or push.
Delegate guarded commit creation to `$track-commit` after review.

Provider invocation constraints: use `$track-review` for the review stage and
`$track-commit <recovery-commit-message>` for the guarded commit stage.

On success report `RECOVER_STATUS: completed`; on a fail-closed gate or missing context report
`RECOVER_STATUS: blocked` with the reason.
