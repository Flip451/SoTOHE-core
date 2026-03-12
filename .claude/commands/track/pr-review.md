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
- Read `.claude/agent-profiles.json` to verify the reviewer provider supports structured output.
- If the provider is not in the structured provider set (currently: `codex`), fail with a clear error message directing the user to use `/track:review` instead.

## Step 1: Push and ensure PR

Run the following wrappers in sequence:

```bash
cargo make track-pr-push
```

Then:

```bash
cargo make track-pr-ensure
```

The ensure step will either create a new PR or reuse an existing one for this track branch.

## Step 2: Trigger review

Run the full cycle which handles trigger, poll, and parse:

```bash
cargo make track-pr-review
```

This executes `scripts/pr_review.py run` which:
1. Pushes the track branch
2. Creates/reuses the PR
3. Posts `@codex review` comment on the PR
4. Polls GitHub API for the Codex Cloud review (default: 15s interval, 10min timeout)
5. Parses the review result (body + inline comments)
6. Reports findings summary

## Step 3: Handle results

After `cargo make track-pr-review` completes:

- If **zero actionable findings (P0/P1)**: the review passed. Proceed to `/track:commit`.
- If **findings exist**: fix the issues locally, then run `/track:pr-review` again to push, trigger a new review round, and verify fixes.

Report to the user:
1. PR URL
2. Review state (APPROVED / CHANGES_REQUESTED / COMMENTED)
3. Finding counts by severity
4. List of actionable findings with file locations
5. Recommended next action

## Async handling

The review is asynchronous. After posting `@codex review`, the script polls GitHub API.
If the poll times out:
- **No bot activity**: Suggests the Codex Cloud GitHub App is not installed.
- **Bot active but no review**: The review is still in progress. Try again later.

## Behavior

After execution, summarize:
1. PR number and URL
2. Review round result (pass/fail)
3. Finding count and severity breakdown
4. Recommended next command (`/track:pr-review` to retry, or `/track:commit <message>`)
