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
- Branch push uses `bin/sotp pr push`; PR creation uses `bin/sotp pr ensure-pr`.
- Do not run `git push` directly.

### (3) Sub-workflow and capability invocation

- PR creation and push are performed via `bin/sotp pr push` and `bin/sotp pr ensure-pr`.
- PR-level review is triggered via `bin/sotp pr review-cycle` (which dispatches `@codex review`).
- Codex-specific prerequisite: the **Codex Cloud GitHub App** must be installed on the
  repository so `@codex review` is acted upon.

### (4) Finding fixes are delegated

- Actionable PR findings are not fixed inline in the root Codex session. Per the workflow SSoT's
  Step 3, prepare a focused briefing per finding and dispatch
  `bin/sotp capability exec implementer --briefing-file <path>` (implementation changes) or
  `cargo make track-local-review-fix -- --scope <scope> --briefing-file <path>
  --round-type fast|final` (review-scope fixes). Omit `--host` on the implementer dispatch from
  a Codex root: the dispatcher then runs the provider subprocess itself with its own sandbox,
  model, and effort flags and the shared no-direct-git discipline, so the outcome is
  `CAPABILITY_EXEC_OUTCOME: executed` and the implementer skill is never loaded into the root
  session. Do not pass `--host codex` here (it would return `delegate-in-host`, for which no
  separate in-host implementer agent exists) and never hand-assemble a `codex exec` command.
  Only an `executed` outcome with the subprocess's completion report counts as the fix being
  applied. Then converge locally
  with `$track-review`, commit with `$track-commit`, and re-run this skill. The root session edits files directly only as
  recovery after a failed delegation, and still runs the local convergence and commit before
  re-running.

### (5) Reporting format

- On successful completion (only when the PR review reaches explicit zero findings or the user
  approves an Accepted Deviations exception per `.harness/workflows/track/pr-review.md`),
  print: `PR_REVIEW_STATUS: completed — PR <url> zero findings`
- On failure or block, print: `PR_REVIEW_STATUS: blocked — <reason>`
