---
description: Run GitHub PR-based review cycle via Codex Cloud @codex review.
---

> Operational SSoT: `.harness/workflows/track/pr-review.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:pr-review`. `$ARGUMENTS` is unused (reserved for future configuration).

## Claude Code invocation constraints

- **Finding-fix dispatch**: the push / PR / review-cycle commands run from the orchestrator
  host, but actionable findings are not fixed inline. Prepare a focused briefing for each
  finding with the review comment, affected path and line, relevant track context, and requested
  correction. For implementation changes, dispatch `implementer` with:
  ```
  bin/sotp capability exec implementer --host claude --briefing-file <path>
  ```
  If the dispatcher returns `CAPABILITY_EXEC_OUTCOME: delegate-in-host`, invoke the matching
  Claude Agent tool with the returned briefing path and discipline body. For review-scope fixes,
  dispatch the `review-fix-lead` lane through the provider-neutral wrapper:
  ```
  cargo make track-local-review-fix -- --scope <scope> \
    --briefing-file <path> \
    --round-type fast|final
  ```
  If the wrapper exits `64` with `SUBAGENT_DISPATCH_REQUIRED`, parse its payload and spawn the
  `review-fix-lead` Claude subagent as instructed. Do not branch on the configured
  `review-fix-lead` provider; the wrapper resolves it.

- **Convergence and commit**: after the delegated capability reports completion, run
  `/track:review` to convergence and `/track:commit` before re-running this command. The
  orchestrator edits directly only as recovery after a failed delegation, and still applies the
  same local-convergence and commit sequence.

- **PR command wrappers**: use these from the orchestrator host in sequence:

  - `bin/sotp pr push` — push the track branch
  - `bin/sotp pr ensure-pr` — create or reuse a PR
  - `bin/sotp pr review-cycle` — trigger + poll + parse the Codex Cloud review cycle

Prerequisites: Codex Cloud GitHub App must be installed; `gh` CLI must be authenticated.
`sotp pr review-cycle` resolves `capabilities.pr-reviewer` internally from
`.harness/config/agent-profiles.json` and fails if the provider is not `codex`; surface that
error and direct the user to use `/track:review` instead.

## Report format

After execution, summarize:

1. PR number and URL.
2. Terminal state: machine PASS (explicit zero-findings signal), or user-approved Accepted Deviations with the user's approval citation.
3. Per-round trace: review state (APPROVED / CHANGES_REQUESTED / COMMENTED), surfaced comments (review body + inline with `path:line`), actionability assessment, and fix commit hashes.
4. Recommended next command (`/track:merge` once 👍 is reached and the user is ready).
