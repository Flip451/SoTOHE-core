# PR-Review Workflow SSoT

> Provider-agnostic workflow SSoT for the `pr-review` track workflow. Both the Claude adapter
> (`.claude/commands/track/pr-review.md`) and the Codex skill adapter
> (`.agents/skills/track-pr-review/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Run GitHub PR-based review via the `pr-reviewer` capability. The workflow pushes the track
branch, creates or reuses a PR, invokes the CLI-owned automated PR review cycle, and handles the
results loop until the reviewer signals explicit zero findings (👍) or the user approves an
Accepted Deviations exception. This workflow does NOT merge the PR; merging is a separate caller
decision.

## Inputs

- **Current branch** — must match `track/<id>`. If not, stop and report.
- **`pr-reviewer` provider** — `sotp pr review-cycle` resolves
  `capabilities.pr-reviewer` internally from `.harness/config/agent-profiles.json` and
  fail-closes unless it supports structured PR review output (currently `codex`). On that
  failure, direct the caller to use the `review` workflow instead.
- **Local-review provider** — does NOT affect this workflow. Setting `reviewer.provider: claude`
  for local review leaves PR-based review on the `pr-reviewer` provider unchanged.
- **`gh` CLI** — must be authenticated.
- **`bin/sotp pr ensure-pr`** — used to create or reuse the PR.
- **Provider-specific PR-review backend** — whatever the active `pr-reviewer` provider requires
  must be set up. Concrete prerequisites (e.g. a specific GitHub App, API key, webhook) live in
  the per-provider adapter (`.claude/commands/track/pr-review.md` /
  `.agents/skills/track-pr-review/SKILL.md`), not in this workflow SSoT.

## Sequence

**Step 0: Resolve context**

Resolve the current track from the current git branch (`track/<id>`) and read `metadata.json`
to confirm track status. `sotp pr review-cycle` performs the `pr-reviewer` structured-output
preflight internally; surface its fail-closed error if it rejects the configured provider.

**Step 1: Push and ensure PR**

Run the following commands in sequence:

```
bin/sotp pr push
```

> `bin/sotp pr push` does NOT enforce task completion. Push is allowed with unresolved tasks.
> Task completion is only enforced at merge time.

Then:

```
bin/sotp pr ensure-pr
```

This creates a new PR or reuses an existing one for this track branch.

**Step 2: Trigger review**

Run the full cycle once as a single blocking call; the CLI owns its trigger, poll, and parse
behavior:

```
bin/sotp pr review-cycle
```

At the next wake-up, read the command's terminal result once. If the host backgrounds the call,
read that result once after the single completion notification. Do not add a manual polling loop,
periodic PR-status probe, or fire-and-forget launch around the CLI; its internal poller is the
only poller in this workflow.

This executes `sotp pr review-cycle`, which:

1. Pushes the track branch
2. Creates / reuses the PR
3. Posts a review-request comment on the PR
4. Polls the GitHub API for the automated review (default: 15s interval, 10-minute timeout)
5. Collects the latest review round (review body + inline comments) without interpreting or
   grading them
6. Surfaces the comments for the caller to judge, or reports a machine PASS when the reviewer
   signalled zero findings

**Step 3: Handle results — continue until explicit zero-findings signal**

After `bin/sotp pr review-cycle` completes, apply the following loop:

- If the reviewer **signalled zero findings** (👍 reaction or a "no major issues" comment):
  machine PASS. Report success to the caller and recommend the `merge` workflow once ready.
- Otherwise, surface the latest review round's comments verbatim (review body + inline
  comments with `path:line`). For each round:
  1. Read each comment and assess actionability.
  2. For each actionable finding, prepare a focused briefing that includes the comment, affected
     path and line, relevant track context, and the requested correction. Delegate the fix to
     the capability that owns the change: `implementer` for implementation changes or
     `review-fix-lead` for review-scope fixes.
  3. After the delegated capability reports completion, run the local review workflow to
     convergence at `zero_findings`, then invoke the `commit` workflow for the fix.
  4. Re-run `pr-review` to push the fix, trigger a new review round, and verify the response.
  5. Repeat until the reviewer signals explicit zero findings.

The implementation-delegation principle used by the `implement` workflow applies to the PR
lane as well: the parent orchestrator delegates the edit through a briefing, and the delegated
capability owns the scoped change. If delegation fails, the parent may directly edit only as
recovery; it must then run local review convergence and the `commit` workflow before re-running
`pr-review`.

**Do NOT stop the loop on intermediate states**, including:

- A round with "all findings are minor / non-blocking" wording (only an explicit 👍 / zero-findings
  comment counts as the terminal state).
- A round where Accepted Deviations are being recorded in the PR body. Recording an Accepted
  Deviation requires **explicit user approval** before the loop may terminate — surface the
  proposed acceptance to the user and wait for confirmation.
- A round that returned the same review ID as the previous round (stale review — see Async
  handling).

**Accepted Deviations format** (in the PR body when applicable):

```markdown
## Accepted Deviations (IMPORTANT: do not re-report these as findings)

### Category Name
1. **Short title** — Why this is accepted

### Other
N. **General findings** ("specific example") — NOT CODE FINDINGS
```

Use a numbered list format, not table format (more reliably parsed by automated reviewers).

**Async handling**:

The review is asynchronous, but the workflow makes one blocking `review-cycle` invocation; its
internal poller waits for the matching review. If that command returns a timeout:

- No reviewer activity: suggests the automated review integration is not installed on the repo.
- Reviewer active but no review: the review is still in progress. Try again later.

**Same-commit re-review and reaction check**: the automated reviewer can re-review the same
HEAD commit when the review request is re-posted. Whether a fresh review is produced depends
on the reviewer adding a reaction to the review-request comment:

- **Reaction present**: reviewer accepted the request; a new review will be produced.
- **No reaction after ~30s**: reviewer silently ignored the request. The poller will time out
  and fall back to the previous stale review via commit-based recovery.

When the poller returns a stale review (same review ID as the previous round), re-trigger
the workflow once more. If no reaction appears after 2 retries, push a trivial commit to force
a new HEAD.

Do NOT substitute manual polling loops. The `sotp pr review-cycle` poller uses
`trigger_timestamp` filtering to match reviews to the correct trigger round, which manual
polling cannot replicate.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 0 | `pr-reviewer` provider supports structured output | OK / ERROR |
| 3 | Reviewer signals explicit zero findings (👍) | machine PASS / loop continues |
| 3 | Accepted Deviations recorded | requires explicit user approval |

## Failure / recovery

- **Wrong branch**: stop and instruct the caller to switch to `track/<id>`.
- **`pr-reviewer` provider not in structured set**: fail with clear error and direct caller to
  the `review` workflow.
- **Poll timeout (no reviewer activity)**: report that the automated review integration may
  not be installed. Do not retry automatically.
- **Poll timeout (reviewer active but no review)**: try the workflow again later.
- **Stale review (same review ID)**: re-trigger the workflow. After 2 retries, push a trivial
  commit to force a new HEAD.
- **Actionable findings remain**: prepare a briefing and delegate each fix to `implementer` or
  `review-fix-lead`, converge local review to `zero_findings`, invoke the `commit` workflow, and
  re-run `pr-review`. Repeat until explicit zero findings. If delegation fails, parent editing is
  recovery only; still converge local review and use the `commit` workflow before re-running.
  Deviations require user approval before the loop may terminate.

## Outputs

- PR number and URL
- Per-round trace: review state (APPROVED / CHANGES_REQUESTED / COMMENTED), surfaced review
  body + inline comments with `path:line`, actionability assessment, fix commit hashes for
  each actionable finding
- Terminal state: machine PASS (explicit zero-findings signal), or user-approved Accepted
  Deviations with user approval citation
- Recommended next action (`merge` workflow once the reviewer signals 👍 and the user is ready)
- **No merge is performed by this workflow**
