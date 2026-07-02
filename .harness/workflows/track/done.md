# Done Workflow SSoT

> Provider-agnostic workflow SSoT for the `done` track workflow. Provider-specific adapters
> (e.g. `.claude/commands/track/done.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Return the working tree to the configured base branch after a track's PR has been merged, and
report a short completion summary. This workflow performs no gate checks — merge-time gates
have already been run by `/track:merge` (or the equivalent PR merge path).

## Inputs

None. The workflow resolves the base branch from the configured `BranchStrategyPort` (via
`cargo make track-switch-base`).

## Sequence

**Step 1: Switch to the configured base branch**

```
cargo make track-switch-base
```

Checks out the configured base branch and pulls the latest changes from origin. The wrapper
delegates to `bin/sotp track switch-base`, which resolves `base_branch` from the active track's
`metadata.json#branch_strategy_snapshot` when available and falls back to
`.harness/config/branch-strategy.json` (via `JsonConfigBranchStrategyAdapter`) otherwise.

**Step 2: Completion summary**

After the branch switch:

1. Read `track/registry.md` and surface:
   - The latest completed track name and date.
   - The count of active tracks remaining.
2. Recommend the next action:
   - If active tracks remain: `/track:implement` or `/track:full-cycle <task>`.
   - If no active tracks: `/track:plan <feature>` to start new work.

## Gates

None. This workflow assumes a successful merge upstream and does not re-verify PR state.

## Outputs

- Working tree checked out on the configured base branch, updated from origin.
- A short completion summary printed to the caller.
- No commits, no PR interaction, no metadata edits.
