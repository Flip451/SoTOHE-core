---
name: track-pr-review
description: Use when Codex is asked to run the GitHub PR-based review cycle — push the current branch, create or reuse a PR, and trigger a PR-level review.
---

# Track-Pr-Review (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/pr-review.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-pr-review` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow pushes the branch to origin and
  interacts with the GitHub API via `gh` / `bin/sotp pr` wrappers.
- Branch push uses `cargo make track-pr-push`; PR creation uses `bin/sotp pr ensure-pr`.
- Do not run `git push` directly.

### (3) Sub-workflow and capability invocation

- PR creation and push are performed via `bin/sotp pr` and `cargo make track-pr-push`.
- PR-level review is triggered via `cargo make track-pr-review` (which dispatches `@codex review`).
- Codex-specific prerequisite: the **Codex Cloud GitHub App** must be installed on the
  repository so `@codex review` is acted upon.

### (4) Reporting format

- On successful completion (only when the PR review reaches explicit zero findings or the user
  approves an Accepted Deviations exception per `.harness/workflows/track/pr-review.md`),
  print: `PR_REVIEW_STATUS: completed — PR <url> zero findings`
- On failure or block, print: `PR_REVIEW_STATUS: blocked — <reason>`
