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
  `plan.md` and `knowledge/DESIGN.md` as the source of truth.

## Step 1: Resolve dispatch capabilities

The reviewer invocation resolves its own provider/model. `bin/sotp review local` reads `capabilities.reviewer` from `.harness/config/agent-profiles.json` and resolves the reviewer
provider/model internally (including the `fast_provider` / `fast_model` fallback for
`--round-type fast`). The skill never reads or passes the reviewer provider/model.
Generated fixer prompts pass the review round, group, track id, and briefing file explicitly;
ad-hoc `bin/sotp review local` calls may still omit `--track-id` when the current branch is
`track/<id>`.

Read `capabilities.review-fix-lead` to resolve the fixer-loop dispatch (used by the Step 4 / Step 5 dispatch):
- `capabilities.review-fix-lead.provider` — `claude` (default) launches the `review-fix-lead` subagent; `codex` launches the Codex fixer wrapper instead.
- `capabilities.review-fix-lead.model` — the fixer model passed to the chosen provider. **When `provider: codex`, this field must also be set to a Codex-compatible model; leaving a Claude model name here will cause `codex exec --model` to receive an invalid model name.**
- If the `review-fix-lead` capability is absent, default to `provider: claude` (legacy behavior unchanged).

The `provider: codex` fixer wrapper (`bin/sotp review fix-local`, Step 4 / Step 5) runs its own
internal Codex reviewer; that reviewer auto-resolves its model from `agent-profiles.json`
(`capabilities.reviewer`, round-type aware) — the orchestrator does not pass a reviewer model.

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
  `track/review-scope.json` (e.g., `plan-artifacts` → `track/review-prompts/plan-artifacts.md`)
  receive the policy reference automatically via `sotp review local`.

## Step 4: Launch review-fix-lead fixers (parallel, fast round)

For each `required` scope, launch one fixer in parallel (`run_in_background: true`),
dispatching on `capabilities.review-fix-lead.provider` (resolved in Step 1):

- **`provider: claude`** (default) — launch a `review-fix-lead` subagent
  (`subagent_type: "review-fix-lead"`). Use the prompt content below.
- **`provider: codex`** — instead of the subagent, launch the Codex fixer wrapper via Bash:
  `cargo make track-local-review-fix-codex -- --scope {scope} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md --track-id {track-id} --round-type fast`
  The `cargo make` wrapper resolves `CODEX_BIN` (asdf shim → real binary) then delegates to
  `bin/sotp review fix-local`. The `--model` flag is optional; when omitted the CLI resolves it
  from `agent-profiles.json` `review-fix-lead.model` (or `fast_model` for fast round).
  The codex fixer self-resolves its modification boundary via `bin/sotp review files --scope`;
  the orchestrator passes neither `--scope-files` nor `--reviewer-model`.
  The wrapper runs the same review → fix → re-review loop inside a `workspace-write` sandbox
  (`.git` is read-only there, so the fixer edits files but cannot stage/commit), performs
  launch-time smoke-tests, isolates credentials, and prints `completed` /
  `blocked_cross_scope` / `failed` — the same status contract as the subagent. The Claude
  subagent definition (`.claude/agents/review-fix-lead.md`) is unchanged and remains the
  `provider: claude` path.

Agent prompt minimum content (`provider: claude` path):

- Track id, scope name, briefing path
- `round_type: fast`, `model: {review-fix-lead.model}` (resolved from `capabilities.review-fix-lead.model`)
- Pre-review gate (run before the reviewer invocation): per TDDD-active layer,
  `bin/sotp signal calc-impl-catalog --signals-path <...> --catalog-hash <...>` and
  `bin/sotp signal calc-catalog-spec --signals-path <...> --catalog-hash <...>`,
  then `bin/sotp track views sync` — see `.claude/agents/review-fix-lead.md` § Workflow
- Reviewer invocation: `bin/sotp review local --round-type fast --group {scope} --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md`
The agent self-resolves its modification boundary by running `bin/sotp review files --scope {scope}`
(see `.claude/agents/review-fix-lead.md` § Scope Ownership). The orchestrator does NOT derive or
pass a scope file list — neither provider path duplicates scope classification. The CLI injecting
the scope file list into the *reviewer's* prompt (Step 3 constraint) is a
separate concern (the reviewer's read scope, not the fixer's modification boundary).

The agent's internal fix loop (review → fix → re-review until `zero_findings`),
canonical-API verification, and status reporting are owned by `.claude/agents/review-fix-lead.md`.
The orchestrator does not parse reviewer JSON directly.

Agent statuses:

- `completed` — fast `zero_findings` confirmed via canonical API → step 5 (final round)
- `blocked_cross_scope` — fix dependencies in other scopes from the orchestrator, then relaunch
- `failed` / timeout — relaunch or report to user depending on cause (treat as `findings_remain`)

## Step 5: Escalate to final round (per-scope, immediate)

When a scope's fast fixer reports `completed`, **immediately** launch the final round for the
same scope (do not wait for other scopes), dispatching on `capabilities.review-fix-lead.provider`:

- **`provider: claude`** — launch a `review-fix-lead` subagent with `round_type: final`, `model: {review-fix-lead.model}` (resolved from `capabilities.review-fix-lead.model`).
- **`provider: codex`** — run `cargo make track-local-review-fix-codex -- --scope {scope} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md --track-id {track-id} --round-type final` (no `--reviewer-model` / `--scope-files`; the codex fixer self-resolves boundary + reviewer model; `--model` omitted = auto-resolved from `agent-profiles.json`).

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
