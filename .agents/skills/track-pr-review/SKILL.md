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

- Follow Step 3 of the workflow SSoT for the finding-fix procedure (briefing contents,
  capability routing, local convergence, commit, re-run, recovery). Codex-specific dispatch
  forms only:
  - implementation corrections: `bin/sotp capability exec implementer --briefing-file <path>`
    with `--host` omitted, so the dispatcher runs the provider subprocess itself
    (dispatcher-owned sandbox / model / effort flags); never pass `--host codex` and never
    hand-assemble a `codex exec` command.
  - review-scope fixes: `cargo make track-local-review-fix -- --scope <scope>
    --briefing-file <path> --round-type fast|final`.
  - local convergence and commit use `$track-review` and `$track-commit`.

### (5) Gate waiting

- `bin/sotp pr review-cycle` owns the trigger → poll → parse sequence internally: run it as one
  blocking call and read its result once. Do not add a manual polling loop or periodic PR-status
  probes around it; if the host backgrounds the call, read the result once after the single
  completion notification, then apply the workflow SSoT's stale-review handling.

### (6) Reporting format

- On successful completion (only when the PR review reaches explicit zero findings or the user
  approves an Accepted Deviations exception per `.harness/workflows/track/pr-review.md`),
  print: `PR_REVIEW_STATUS: completed — PR <url> zero findings`
- On failure or block, print: `PR_REVIEW_STATUS: blocked — <reason>`
