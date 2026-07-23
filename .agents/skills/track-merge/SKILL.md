---
name: track-merge
description: Use when Codex is asked to wait for a track PR's CI checks and merge it through the guarded workflow.
---

# Track-Merge (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/merge.md` — the provider-agnostic workflow contract for this skill.
Do not duplicate step sequence, gate conditions, state transitions, or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-merge` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.
- An optional PR number and optional merge method may be supplied as `$track-merge <pr-number> [merge-method]`.
- When no PR number is supplied, resolve the PR for the current branch with `gh pr view --json number -q .number` before following the workflow SSoT.

### (2) Tool constraints

- Requires `--sandbox workspace-write`: the workflow may merge a PR and may invoke guarded recovery workflows.
- Use `bin/sotp pr wait-and-merge` for the merge operation.
- Do not invoke `gh pr merge`, `git merge`, `git push`, `git add`, or `git commit` directly.
- Pass `--method` only when the user explicitly supplied a merge method.

### (3) Reporting format

- On successful completion, print: `MERGE_STATUS: completed — PR <url> merged with <method> at <commit>`.
- On failure or block, print: `MERGE_STATUS: blocked — <reason>`.
