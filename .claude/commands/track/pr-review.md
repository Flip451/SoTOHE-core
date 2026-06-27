---
description: Run GitHub PR-based review cycle via Codex Cloud @codex review.
---

Canonical command for GitHub PR-based review using Codex Cloud.

This command pushes the track branch, creates/reuses a PR, triggers `@codex review`,
polls for the review result, and reports findings.

**Prerequisites:**
- Codex Cloud GitHub App must be installed on the repository.
- `gh` CLI must be authenticated.

Arguments:
- Use `$ARGUMENTS` as optional options (currently unused, reserved for future configuration).

## Step 0: Resolve context

- Resolve the current track: the current git branch must match `track/<id>`.
- Read the track's `metadata.json` to confirm track status.
- Read `.harness/config/agent-profiles.json` to verify the `pr-reviewer` provider supports structured output.
- If the `pr-reviewer` provider is not in the structured provider set (currently: `codex`), fail with a clear error message directing the user to use `/track:review` instead.
- The local-review provider (`capabilities.reviewer.provider`) does not affect this command: `/track:pr-review` resolves `capabilities.pr-reviewer` (default `codex`), so setting `reviewer.provider: claude` for local review leaves PR-based review on Codex Cloud unchanged.

## Step 1: Push and ensure PR

Run the following wrappers in sequence:

```bash
cargo make track-pr-push
```

> **Note**: `track-pr-push` does NOT enforce task completion. Push is allowed with unresolved tasks.
> Task completion is only enforced at merge time (`bin/sotp pr wait-and-merge` / `/track:merge`).

Then:

```bash
bin/sotp pr ensure-pr
```

The ensure step will either create a new PR or reuse an existing one for this track branch.

## Step 2: Trigger review

Run the full cycle which handles trigger, poll, and parse:

```bash
cargo make track-pr-review
```

This executes `sotp pr review-cycle` which:
1. Pushes the track branch
2. Creates/reuses the PR
3. Posts `@codex review` comment on the PR
4. Polls GitHub API for the Codex Cloud review (default: 15s interval, 10min timeout)
5. Collects the latest review round (sanitized `review.body` + inline comments) — without interpreting or grading them
6. Surfaces those comments for you to judge, or reports a machine PASS when the bot signalled zero findings

## Step 3: Handle results — MUST continue until explicit 👍

After `cargo make track-pr-review` completes:

- If the bot **signalled zero findings** (👍 reaction or a "no major issues" comment): this is a machine PASS. Report success to the user and recommend `/track:merge` once they are ready.
- Otherwise the command surfaces the latest review round's comments verbatim (sanitized `review.body` + inline comments). **You MUST**:
  1. Read each comment and assess actionability — the command does not grade them for you.
  2. Fix every actionable finding locally (apply code edits, run local reviews to zero_findings, commit).
  3. Re-run `/track:pr-review` to push the fix, trigger a new Codex Cloud review round, and verify the response.
  4. Repeat steps 1–3 until the bot signals **explicit 👍 (zero-findings reaction or a "no major issues" comment)**.

**Do NOT stop the loop on intermediate states**, including:

- A round with "all findings are minor / non-blocking" wording (only an explicit 👍 / zero-findings comment counts as the terminal state).
- A round where you intend to record `## Accepted Deviations` in the PR body. Recording an Accepted Deviation requires **explicit user approval** before the loop is allowed to terminate without 👍 — surface the proposed acceptance to the user and wait for their confirmation. Do NOT silently transition from "I judged this acceptable" to "stopping the loop"; the loop's terminal condition is 👍 by default, and the only documented exception is user-approved Accepted Deviations.
- A round that returned the same review ID as the previous round (stale review — see Async handling below for retry guidance).

Report to the user once the loop reaches its terminal state:
1. PR URL
2. **Terminal state**: explicit 👍 machine PASS, or user-approved Accepted Deviations (with the user's approval citation).
3. **Per-round trace**: for each round, the review state (APPROVED / CHANGES_REQUESTED / COMMENTED), the surfaced comments (review body + inline comments with `path:line`), and the fix commit hashes that closed each actionable finding.
4. The recommended next action (e.g., `/track:merge` once 👍 is reached and the user is ready).

## Async handling

The review is asynchronous. After posting `@codex review`, the script polls GitHub API.
If the poll times out:
- **No bot activity**: Suggests the Codex Cloud GitHub App is not installed.
- **Bot active but no review**: The review is still in progress. Try again later.

**Same-commit re-review and reaction check**: Codex Cloud can re-review the same HEAD
commit when `@codex review` is re-posted. Whether a fresh review is produced depends on
Codex Cloud adding a **reaction** (e.g., eyes emoji) to the `@codex review` comment:

- **Reaction present**: Codex Cloud accepted the request. A new review will be produced.
- **No reaction after ~30s**: Codex Cloud silently ignored the request. The poller will
  time out and fall back to the previous stale review via commit-based recovery.

When the poller returns a stale review (same review ID as the previous round), re-trigger
`/track:pr-review` once more. If the reaction still does not appear after 2 retries, push
a trivial commit (e.g., whitespace or doc comment) to force a new HEAD.

**No manual polling**: `cargo make track-pr-review` (which delegates to `sotp pr review-cycle`)
handles the full trigger → poll → parse → report cycle internally (15s interval, 10min timeout).
Do NOT substitute manual `sleep` + `gh api` loops. The internal poller uses `trigger_timestamp`
filtering to match reviews to the correct trigger round, which manual polling cannot replicate.

## Accepted findings

When a reviewer finding is valid but intentionally deferred (e.g., edge case not applicable
to the current workflow), record it in the **PR body** under an `## Accepted Deviations` section
using numbered list format (reference: PR #72):

```markdown
## Accepted Deviations (IMPORTANT: do not re-report these as findings)

### Category Name
1. **Short title** — Why this is accepted
2. **Short title** — Why this is accepted

### Other
N. **General findings** ("specific example") — NOT CODE FINDINGS
```

This makes the acceptance visible to the reviewer on subsequent rounds and serves as a
documented decision. Do NOT use table format — the numbered list format is more reliably
parsed by automated reviewers.

## Behavior

After execution, summarize:
1. PR number and URL
2. Terminal state: machine PASS (explicit zero-findings signal), or user-approved Accepted Deviations with the user's approval citation
3. Per-round trace: review state (APPROVED / CHANGES_REQUESTED / COMMENTED), surfaced review body + inline comments with `path:line`, actionability assessment, and fix commit hashes for each actionable finding
4. Recommended next command (`/track:merge` once 👍 is reached and the user is ready)
