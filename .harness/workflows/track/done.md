# Done Workflow SSoT

> Provider-agnostic workflow SSoT for the `done` track workflow. Provider-specific adapters
> (e.g. `.claude/commands/track/done.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Return the working tree to the configured base branch after a track's PR has been merged,
attempt a ff-only sync, and report a short completion summary. This workflow performs no
gate checks — merge-time gates have already been run by `/track:merge` (or the equivalent PR
merge path). The sync is attempted, not guaranteed: all fail-closed sync modes (missing
upstream / non-fast-forward / worktree unresolved) are downgraded to a warning and do not
fail the workflow, preserving the pre-track CLI behavior contract (CN-05 bit-equivalent).

## Inputs

None. The workflow resolves the base branch from the configured `BranchStrategyPort` (via
`cargo make track-switch-base`).

## Sequence

**Step 1: Switch to the configured base branch and attempt sync**

```
cargo make track-switch-base
```

Checks out the configured base branch and then runs a ff-only current-branch sync (the same pull
operation exposed by `cargo make sync` / `bin/sotp git sync` / `git pull --ff-only`) against
origin. The switch+sync composition belongs to `cargo make track-switch-base` /
`bin/sotp track switch-base`: the CLI's track-git logic switches branches, then syncs the current
branch; all fail-closed sync modes are downgraded to the non-fatal "[WARN] Pull failed" message
so the branch switch itself always succeeds when possible.

The wrapper delegates to `bin/sotp track switch-base`, which resolves `base_branch` from the
active track's `metadata.json#branch_strategy_snapshot`. The current branch must be
`track/<id>` for the active track to resolve; if the caller is already on the base branch or
on any other non-`track/<id>` branch, the command fails-closed. In that case, switch back to
the track branch manually before invoking `/track:done`.

To sync only (without a branch switch — e.g. on the track branch to catch up with the
remote), use `cargo make sync` directly.

**Step 2: Completion summary**

After `cargo make track-switch-base`:

1. Report the command's branch/sync result verbatim without paraphrase:
   - On success, the wrapper prints `[OK] On <base>, up to date.` — surface that line as-is.
   - On any sync failure (missing upstream / non-fast-forward / worktree unresolved), the wrapper prints `[WARN] Pull failed ...` — surface that line as-is and explicitly note the sync was **attempted** and did not confirm origin state. Do not claim the branch is up to date with origin in this case.
2. Read `track/registry.md` and surface:
   - The latest completed track name and date.
   - The count of active tracks remaining.
3. Recommend the next action:
   - If active tracks remain: `/track:implement` or `/track:full-cycle <task>`.
   - If no active tracks: `/track:plan <feature>` to start new work.

## Gates

None. This workflow assumes a successful merge upstream and does not re-verify PR state.

## Outputs

- Working tree checked out on the configured base branch (sync attempted; the wrapper's `[OK]` line or `[WARN]` line is surfaced verbatim so callers can tell whether the branch is confirmed up to date with origin or only attempted).
- A short completion summary printed to the caller.
- No commits, no PR interaction, no metadata edits.
