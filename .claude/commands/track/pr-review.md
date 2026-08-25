---
description: Run GitHub PR-based review cycle via Codex Cloud @codex review.
---

> Operational SSoT: `.harness/workflows/track/pr-review.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:pr-review`. `$ARGUMENTS` is unused (reserved for future configuration).

## Claude Code invocation constraints

- **Finding-fix dispatch**: the push / PR / review-cycle commands run from the orchestrator
  host. Follow Step 3 of the workflow SSoT for the finding-fix procedure. For implementer-owned
  non-ADR implementation or any named non-ADR review-scope artifact within its boundary, dispatch `implementer` with:
  ```
  bin/sotp capability exec implementer --host claude --briefing-file <path>
  ```
  The briefing must carry `dispatch_mode: delegated-pr-finding`; source-editing briefings also
  carry the architecture constraints required by the workflow SSoT. If the dispatcher returns
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host`, invoke the matching Claude Agent tool with the
  returned briefing path and discipline body. Writer-owned spec, catalogue, and plan artifacts
  use their owning phase workflow from the SSoT, not this focused implementer dispatch;
  `review-fix-lead` remains the normal scope-review lane until its wrapper supports a typed
  focused mode.

- **Convergence and commit**: after the delegated capability reports completion, use the
  workflow SSoT's partial-reentry / post-routing descent for any writer-owned correction, then
  run `/track:review` to convergence and `/track:commit` before re-running this command. The Claude
  root may edit directly only as recovery after a failed delegation for implementer-owned
  non-ADR findings. A
  finding requiring an edit to `knowledge/adr/*.md` must go through the review workflow SSoT's
  `ADR-scope repair lane` and is never edited by the Claude root or `review-fix-lead`; apply the
  same local-convergence and commit sequence afterward.

- **PR command wrappers**: use these from the orchestrator host in sequence:

  - `bin/sotp pr push` — push the track branch
  - `bin/sotp pr ensure-pr` — create or reuse a PR
  - `bin/sotp pr review-cycle` — trigger + poll + parse the Codex Cloud review cycle

Prerequisites: Codex Cloud GitHub App must be installed; `gh` CLI must be authenticated.
`sotp pr review-cycle` resolves `capabilities.pr-reviewer` internally from
`.harness/config/agent-profiles.json` and fails if the provider is not `codex`; surface that
error and direct the user to use `/track:review` instead.

### Gate waiting

- `bin/sotp pr review-cycle` owns the trigger → poll → parse sequence internally: run it as one
  blocking call and read its result once. Do not add a manual polling loop or periodic PR-status
  probes around it; if the host backgrounds the call, read the result once after the single
  completion notification, then apply the workflow SSoT's stale-review handling.

## Report format

After execution, summarize:

1. PR number and URL.
2. Terminal state: machine PASS (explicit zero-findings signal), or user-approved Accepted Deviations with the user's approval citation.
3. Per-round trace: review state (APPROVED / CHANGES_REQUESTED / COMMENTED), surfaced comments (review body + inline with `path:line`), actionability assessment, and fix commit hashes.
4. Recommended next command (`/track:merge` once 👍 is reached and the user is ready).
