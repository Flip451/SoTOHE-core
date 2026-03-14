# GitHub PR-Based Review Cycle Design

## Recommendation

Add `/track:pr-review` as a branch-aware, PR-aware extension of the existing local `/track:review` loop.

Principles:

- Keep `/track:review` unchanged as the fast local reviewer loop.
- Add a separate PR command so GitHub-side review is opt-in and explicit.
- Route all git and GitHub mutations through `cargo make` wrappers.
- Reuse the existing reviewer provider resolution from `.claude/agent-profiles.json`.
- Treat Codex JSON review output as the canonical machine-readable finding source.
- Post one GitHub review per run, with multiple line comments when findings exist.
- If there are zero findings, submit an approval review on the PR.

## Desired Operator Flow

1. User works on `track/<track-id>`.
2. `/track:review` runs locally until the implementation is stable enough to publish.
3. `/track:pr-review`:
   - resolves the current track and branch
   - pushes the branch
   - creates or reuses a PR to `main`
   - runs reviewer capability
   - translates findings to GitHub review comments
   - submits either:
     - `COMMENT` review with inline comments when findings exist
     - `APPROVE` review when zero findings are returned
4. User fixes findings locally and reruns `/track:pr-review`.
5. Final merge remains a user action.

This preserves the current review then fix loop, but adds a second review surface on GitHub for remote collaboration and PR history.

## Key Decisions

### 1. Wrapper task strategy

Add narrow wrappers instead of allowing direct `gh` shell usage:

- `track-pr-push`
- `track-pr-ensure`
- `track-pr-review`

Optional split wrappers if implementation clarity is better than one monolith:

- `track-pr-find`
- `track-pr-create`
- `track-pr-update-body`
- `track-pr-submit-review`

Recommendation: keep the public surface small with three primary wrappers and let one Python orchestrator script handle the detailed `gh` invocations.

### 2. Review source of truth

Use:

```text
cargo make track-local-review -- --model {model} --briefing-file tmp/codex-briefing.md
```

for the Codex reviewer path.

Rationale:

- it routes through the repo-owned wrapper instead of a raw reviewer subcommand
- it enforces the read-only/timeout/output-schema contract in one place
- it still returns structured findings for deterministic parsing

For non-Codex reviewer providers:

- keep local `/track:review` behavior as-is
- v1 of `/track:pr-review` should support only reviewer providers that emit structured findings
- if the active reviewer is `claude`, either:
  - fail closed with a clear message that PR review posting currently requires a structured reviewer provider, or
  - add an explicit structured markdown contract and a parser later

Recommendation: fail closed for non-structured providers in v1.

### 3. PR lifecycle

Create once, then update on later runs:

1. Search for an open PR with:
   - head branch `track/<track-id>`
   - base `main`
2. If found, reuse it.
3. If not found, create it.
4. On every run, push the branch before review submission.
5. Optionally refresh title/body from track metadata each run for consistency.

Do not create a new PR per review round.

### 4. Review submission shape

Submit findings as a single review with many inline comments, not individual standalone comments.

Reasoning:

- one run maps to one review event in GitHub history
- easier to distinguish rounds
- simpler summary messaging
- `APPROVE` on zero findings is a natural complement

Recommended states:

- findings present: `gh pr review --comment`
- zero findings: `gh pr review --approve`

If `gh pr review` line-comment ergonomics prove insufficient, use `gh api` for a single pending review plus comment batch submission, still behind the same wrapper.

### 5. Zero findings behavior

Zero findings means:

- no severity at or above the posting threshold
- no unmappable finding remains after path/line resolution

When zero findings:

- submit approval review
- include a short body such as `Automated reviewer found no actionable issues in this revision.`

When findings exist:

- submit `COMMENT` review
- do not approve

### 6. Finding threshold policy

Post only actionable findings:

- post: `CRITICAL`, `HIGH`, `MEDIUM`, `LOW`
- skip: `INFO`

If the reviewer outputs different severity names, normalize them in the bridge script.

### 7. Line mapping policy

Only post line comments when the finding can be resolved to a changed line on the PR.

Resolution order:

1. explicit file + line from reviewer JSON
2. explicit file + approximate line mapped to nearest changed hunk line
3. explicit file but no line:
   - post as general review body bullet, not inline
4. no file:
   - post as general review body bullet, not inline

Do not force inline comments when location confidence is poor.

## Task Decomposition

### T0. Review JSON contract and sample corpus

Define the structured finding contract expected from the reviewer bridge:

- capture sample Codex JSON outputs in fixtures
- define normalized internal finding schema
- document required fields, optional fields, and zero-findings detection

Deliverables:

- `scripts/pr_review.py` normalized types
- tests for parser against fixture variants
- plan note in `/track:pr-review` command prompt

### T1. Makefile wrappers for PR operations

Add wrapper tasks to `Makefile.toml`.

Required wrappers:

- `track-pr-push`
- `track-pr-ensure`
- `track-pr-review`

Suggested shapes:

- `track-pr-push '<track-id>'`
- `track-pr-ensure '<track-id>'`
- `track-pr-review '<track-id>'`

Implementation rule:

- wrappers invoke `scripts/pr_review.py <subcommand> ...`
- no direct `gh` commands from Claude prompts

### T2. PR orchestration script

Add `scripts/pr_review.py` as the orchestration layer for:

- branch validation
- push
- PR lookup/create/update
- reviewer invocation
- review parsing
- GitHub review submission

Subcommands:

- `push`
- `ensure-pr`
- `submit-review`
- optional `run` for end-to-end orchestration

Recommendation:

- expose all subcommands for testability
- have `track-pr-review` call `run`

### T3. GitHub PR discovery and creation

Implement deterministic PR lifecycle handling:

- derive head branch from track metadata or current branch
- require `track/<track-id>` branch
- use `gh pr list --head ... --base main --state open --json ...`
- create PR with `gh pr create` if missing
- emit PR number/url/json for downstream steps

Recommended PR content:

- title from track title
- body summarizing:
  - spec intent
  - plan status
  - verification expectations

### T4. Review finding normalization and diff mapping

Build the bridge from reviewer findings to GitHub review comments.

Responsibilities:

- parse reviewer JSON into normalized findings
- normalize severity names
- sanitize bodies
- map repo-relative file paths
- resolve line numbers against PR diff / changed files
- separate:
  - inline comments
  - general review summary bullets

Need:

- `gh pr diff --patch` or `gh api` file metadata to determine changed lines
- mapping logic for added/modified line anchors

### T5. GitHub review submission

Submit exactly one review per run.

Cases:

- actionable findings exist:
  - submit `COMMENT` review
  - include inline comments
  - include summary body with counts and any non-inline findings
- zero findings:
  - submit `APPROVE` review
  - include short approval body

Deduplication rule:

- do not try to delete or edit earlier bot reviews in v1
- each rerun creates a new review round

This keeps the implementation simple and preserves review history.

### T6. `/track:pr-review` command prompt

Add `.claude/commands/track/pr-review.md`.

Responsibilities:

- resolve current track
- read `spec.md`, `plan.md`, `metadata.json`, `verification.md`
- resolve reviewer provider from `.claude/agent-profiles.json`
- require structured reviewer support
- run:
  - `cargo make track-pr-push '<track-id>'`
  - `cargo make track-pr-ensure '<track-id>'`
  - `cargo make track-pr-review '<track-id>'`
- summarize:
  - PR URL
  - review action taken (`COMMENT` or `APPROVE`)
  - finding counts
  - next recommended command

### T7. Permissions, hooks, and guardrails

Update orchestration guardrails for the new wrappers.

Changes:

- `.claude/settings.json`
  - allow:
    - `Bash(cargo make track-pr-push:*)`
    - `Bash(cargo make track-pr-ensure:*)`
    - `Bash(cargo make track-pr-review:*)`
- `.claude/permission-extensions.json`
  - mirror if this repo treats wrapper additions as extension-tracked
- `.claude/hooks/block-direct-git-ops.py`
  - no broad `git push` allowance
  - keep raw `git push` blocked
  - optionally update message text to mention `/track:pr-review`
- `scripts/verify_orchestra_guardrails.py`
  - add new expected allow entries
  - keep direct repo scripts forbidden
  - do not allow direct `gh:*`

Recommendation:

- keep `gh` completely unapproved at shell permission level
- only `cargo make` wrappers may reach it internally

### T8. Tests

Add focused tests for:

- wrapper presence in `Makefile.toml`
- guardrail allowlist sync
- parser normalization
- diff line mapping
- PR lookup/create logic via mocked `gh`
- zero findings to `APPROVE`
- findings present to `COMMENT`
- fallback of unmappable findings to review body
- path sanitization preventing internal path leakage

Likely test files:

- `scripts/test_pr_review.py`
- updates to `scripts/test_make_wrappers.py`
- updates to `scripts/test_verify_scripts.py`
- updates to `.claude/hooks/test_policy_hooks.py` if user-facing messages change

### T9. Documentation and workflow updates

Update:

- `CLAUDE.md`
- `DEVELOPER_AI_WORKFLOW.md`
- `track/workflow.md`

Document:

- when to use `/track:review` vs `/track:pr-review`
- that merge still remains manual
- that approval is generated only on zero findings
- that GitHub operations require authenticated `gh`

## Canonical Blocks

### A. Wrapper task outline

```toml
[tasks.track-pr-push]
description = "[wrapper] Push current track branch to origin"
script_runner = "@shell"
script = ['"${PYTHON_BIN:-python3}" scripts/pr_review.py push "$CARGO_MAKE_TASK_ARGS"']

[tasks.track-pr-ensure]
description = "[wrapper] Create or reuse a PR from track/<id> to main"
script_runner = "@shell"
script = ['"${PYTHON_BIN:-python3}" scripts/pr_review.py ensure-pr "$CARGO_MAKE_TASK_ARGS"']

[tasks.track-pr-review]
description = "[wrapper] Run reviewer and submit GitHub PR review"
script_runner = "@shell"
script = ['"${PYTHON_BIN:-python3}" scripts/pr_review.py run "$CARGO_MAKE_TASK_ARGS"']
```

### B. Normalized finding types

```python
from dataclasses import dataclass
from typing import Literal

Severity = Literal["critical", "high", "medium", "low", "info"]
ReviewAction = Literal["comment", "approve"]

@dataclass(frozen=True)
class ReviewFinding:
    severity: Severity
    summary: str
    body: str
    path: str | None
    line: int | None
    end_line: int | None
    rule_id: str | None

@dataclass(frozen=True)
class InlineComment:
    path: str
    line: int
    body: str

@dataclass(frozen=True)
class ReviewSubmission:
    action: ReviewAction
    body: str
    comments: list[InlineComment]
```

### C. Script module structure

```text
scripts/pr_review.py
  - main(argv)
  - cmd_push(...)
  - cmd_ensure_pr(...)
  - cmd_run(...)
  - resolve_track_context(...)
  - resolve_reviewer_provider(...)
  - run_codex_review_json(...)
  - parse_codex_review_output(...)
  - normalize_findings(...)
  - load_pr_changed_lines(...)
  - map_findings_to_comments(...)
  - build_review_submission(...)
  - submit_review_with_gh(...)
```

### D. GitHub command outline

```text
gh pr list --head track/<track-id> --base main --state open --json number,url,title
gh pr create --base main --head track/<track-id> --title <title> --body-file <tmpfile>
gh pr view <number> --json number,url,headRefName,baseRefName
gh pr diff <number> --patch
gh pr review <number> --approve --body <text>
gh api repos/{owner}/{repo}/pulls/{number}/reviews ...   # fallback for inline batch comments
```

### E. Command prompt outline

```md
## /track:pr-review

1. Resolve current track from active branch.
2. Read spec.md, plan.md, metadata.json, verification.md.
3. Resolve reviewer provider from .claude/agent-profiles.json.
4. If provider cannot emit structured findings, stop with guidance.
5. Run cargo make track-pr-push '<track-id>'.
6. Run cargo make track-pr-ensure '<track-id>'.
7. Run cargo make track-pr-review '<track-id>'.
8. Report PR URL, review action, finding counts, and next step.
```

## Mapping Rules

### Reviewer finding to GitHub comment body

Format:

```text
[MEDIUM] Potential lock ownership leak on error path.
Why it matters: ...
Suggested fix: ...
```

Rules:

- no internal absolute paths
- no secrets or environment values
- concise, actionable wording
- cap comment size to avoid oversized review payloads

### Repo path sanitation

Accept only repo-relative paths that resolve inside the repository root.

Reject or downgrade to summary-only when:

- path is absolute
- path escapes repo root
- file is not in the PR diff

### Line mapping fallback

If a finding line is outside the changed hunk:

1. try nearest changed line in same hunk
2. else move finding to general review body

Do not post misleading inline comments.

## Permission and Guardrail Changes

### `.claude/settings.json`

Add:

- `Bash(cargo make track-pr-push:*)`
- `Bash(cargo make track-pr-ensure:*)`
- `Bash(cargo make track-pr-review:*)`

Do not add:

- `Bash(gh:*)`
- `Bash(git push:*)`

### `scripts/verify_orchestra_guardrails.py`

Extend:

- `EXPECTED_CARGO_MAKE_ALLOW`
- any reserved task-name sets derived from it

Keep these invariants:

- repo scripts are still routed through wrappers
- direct `gh` shell permissions remain forbidden
- raw `git push` stays blocked

## Open Questions and v1 Decisions

1. Should findings be posted as a single review or individual comments?
   Decision: single review per run with multiple inline comments.

2. How to detect zero findings and mark PR as approved?
   Decision: zero normalized actionable findings yields `APPROVE`.

3. How to handle non-inline findings?
   Decision: append them to the main review body under `General findings`.

4. Should previous bot reviews be updated or replaced?
   Decision: no, create a fresh review each run in v1.

5. Should `/track:pr-review` run full CI itself?
   Decision: no new mandatory CI step in v1. Keep CI responsibility with existing `/track:review` and `/track:ci` flow. PR review posting is not a substitute for `cargo make ci`.

6. Should the script support non-Codex reviewers in v1?
   Decision: no, only structured-review providers.

## Implementation Order

Recommended sequence:

1. T0 review contract
2. T1 wrappers
3. T2 orchestration skeleton
4. T3 PR lifecycle
5. T4 finding normalization and diff mapping
6. T5 review submission
7. T6 command prompt
8. T7 permissions and guardrails
9. T8 tests
10. T9 docs

## Acceptance Criteria

- `/track:review` still works unchanged.
- `/track:pr-review` pushes `track/<id>` and creates or reuses a PR to `main`.
- Codex JSON findings are converted into GitHub review comments without leaking internal paths.
- Zero findings produce an approval review.
- Raw `git push` and raw `gh` usage remain outside allowed shell permissions.
- `cargo make verify-orchestra` and wrapper tests pass after the changes.
