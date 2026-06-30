# Review Workflow SSoT

> Provider-agnostic workflow SSoT for the `review` track workflow. Both the Claude adapter
> (`.claude/commands/track/review.md`) and the Codex skill adapter
> (`.agents/skills/track-review/SKILL.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Run the review → fix → review cycle for the current track. The workflow drives each required
scope through at least one fast round and one final round until every required scope reaches
`zero_findings` at the `final` round level. The workflow must not complete until every required
scope reaches `final` `zero_findings` (or the `NotStarted` bypass applies — see Step 6).
No commit may proceed until this workflow reports `check-approved` success.

The review-fix loop per scope is delegated to the `review-fix-lead` capability
(`.harness/capabilities/review-fix-lead.md`). The workflow orchestrates scope discovery,
briefing preparation, and capability dispatching.

## Inputs

- **Current branch** — must match `track/<id>`. The track id is resolved from this branch. If
  the branch does not match this pattern, stop and instruct the caller to switch first.
- **Track context** — `spec.md`, `plan.md`, `metadata.json`, and all conventions listed in the
  `## Related Conventions (Required Reading)` section of `spec.md` (or `plan.md` for legacy
  tracks). For exact type signatures / module trees / Mermaid diagrams, `## Canonical Blocks`
  in `plan.md` is the source of truth.

## Sequence

**Step 0: Gather context**

Extract the track id from the current git branch (`track/<id>`). Read the current track's
`spec.md`, `plan.md`, `metadata.json`, and every convention listed under
`## Related Conventions (Required Reading)`.

**Step 1: Resolve dispatch capabilities**

Confirm that `bin/sotp review local` and the `review-fix-lead` dispatch wrapper are available.
Provider / model resolution for the reviewer and the fixer is owned by the CLI
(`bin/sotp review local` reads `capabilities.reviewer` from `.harness/config/agent-profiles.json`
internally). The workflow does not branch on provider identity.

**Step 2: Determine required scopes**

```
bin/sotp review results
```

State legend: `[+] approved` (skip) / `[-] required (...)` (run) / `[.] not required (empty)` (skip).
Scope partitioning, hash computation, and approval state are owned by the CLI. Do not
hand-classify files into groups.

**Step 3: Build per-scope briefings**

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

The CLI auto-injects the scope file list and severity policy. Do NOT hand-author the
`## Scope-specific severity policy` section: scopes with `briefing_file` configured in
`.harness/config/review-scope.json` receive the policy reference automatically via
`bin/sotp review local`.

**Step 4: Launch review-fix-lead fixers (parallel, fast round)**

For each `required` scope, launch one `review-fix-lead` capability invocation in parallel via
the provider-agnostic wrapper:

```
cargo make track-local-review-fix -- --scope {scope} \
  --briefing-file tmp/reviewer-runtime/briefing-{scope}.md \
  --round-type fast
```

The `cargo make track-local-review-fix` wrapper runs an inline `signal calc-impl-catalog`
refresh + pre-review task-contract check, then delegates to `bin/sotp review fix-local`. The
CLI resolves `capabilities.review-fix-lead.provider` from `.harness/config/agent-profiles.json`
and dispatches to the appropriate runner. The workflow carries no provider conditional.

The `review-fix-lead` capability self-resolves its modification boundary via
`bin/sotp review files --scope {scope}`. The workflow does not pass scope file lists to the
capability directly.

Fixer terminal statuses (uniform across providers):

- `completed` — fast `zero_findings` confirmed; proceed to Step 5 for this scope immediately
- `blocked_cross_scope` — fix dependencies in other scopes, then relaunch this scope
- `failed` / timeout — relaunch or report to user depending on cause

**Step 5: Escalate to final round (per-scope, immediate)**

When a scope's fast fixer reports `completed`, immediately launch the final round for the same
scope (do not wait for other scopes):

```
cargo make track-local-review-fix -- --scope {scope} \
  --briefing-file tmp/reviewer-runtime/briefing-{scope}.md \
  --round-type final
```

Provider routing follows the same rule as Step 4. Each scope's lifecycle is independent —
scope hashes are per-group, so modifications in one scope do not affect another scope's hash.

Final round fixer terminal statuses:

- `completed` — scope is review-complete
- `blocked_cross_scope` — fix cross-scope dependencies, then relaunch
- `failed` / timeout — relaunch or report to user depending on cause

**Step 6: Final validation**

1. Run `cargo make ci` (full CI, not just `ci-rust`). Fix and re-run on failure (does not
   reset the review loop).
2. Run `bin/sotp review check-approved`. Exit 0 confirms readiness. Non-zero exit means review
   is not complete (stale hash, auto-record failure, etc.); diagnose and resolve before
   declaring readiness.

**NotStarted bypass**: when `review.json` does not exist and every required scope is `NotStarted`,
`bin/sotp review check-approved` returns exit 0 to allow PR-based reviews without a local round.
Once any local round is recorded, the bypass is no longer available.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 2 | `bin/sotp review results` produces scope list | required / approved / not required |
| 5 | Each `required` scope reaches `final` `zero_findings` | completed / blocked / failed |
| 6a | `cargo make ci` exits 0 | pass / fail |
| 6b | `bin/sotp review check-approved` exits 0 | pass / fail |

All four gates must pass before the workflow reports readiness.

## Failure / recovery

- **Non-track branch**: stop and instruct the caller to switch to the `track/<id>` branch.
- **Fixer `blocked_cross_scope`**: fix the cross-scope dependencies from the orchestrator
  context, then relaunch the affected scope.
- **Fixer `failed` / timeout**: relaunch (up to 2 retries). If retries also fail, report to
  the user and ask for a decision.
- **`cargo make ci` failure**: fix the CI failure (format, clippy, test), re-run, and continue
  the workflow. CI failure does not reset the review loop.
- **`bin/sotp review check-approved` non-zero**: diagnose — stale hash (re-stage and re-run
  the final review), auto-record failure (check `review.json` state), or scope not complete
  (relaunch the incomplete scope).

## Outputs

- `tmp/reviewer-runtime/briefing-{scope}.md` files (written by this workflow, read by fixers)
- `review.json` (updated by the review-fix-lead capability)
- Per-scope `final` round verdicts (surfaced to caller)
- Findings fixed (with file references, from fixer output)
- CI result and `check-approved` result
- Commit readiness signal (pass / fail)
- No commit is created by this workflow
