---
description: Run review for current track implementation.
---

Canonical command for review in the track workflow.

Implements the review → fix → review cycle: do not commit until every required scope
reaches `final` round `zero_findings` (or the `NotStarted` bypass applies — see Step 6).

Arguments: none. The current branch (`track/<id>` or `plan/<id>`) determines the review target.

## Step 0: Gather context

- Extract the track id from the current git branch (`track/<id>` or `plan/<id>`).
  If the branch matches neither pattern, stop and instruct the user to switch first.
- Read the current track's `spec.md`, `plan.md`, `metadata.json`, and every convention listed
  in `## Related Conventions (Required Reading)` — check both `spec.md` and `plan.md` (legacy
  tracks may carry this section in `plan.md` instead of `spec.md`).
- For exact type signatures / module trees / Mermaid diagrams, treat `## Canonical Blocks` in
  `plan.md` and `knowledge/DESIGN.md` as the source of truth.

## Step 1: Resolve reviewer models

Read `.harness/config/agent-profiles.json` `capabilities.reviewer`:
- `capabilities.reviewer.provider` — invocation route (Codex CLI is the only supported provider; `claude` is unsupported)
- `capabilities.reviewer.fast_model` — used for `--round-type fast` (falls back to `capabilities.reviewer.model` if absent)
- `capabilities.reviewer.model` — used for `--round-type final`

## Step 2: Determine required scopes

```
cargo make track-review-results -- --track-id {track-id}
```

State legend: `[+] approved` (skip) / `[-] required (...)` (run) / `[.] not required (empty)` (skip).
Scope partitioning, hash computation, and approval state are owned by the CLI — do not
hand-classify files into groups.

### track-review-results flag reference

`cargo make track-review-results -- --help` is the canonical source. Common flags: `--track-id`, `--scope`,
`--round-type`, `--limit` (0 = state summary only; N > 0 = last N round entries).

## Step 3: Build per-scope briefings

For each scope reporting `required`, write `tmp/reviewer-runtime/briefing-{scope}.md`:

```markdown
# Review Briefing: {track-id} — {scope} layer

## Design Intent
{3-5 bullets from spec.md / plan.md describing what changed and why}

## Review Checklist
{scope-specific checklist items — keep this list short and observable}

## Known Accepted Deviations
{scope-specific notes for findings that should be dismissed}
```

Constraints:

- The CLI auto-injects scope file list and severity policy. Do NOT hand-author the
  `## Scope-specific severity policy` section: scopes with `briefing_file` configured in
  `track/review-scope.json` (e.g., `plan-artifacts` → `track/review-prompts/plan-artifacts.md`)
  receive the policy reference automatically via `sotp review codex-local`.

## Step 4: Launch review-fix-lead agents (parallel, fast round)

For each `required` scope, launch one `review-fix-lead` subagent in parallel
(`run_in_background: true`, `subagent_type: "review-fix-lead"`).

Agent prompt minimum content:

- Track id, scope name, briefing path
- `round_type: fast`, `model: {fast_model}`
- Reviewer invocation: `cargo make track-local-review -- --model {fast_model} --round-type fast --group {scope} --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md`
- Scope file list (files the agent is allowed to **modify** — this is the agent's modification
  boundary, distinct from the reviewer's scope which the CLI injects automatically): apply
  the CLI classifier logic to the changed file list — exclude `review_operational` and
  `other_track` matches (exception: current-track `track/items/{track-id}/**` files are never
  excluded by `other_track`), then keep files matching that group's glob patterns (named groups)
  or all remaining unmatched files (`other` scope).

The agent's modification boundary comes from the scope file list in its prompt
(see `.claude/agents/review-fix-lead.md` § Scope Ownership). Note: the CLI injecting the
scope file list into the reviewer's prompt (Step 3 constraint) is separate from this — the
orchestrator must independently derive and pass the modification boundary to the agent.

The agent's internal fix loop (review → fix → re-review until `zero_findings`),
canonical-API verification, and status reporting are owned by `.claude/agents/review-fix-lead.md`.
The orchestrator does not parse reviewer JSON directly.

Agent statuses:

- `completed` — fast `zero_findings` confirmed via canonical API → step 5 (final round)
- `blocked_cross_scope` — fix dependencies in other scopes from the orchestrator, then relaunch
- `failed` / timeout — relaunch or report to user depending on cause (treat as `findings_remain`)

## Step 5: Escalate to final round (per-scope, immediate)

When a scope's fast agent reports `completed`, **immediately** launch a `review-fix-lead`
subagent for the same scope with `round_type: final`, `model: {model}`. Do not wait for
other scopes.

Agent status handling for the `final` agent:

- `completed` → that scope is review-complete.
- `blocked_cross_scope` → fix cross-scope dependencies from the orchestrator, then relaunch.
- `failed` / timeout → relaunch or report to user depending on cause.

Each scope's lifecycle is independent — scope hashes are per-group, so modifications in one
scope do not affect another scope's hash.

## Step 6: Final validation

1. `cargo make ci` (full CI, not just `ci-rust`) — fix and re-run on failure (does not reset the loop).
2. `cargo make track-check-approved -- --track-id {track-id}` — exit 0 confirms readiness.
   Non-zero exit means review is not complete (e.g., stale hash, auto-record failure); diagnose
   and resolve before declaring readiness.

NotStarted bypass: when `review.json` does not exist and every required scope is `NotStarted`,
`check-approved` returns exit 0 to allow PR-based reviews (`/track:pr-review`) without a local
round. Once any local round is recorded, the bypass is no longer available.

## Behavior

After execution, summarize:

1. Required scopes and their `final` round verdicts
2. Findings fixed (with file references)
3. CI + `check-approved` result
4. Commit readiness and the recommended next command (`/track:commit <message>`)
