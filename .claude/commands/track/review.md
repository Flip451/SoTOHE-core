---
description: Run review for current track implementation.
---

Canonical command for review in the track workflow.

Implements the review → fix → review cycle: do not commit until every required scope
reaches `final` round `zero_findings` (or the `NotStarted` bypass applies — see Step 6).

Arguments: none. The current `track/<id>` branch determines the review target.

## Step 0: Gather context

- Extract the track id from the current git branch (`track/<id>`).
  If the branch does not match this pattern, stop and instruct the user to switch first.
- Read the current track's `spec.md`, `plan.md`, `metadata.json`, and every convention listed
  in `## Related Conventions (Required Reading)` — check both `spec.md` and `plan.md` (legacy
  tracks may carry this section in `plan.md` instead of `spec.md`).
- For exact type signatures / module trees / Mermaid diagrams, treat `## Canonical Blocks` in
  `plan.md` as the source of truth.

## Step 1: Resolve dispatch capabilities

The reviewer invocation resolves its own provider/model. `bin/sotp review local` reads `capabilities.reviewer` from `.harness/config/agent-profiles.json` and resolves the reviewer
provider/model internally (including the `fast_provider` / `fast_model` fallback for
`--round-type fast`). The skill never reads or passes the reviewer provider/model.
Generated fixer prompts pass the review round, group, track id, and briefing file explicitly;
ad-hoc `bin/sotp review local` calls may still omit `--track-id` when the current branch is
`track/<id>`.

The fixer-loop dispatch is owned by the provider-agnostic wrapper in Step 4 / Step 5.
The orchestrator does not branch on `capabilities.review-fix-lead.provider`; it always invokes
`cargo make track-local-review-fix` and handles the wrapper's exit/status contract. The wrapper
delegates to `bin/sotp review fix-local`, which reads `capabilities.review-fix-lead` from
`.harness/config/agent-profiles.json` and either runs the Codex fixer or emits the Claude
subagent dispatch instruction. When the Codex fixer path runs, its internal reviewer
auto-resolves its model from `capabilities.reviewer` (round-type aware) — the orchestrator does
not pass a reviewer model.

## Step 2: Determine required scopes

```
bin/sotp review results
```

State legend: `[+] approved` (skip) / `[-] required (...)` (run) / `[.] not required (empty)` (skip).
Scope partitioning, hash computation, and approval state are owned by the CLI — do not
hand-classify files into groups.

### sotp review results flag reference

`bin/sotp review results --help` is the canonical source. Common flags: `--track-id`, `--scope`,
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
  `.harness/config/review-scope.json` (e.g., `plan-artifacts` → `.harness/custom/review-prompts/plan-artifacts.md`)
  receive the policy reference automatically via `sotp review local`.

## Step 4: Launch review-fix-lead fixers (parallel, fast round)

For each `required` scope, launch one fixer in parallel (`run_in_background: true`) via the
**provider-agnostic** wrapper:

```
cargo make track-local-review-fix -- --scope {scope} \
  --briefing-file tmp/reviewer-runtime/briefing-{scope}.md \
  --round-type fast
```

The `cargo make track-local-review-fix` task runs an inline `signal calc-impl-catalog` refresh
+ pre-review task-contract check, fail-closed) and then delegates to `bin/sotp review fix-local`.
The CLI resolves `capabilities.review-fix-lead.provider` from `.harness/config/agent-profiles.json`
and branches internally — the orchestrator skill carries **no `provider:` conditional**:

- **`provider: codex`** — the CLI constructs `CodexReviewFixRunner` and runs the entire fix loop
  inside a codex CLI subprocess (`workspace-write` sandbox; the codex skill loops calling
  `cargo make track-local-review` per round so the task-contract gate fires before every
  reviewer round inside the sandbox). The wrapper sets `CODEX_BIN` (asdf shim → real binary)
  for codex resolution. Normal fixer statuses are reported on stdout as the final
  `REVIEW_FIX_STATUS: ...` line and use the runner status exit code: `0` for `completed`, `2`
  for `blocked_cross_scope`, and `1` for `failed`. A launch-time smoke-test failure also exits
  `2`, but has no `REVIEW_FIX_STATUS` line and carries the diagnostic on stderr.
- **`provider: claude`** — the CLI **does not** spawn the fix loop. The loop is owned by a
  Claude Code subagent that must run in-process. Instead the CLI emits a structured dispatch
  instruction on stdout and exits with the dedicated subagent-dispatch exit code
  (`SUBAGENT_DISPATCH_EXIT_CODE`, currently `64`). stdout layout:

  ```
  SUBAGENT_DISPATCH_REQUIRED
  {"agent":"review-fix-lead","model":"<model>","scope":"<scope>","briefing_file":"<path>","track_id":"<id>","round_type":"<round>"}
  ```

  When the orchestrator sees exit code `64`, it parses the JSON object on the line after the
  `SUBAGENT_DISPATCH_REQUIRED` sentinel and spawns a `review-fix-lead` Claude Code subagent
  via the Agent tool (`subagent_type: "review-fix-lead"`) with those parameters. The subagent
  owns the fix loop and prints `REVIEW_FIX_STATUS: completed` / `blocked_cross_scope` /
  `failed` exactly as the codex path does. See `.claude/agents/review-fix-lead.md` for the
  subagent's internal workflow (per-round reviewer invocation uses
  `cargo make track-local-review`, which inlines the same signal + task-contract chain).

The fixer (whether codex subprocess or claude subagent) self-resolves its modification
boundary via `bin/sotp review files --scope {scope}`; the orchestrator passes neither
`--scope-files` nor `--reviewer-model`. The CLI injecting the scope file list into the
*reviewer's* prompt (Step 3 constraint) is a separate concern (the reviewer's read scope, not
the fixer's modification boundary).

The agent's internal fix loop (review → fix → re-review until `zero_findings`),
canonical-API verification, and status reporting are owned by
`.claude/agents/review-fix-lead.md` (claude path) and the codex CLI subprocess (codex path).
The orchestrator does not parse reviewer JSON directly.

Agent statuses (uniform across providers):

- `completed` — fast `zero_findings` confirmed via canonical API → step 5 (final round)
- `blocked_cross_scope` — fix dependencies in other scopes from the orchestrator, then relaunch
- `failed` / timeout — relaunch or report to user depending on cause (treat as `findings_remain`)

## Step 5: Escalate to final round (per-scope, immediate)

When a scope's fast fixer reports `completed`, **immediately** launch the final round for the
same scope (do not wait for other scopes) via the same provider-agnostic wrapper:

```
cargo make track-local-review-fix -- --scope {scope} \
  --briefing-file tmp/reviewer-runtime/briefing-{scope}.md \
  --round-type final
```

Provider routing follows the same rule as Step 4. Exit code `64` with the
`SUBAGENT_DISPATCH_REQUIRED` sentinel means spawn the Claude subagent. Exit codes `0`, `1`, or
`2` with a final stdout `REVIEW_FIX_STATUS` line mean handle that status directly. Exit code `2`
without a status sentinel is a smoke-test failure.

Agent status handling for the `final` agent:

- `completed` → that scope is review-complete.
- `blocked_cross_scope` → fix cross-scope dependencies from the orchestrator, then relaunch.
- `failed` / timeout → relaunch or report to user depending on cause.

Each scope's lifecycle is independent — scope hashes are per-group, so modifications in one
scope do not affect another scope's hash.

## Step 6: Final validation

1. `cargo make ci` (full CI, not just `ci-rust`) — fix and re-run on failure (does not reset the loop).
2. `bin/sotp review check-approved` — exit 0 confirms readiness.
   Non-zero exit means review is not complete (e.g., stale hash, auto-record failure); diagnose
   and resolve before declaring readiness.

NotStarted bypass: when `review.json` does not exist and every required scope is `NotStarted`,
`bin/sotp review check-approved` returns exit 0 to allow PR-based reviews (`/track:pr-review`)
without a local round. Once any local round is recorded, the bypass is no longer available.

## Behavior

After execution, summarize:

1. Required scopes and their `final` round verdicts
2. Findings fixed (with file references)
3. CI + `check-approved` result
4. Commit readiness and the recommended next command (`/track:commit <message>`)
