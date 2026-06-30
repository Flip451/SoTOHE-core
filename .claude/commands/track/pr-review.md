---
description: Run GitHub PR-based review cycle via Codex Cloud @codex review.
---

> Operational SSoT: `.harness/workflows/track/pr-review.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:pr-review`. `$ARGUMENTS` is unused (reserved for future configuration).

## Claude Code invocation constraints

This command runs directly — no subagents. Key wrappers used in sequence:

- `cargo make track-pr-push` — push the track branch
- `bin/sotp pr ensure-pr` — create or reuse a PR
- `cargo make track-pr-review` — trigger + poll + parse the Codex Cloud review cycle

Prerequisites: Codex Cloud GitHub App must be installed; `gh` CLI must be authenticated. Resolve `capabilities.pr-reviewer` from `.harness/config/agent-profiles.json`; if the provider is not `codex`, fail and direct the user to use `/track:review` instead.

## Report format

After execution, summarize:

1. PR number and URL.
2. Terminal state: machine PASS (explicit zero-findings signal), or user-approved Accepted Deviations with the user's approval citation.
3. Per-round trace: review state (APPROVED / CHANGES_REQUESTED / COMMENTED), surfaced comments (review body + inline with `path:line`), actionability assessment, and fix commit hashes.
4. Recommended next command (`/track:merge` once 👍 is reached and the user is ready).
