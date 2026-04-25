---
description: Wait for PR CI checks to pass, then merge.
---

Canonical command for CI-gated PR merge.

Polls PR checks until all pass, then merges automatically.
If any check fails, stops and reports the failure.

Arguments:
- Use `$ARGUMENTS` as PR number (required). Optionally append merge method: `3 squash`, `3 rebase`. Default is merge commit.

**IMPORTANT — do NOT auto-select the merge method**: when `$ARGUMENTS` does
not include a method, that means the user has explicitly chosen the default
(merge commit). Pass `--method merge` (or omit `--method`, whichever the
wrapper interprets as the default) and stop. Do NOT substitute `squash` or
`rebase` based on prior knowledge of how other projects merge.
Override the default only when the user typed `squash` or `rebase`
verbatim in `$ARGUMENTS`.

## Step 0: Resolve PR

- If `$ARGUMENTS` is empty, detect the PR for the current branch via `gh pr view --json number -q .number`.
- Extract the PR number and optional merge method from `$ARGUMENTS`.

## Step 1: Wait and merge

Run the merge wrapper:

```bash
cargo make track-pr-merge <pr_number> --method <method>
```

This executes `bin/sotp pr wait-and-merge` which:
1. **Task completion guard**: blocks merge if any tasks in `metadata.json` are unresolved (not done/skipped). This is the only command that enforces task completion — push and PR review are allowed with unresolved tasks.
2. Polls `gh pr checks` every 15 seconds (10 minute timeout)
3. On all checks passed: merges via `gh pr merge --<method>`
4. On any check failed: stops and reports failures
5. On timeout: stops and reports pending checks

## Step 2: Post-merge

After successful merge:
1. Report the merge result (PR URL, method, commit)
2. Recommend next action (`git pull` on main, or `/track:plan <feature>` for next work)

## Behavior

After execution, summarize:
1. PR number and URL
2. Check status (all passed / failed / timeout)
3. Merge result (success with method, or failure reason)
4. Recommended next command
