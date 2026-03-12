---
description: Revert the latest track change set safely and explain the impact.
---

Canonical command for backing out track work when the latest change needs to be undone.

Arguments:
- Use `$ARGUMENTS` as optional scope notes (target commit, files, or reason).

Execution:
- Read `track/registry.md`.
- Resolve the current track: if the current git branch matches `track/<id>`, use that track. Otherwise, fall back to the latest active track by `updated_at`.
- If any track exists, identify the current track directory under `track/items/`.
- Read that track's `metadata.json`, `spec.md`, and `plan.md` when present.
- Inspect recent git history and diff to identify the safest revert target.
- Summarize the recommended revert scope before applying anything destructive.
- Prefer non-destructive guidance first (`git revert`, follow-up fix commit, or plan rollback).
- If an actual revert is requested, explain:
  1. target commit(s)
  2. affected files
  3. expected impact on `spec.md` / `plan.md` / `track/registry.md`
- Do not silently rewrite history.

Output format:
1. Revert target summary
2. Recommended command or sequence
3. Track document updates required after revert
4. Risks / follow-up checks
