# Review Workflow SSoT

> Provider-agnostic workflow SSoT for the `review` track workflow. Both the Claude adapter
> (`.claude/commands/track/review.md`) and the Codex skill adapter
> (`.agents/skills/track-review/SKILL.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Run the review → fix → review cycle for the current track. The workflow drives each required
scope through at least one fast round and one final round until every required scope reaches
`zero_findings` at the `final` round level. The workflow must not complete until every required
scope reaches `final` `zero_findings` (or the `NotStarted` bypass applies — see Step 6). During
the Phase 0 baseline review only, the guardian-conflict exception below may instead end the
workflow `adjudication-ready`; it permits user adjudication but never commit.
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
- **Primary ADR sources** — Phase-0 init-kind ledger records are the orchestrator's primary-ADR
  designation records; no separate primary identity exists. The review prelude requires a
  nonempty init-record designation set and verifies every recorded ledger copy. It does not
  block a review for an ADR byte mismatch: a mismatch is the normal draft state during
  Phase 0. `--primary-source <file>` is available only for direct `bin/sotp adr-baseline
  check-review` invocation. Byte matching and coverage for ADRs cited by `spec.json` remain
  enforced at the commit gate and track-aware CI.

## Sequence

**Step 0: Gather context**

Extract the track id from the current git branch (`track/<id>`). Read the current track's
`spec.md`, `plan.md`, `metadata.json`, and every convention listed under
`## Related Conventions (Required Reading)`.

**Step 1: Verify the primary ADR baseline and resolve dispatch capabilities**

Before any reviewer or fixer can modify the worktree, run:

```
bin/sotp adr-baseline check-review
```

This fails closed when the ledger is missing or empty, has no init record, or a recorded ledger
copy is missing or corrupt. A current ADR
that differs from its latest baseline is a draft state and does not block review dispatch; byte
matching remains a commit-gate and track-aware-CI check.

Confirm that `bin/sotp review local` and the `review-fix-lead` dispatch wrapper are available.
Provider / model resolution for the reviewer and the fixer is owned by the CLI
(`bin/sotp review local` reads `capabilities.reviewer` from `.harness/config/agent-profiles.json`
internally). The workflow does not branch on provider identity.

**Reviewer session resume (automatic).** A round resumes the prior provider session recorded
under its track × scope × round-type × diff-base key whenever a valid entry exists — typically
the fix → re-review loop within one review cycle. A later cycle has a new diff base after its
commit, so it receives a different key and starts fresh rather than resuming prior-cycle context.
A round runs fresh when no entry exists for its key (a scope's first-ever round), when the diff
base cannot be resolved, when the recorded provider or model mismatches the current profile
resolution, or when the resume attempt fails or the session has expired. Because the round type
is part of the key, a final round never resumes a fast-round session — escalation cannot inherit
the fast context — though it may resume a prior final round with the same diff base. Resume is
built into `bin/sotp review local` — the workflow takes no extra action. On both resumed and fresh rounds,
the resolved reviewer execution configuration is re-injected; each provider-native adapter supplies
its applicable flags. Any fallback runs fresh without failing the round. Resume reuses context only: the reviewer must re-read the
CURRENT scope file list and diff and re-adjudicate the FULL scope, and the round's judgment and
review record carry the same units and meaning as a fresh round.

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

Every briefing must include one sentence requiring that findings matching the severity policy
be enumerated in full for the round — all matches reported, not truncated after the first
finding — with the same sentence stating that the severity constraints themselves remain
unchanged.

For an ADR scope, every briefing must reference
`.harness/policies/pre-track-adr-authoring.md#In-track 意味変更の裁定権`. This standing
methodology is not a consumer-owned severity preference and cannot be relaxed by a scope policy.

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

Dispatch is a single wave: launch every `required` scope's fixer before waiting on any of
them. Serializing scopes — waiting for one scope's round to finish before launching another
scope's round — is a workflow violation. The only ordering constraints in this workflow are:

1. Within one scope, `fast` precedes `final` (Step 5).
2. The DRY fixpoint (DFP) and this review fixpoint must not run concurrently
   (`full-cycle.md` orders DFP before Review).

No cross-scope ordering constraint exists for the initial wave. A
`blocked_cross_scope` terminal status is the recovery exception: resolve that dependency, then
relaunch the affected scope as specified below.

Fix boundaries are disjoint: the CLI derives the review partition from
`.harness/config/review-scope.json` (named groups plus the mandatory `other` complement), so
every file belongs to exactly one scope, and each fixer may modify only its own scope's file
list (`bin/sotp review files --scope <scope>`); a fix that needs another scope's files must
stop as `blocked_cross_scope`.

Shared operational artifacts (signal snapshots, rendered views) are CLI-owned deterministic
regenerations, not fixer-owned edits: the wrapper refreshes them at each invocation, and the
commit gate re-runs the full verification serially before any commit. Dispatch-level
synchronization of those refreshes and per-fixer build isolation are intentionally outside
this workflow's scope.

The `cargo make track-local-review-fix` wrapper runs an inline `signal calc-impl-catalog`
refresh + pre-review task-contract check, then delegates to `bin/sotp review fix-local`. The
CLI resolves `capabilities.review-fix-lead.provider` from `.harness/config/agent-profiles.json`
and dispatches to the appropriate runner. The workflow carries no provider conditional.

The `review-fix-lead` capability self-resolves its modification boundary via
`bin/sotp review files --scope {scope}`. The workflow does not pass scope file lists to the
capability directly.

**ADR-scope repair lane (before re-launching the affected review).** The orchestrator, rather
than `review-fix-lead`, owns any ADR change requested by an ADR-scoped finding, per
`.harness/policies/pre-track-adr-authoring.md#In-track 意味変更の裁定権`. The lane forks
on the Phase 0 adjudication boundary:

- **Before the boundary (Phase 0 baseline-review loop)**: when the recorded reviewer finding
  or guardian verdict explicitly identifies a NEW decision as hearing-required, route from
  that record mechanically, independent of the input ADR's lifecycle. Do not apply it or
  create a delta candidate; carry it to the user-present hearing lane. The orchestrator must
  not infer that classification. For an existing decision, dispatch `adr-editor` to apply the
  fix in place on the input-box ADR, then
  immediately dispatch `adr-diagnoser` in edit-judgment mode against the applied edit. A
  post-merge input ADR accepts only typo / reference-path / back-reference fixes in place:
  route a semantic finding on it directly to the user-present new-ADR/hearing lane instead.
  For an applied edit, decision-preserved → retain and re-launch the review;
  decision-breaking → have adr-editor revert, relay the `alternative` /
  `no_change_rationale` verbatim to the reviewer, and carry an unresolved conflict to the
  Phase 0 user adjudication. Do not create an intermediate baseline stamp.
- **After the boundary (Phase 1+)**: the input box is frozen, and the orchestrator never
  selects a lane by classifying a finding as semantic or non-semantic itself. A finding
  with no existing input-box target (it requests a new decision) routes structurally to
  delta-candidate authoring: dispatch `adr-editor` to author the candidate, then
  `adr-diagnoser` for the three-way admission judgment (admit / bounce-with-resolution /
  modification-proposal). For an existing input-box target, the recorded finding itself
  must expressly limit the proposal to a typo, broken cross-reference, or newer-ADR
  back-reference before the apply-then-classify lane is available: dispatch `adr-editor`
  to apply that proposed fix in place, then `adr-diagnoser` to classify the concrete diff.
  A non-semantic verdict retains the edit and restamps kind `non-semantic-fix`; a semantic
  or uncertain verdict reverts to the pre-edit text and re-authors the content as a delta
  candidate into the same three-way admission judgment. Every other existing-target
  proposal — including a proposal whose scope is ambiguous or expressly semantic — bypasses
  the in-place lane:
  carry the proposal verbatim to `adr-editor` for delta-candidate authoring, then to
  `adr-diagnoser` for the same three-way admission judgment. The orchestrator does not
  supply a semantic classification in either branch. An admitted draft stays 🟡 for the
  merge-stage user adjudication; a bounce removes the candidate and returns the resolution
  to the finding's origin.

Re-launch the affected ADR review only after the applicable lane completes. This lane is
capability-routed through the active profile and does not introduce a provider-specific branch.

**Phase 0 guardian-conflict exception.** When every required non-ADR scope is
`zero_findings` (approved and not-required scopes do not block), and all remaining ADR findings
are either guardian-withheld decision conflicts with their required `alternative` or
`no_change_rationale`, or hearing-required findings recorded as needing a new decision and the
user-present hearing lane, stop as `adjudication-ready` rather than relaunching the same findings. This is
not `check-approved` and cannot stage or commit; present the whole set together for the user
adjudication, then after its implementation and re-audit resume the ordinary loop to final
`zero_findings`.

The handoff channel into this lane is the recorded round itself: a fixer honoring the ADR
semantic freeze applies no ADR edit, leaves the finding recorded on the round
(`findings_remain`), and terminates with a non-`completed` status (typically `failed`). The
orchestrator reads the recorded finding via
`bin/sotp review results --track-id <track-id> --scope <adr-scope> --round-type <round-type> --limit 1`
and enters the guardian lane. No dedicated fixer status exists for this lane; for an ADR-scoped
ADR finding, `failed` + a recorded finding is the expected handoff, not a tooling error.

Fixer terminal statuses (uniform across providers):

- `completed` — fast `zero_findings` confirmed; proceed to Step 5 for this scope immediately
- `blocked_cross_scope` — fix dependencies in other scopes, then relaunch this scope
- `failed` / timeout — for an ADR-scoped finding requiring an ADR edit with a recorded round, this is the
  guardian-lane handoff (see above); otherwise relaunch or report to user depending on cause

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
- `failed` / timeout — for an ADR-scoped finding requiring an ADR edit with a recorded round, this is the
  guardian-lane handoff (see above); otherwise relaunch or report to user depending on cause

**Single-scope re-entry round (back-and-forth re-convergence surface)**

Back-and-forth loops (the `plan` workflow's Phase 1/Phase 2 loops and the `diagnose` workflow's post-routing descent, per `.harness/policies/sot-reentry-sequencing.md`) re-converge one edited upstream scope without launching the Step 4 all-required-scopes wave. The single-scope round reuses this workflow's building blocks unchanged: run Step 1 (`bin/sotp adr-baseline check-review`), prepare the target scope's briefing per Step 3, then dispatch that one scope through the same provider-neutral wrapper used by Steps 4-5 — `cargo make track-local-review-fix -- --scope {scope} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md --round-type fast`, then `--round-type final` — to `zero_findings`, honoring the same fixer terminal statuses, the ADR-scope repair lane when the scope is `adr`, and re-launches after applied fixes. Downstream scopes deliberately stay un-launched: their re-entry follows only after this upstream scope re-converges. This surface changes no gate: `bin/sotp review check-approved` and the commit gate still require every required scope to be approved before any commit.

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
| 1 | `bin/sotp adr-baseline check-review` exits 0 | pass / fail |
| 2 | `bin/sotp review results` produces scope list | required / approved / not required |
| 5 | Each `required` scope reaches `final` `zero_findings` | completed / blocked / failed |
| 6a | `cargo make ci` exits 0 | pass / fail |
| 6b | `bin/sotp review check-approved` exits 0 | pass / fail |

All five gates must pass before the workflow reports readiness.

## Failure / recovery

- **Non-track branch**: stop and instruct the caller to switch to the `track/<id>` branch.
- **ADR baseline review check failure**: stop before dispatching a reviewer or fixer. For a
  missing init snapshot, use the sanctioned init snapshot route. A byte mismatch is not a
  review-check failure: continue the draft review loop and let the commit gate / track-aware CI
  enforce byte matching.
- **Fixer `blocked_cross_scope`**: fix the cross-scope dependencies from the orchestrator
  context, then relaunch the affected scope.
- **Fixer `failed` / timeout**: for an ADR-scoped finding requiring an ADR edit with a recorded round, enter
  the guardian lane before any retry. Otherwise relaunch (up to 2 retries). If retries also
  fail, report to the user and ask for a decision.
- **`cargo make ci` failure**: fix the CI failure (format, clippy, test), re-run, and continue
  the workflow. CI failure does not reset the review loop.
- **`bin/sotp review check-approved` non-zero**: diagnose — stale hash (re-stage and re-run
  the final review), auto-record failure (check `review.json` state), or scope not complete
  (relaunch the incomplete scope).

## Outputs

- `tmp/reviewer-runtime/briefing-{scope}.md` files (written by this workflow, read by fixers)
- `review.json` (updated by the review-fix-lead capability)
- Per-scope `final` round verdicts (surfaced to caller), or, for the Phase 0
  guardian-conflict exception, an explicit `adjudication-ready` terminal outcome instead of
  review completion. That outcome carries the grouped ADR findings: each guardian-withheld
  proposal with its verbatim `alternative` / `no_change_rationale`, and each
  hearing-required proposal with its recorded grounds, plus the init-stamp diff for the user
  adjudication handoff.
- Findings fixed (with file references, from fixer output)
- CI result and `check-approved` result
- Commit readiness signal (pass / fail)
- No commit is created by this workflow
