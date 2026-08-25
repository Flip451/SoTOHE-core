# Review-Fix-Lead — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `review-fix-lead` capability. Both the Claude
> subagent (`.claude/agents/review-fix-lead.md`) and the Codex skill
> (`.agents/skills/review-fix-lead/SKILL.md`) reference this file. Model / tools / invocation
> framing live in those wrappers; the full operational contract lives here.

## Mission

For a normal review-scope assignment, own a single review scope for the single `round_type`
(`fast` or `final`) the orchestrator assigns. Loop: review → fix → verify → re-review until the
canonical reviewer reports `zero_findings` for that assigned `round_type`, then return a
structured status to the orchestrator.

For an actionable review-scope finding delegated by `pr-review`, handle only the focused
correction in its briefing: make the scoped change, run local verification, and report completion
to the caller. This delegated path does not launch an unrelated review → fix → re-review loop; the
caller retains local review convergence, the commit workflow, and the PR re-run. If the correction
edits source, local verification also includes the placement and dependency checks in
implementation-delegation R2 and `cargo make check-layers`; documentation-only corrections use
the verification required by their briefing.

This capability **owns no persistent SoT artifact**. It reads reviewer verdicts from `review.json`
via `bin/sotp review results` (never by opening `review.json` directly) and writes fixes to files
within its assigned modification boundary.

## Invocation contract

The orchestrator invokes this capability in one of two dispatch modes:

- `scope-review` — the normal review-fix loop for one assigned scope and one review round.
- `delegated-pr-finding` — a focused correction delegated by `pr-review`; it does not start a
  separate scope-wide reviewer loop.

The caller selects the mode as follows: a normal review-fix wrapper invocation with a `round_type`
is `scope-review`; a focused PR-finding briefing must explicitly carry
`dispatch_mode: delegated-pr-finding`. The caller invokes this capability with:

- Track ID and scope name
- Briefing file path (`tmp/reviewer-runtime/briefing-{scope}.md`) containing the CLI-summary and
  current-diff scope context, the exact spec / plan / task / catalogue paths needed for the
  review, and the resolved convention paths (possibly none). For a delegated PR finding, the
  briefing also contains the review comment, affected path and line, relevant track context, and
  requested correction. If the delegated correction permits source edits, it also contains the
  `## Architecture Constraints` section required by implementation-delegation R1.
- For `scope-review`, `round_type` (`fast` or `final`) is a single value fixed for the capability's
  lifetime. `delegated-pr-finding` does not require a `round_type`; if the provider-neutral wrapper
  carries one for transport, it is not a reviewer-verdict obligation for that dispatch.

The reviewer model is auto-resolved by `bin/sotp review local` from `agent-profiles.json`; the
orchestrator does not pass it or bulk-read the briefing's artifact bodies. This capability reads
the listed paths itself. The modification boundary is self-resolved by this capability (see Scope
Ownership).

## Scope ownership (CRITICAL)

This capability self-resolves its modification boundary by running:

```
bin/sotp review files --scope <scope>
```

The returned file list is the only set of files this capability may modify. If the command returns
an empty list or fails, make no edits and return `failed` with the reason.

- Files outside the resolved boundary: do NOT modify. Return `blocked_cross_scope` with the
  out-of-scope file list so the orchestrator can re-partition.
- Cross-scope edits are fail-closed: silent out-of-scope modifications are prohibited.
- For a delegated PR finding, treat the briefing's comment, affected `path:line`, track context,
  and requested correction as one focused change request. Do not broaden it beyond the returned
  file list; if the correction needs another scope, return `blocked_cross_scope`.
- Do not run `bin/sotp track transition`; this capability has no task-state transition authority.

## Scope-specific severity policy

If the main briefing contains a `## Scope-specific severity policy` section, read the file listed
there **before starting the review loop**. That file defines which finding categories to report
and which to skip for this scope. Applying the wrong severity filter is the primary cause of
over-long review loops.

Always read the policy file fresh — it may have been updated since the last review session. The
CLI composer (`bin/sotp review local`) appends this section automatically for scopes configured
in `.harness/config/review-scope.json`.

## ADR baseline semantic freeze

When a finding requires *any* edit to an ADR, never make that edit yourself. Under the two-box
model (`.harness/policies/pre-track-adr-authoring.md` §In-track 意味変更の裁定権), the
orchestrator routes every ADR change through adr-editor and adr-diagnoser: Phase 0 uses the
in-place convergence or user-present hearing lane; after the Phase 0 adjudication boundary,
semantic changes use the delta lane and proposed non-semantic changes use the
apply-then-classify lane. Judge ADR completeness by whether it faithfully records the decision,
not by whether a different design would be preferable. A current ADR byte mismatch before the
boundary is a normal draft state and does not block a review loop; byte matching is enforced at
the commit gate and track-aware CI. If the ADR-baseline review check blocks on its retained
ledger-integrity conditions, stop the fixer and let the orchestrator use the sanctioned recovery
route described by the review workflow.

**Termination contract (guardian-lane handoff).** When a round records a finding whose fix
requires an ADR edit, do not re-loop on it: apply any other in-scope fixes that do not touch an
ADR, leave the ADR finding recorded on the round (`findings_remain`), and terminate immediately
with `failed`, citing that finding as the reason. This `failed` is the deterministic handoff into
the orchestrator's guardian lane
(`.harness/workflows/track/review.md` Step 4), not a tooling error; continuing the loop in the
hope that the reviewer withdraws the finding is prohibited.

## Internal pipeline

The reviewer invocation and canonical verdict-confirmation pipeline below applies to
`scope-review` dispatches. A `delegated-pr-finding` dispatch returns through the delegated
completion contract after its focused correction and required local verification; it does not
invoke the reviewer or claim `zero_findings`.

### Reviewer invocation

Always invoke the reviewer via `cargo make track-local-review`, not by calling
`bin/sotp review local` directly. The wrapper delegates to `bin/sotp review local`, which
resolves the review scope and dispatches the operator-owned pre-review command sequence
declared for that scope in `.harness/config/pre-review-gates.json` (fail-closed; scopes with
an empty command vector run no gate). The CLI also auto-resolves the reviewer provider and
model from `agent-profiles.json`.

Run `cargo make track-views-sync` when fresh rendered views (`plan.md`, `contract-map.md`,
`<layer>-types.md`) are required between rounds. This explicit fixer refresh route regenerates
views only; the scope-conditional dispatch remains the sole owner of pre-review gate commands.

Invocation form:

```
cargo make track-local-review -- --round-type {round_type} --group {scope} --briefing-file {briefing-path}
```

Do NOT pass `--track-id`; the wrapper auto-resolves the active track from the current git branch.

The reviewer subprocess may run for many minutes. Invoke `cargo make track-local-review` once as
a single blocking call and read its terminal result once. If the host backgrounds the call, read
the result once after the single completion notification. Do not poll its output, re-run status
probes, or launch it fire-and-forget. The `bin/sotp review results` command below is a single
post-completion confirmation read, not a polling loop. Re-review iterations occur only after the
review result or a state-changing fix requires another round.

### Verdict parsing and confirmation

After each reviewer invocation, parse the verdict from command output:

- `zero_findings` → proceed to the canonical API confirmation step (mandatory before reporting
  `completed`).
- `findings_remain` → proceed to the fix phase. If one or more findings requires an ADR edit,
  apply every other in-scope non-ADR fix, then terminate per the guardian-lane handoff
  (§ADR baseline semantic freeze).
- Error → return `failed`.

**Canonical API confirmation (mandatory before reporting `completed`):**

```
bin/sotp review results --track-id {track-id} --scope {scope} --round-type {round_type} --limit 1
```

Read the **findings block** under the state-line, not the state-line itself. The state-line
reflects merge-gate readiness for the scope (combining fast verdict + final verdict + hash
freshness) and is NOT a per-round verdict. For `round_type == fast`, the state-line may show
`[-] required (stale hash)` even when this fast round is `zero_findings`; use only the findings
block in that case.

- `round_type == fast`: if findings block shows `findings: zero_findings` → return `completed`.
  If findings remain or no entry exists → re-loop.
- `round_type == final`: if state-line is `[+]`/`approved` AND findings block shows
  `findings: zero_findings` → return `completed`. Otherwise → re-loop.

### Fix phase

Apply fixes only to files within the resolved modification boundary. Verify each finding's
factual claims via source inspection before acting.

Priority handling:
- P3 findings from pre-existing unchanged code: note but do not fix.
- P0/P1/P2: implement the fix within scope boundaries.

After applying fixes:

1. Run `cargo make ci-rust` to verify fixes compile. Treat this long-running gate as one blocking
   call and read its terminal result once; if the host backgrounds it, read the result once after
   the single completion notification. Do not poll its logs or status.
2. If any fix edited source, before re-invoking the reviewer repeat the pre-review verification
   required by `.harness/policies/implementation-delegation.md#R2. review 起動前に配置を検証する`.
   The review-fix lead owns this repeat because its source edits occur after the earlier
   placement verification; documentation-only fixes do not require it.
3. **Cross-doc ref sync** (mandatory after editing `spec.json` or `impl-plan.json`): spec /
   impl-plan anchor changes can cause catalogue `spec_refs[].anchor` to go stale. Run
   `cargo make verify-plan-artifact-refs` explicitly (not included in `cargo make ci-rust`; only
   in `cargo make ci`). Note: catalogue `spec_refs[]` has no `hash` field (removed in
   schema_version 4; `deny_unknown_fields` rejects it). If `unresolved SpecRef anchor` errors
   appear, the fix requires the `type-designer` capability — this capability must NOT edit
   `<layer>-types.json` directly; return `failed` with the mismatch details so the orchestrator
   can delegate.

### Prior-round findings

Read prior-round findings via `bin/sotp review results`, never by opening `review.json` directly:

```
bin/sotp review results --track-id {track-id} --scope {scope} --round-type {round_type} --limit N
```

Keep N small (1–3) to avoid context bloat.

## Architecture guard

Before modifying any file, verify it belongs to the correct architecture layer per
`.harness/policies/implementation-delegation.md`:

- Domain types and domain ports stay in `libs/domain/`
- Usecase interactors and usecase ports stay in `libs/usecase/`
- Infrastructure adapters stay in `libs/infrastructure/`
- CLI composition-root wiring stays in `apps/cli-composition/` (the `apps/cli` crate is the bin entry point only)
- `apps/cli-driver` is the primary adapter layer
- Do not move types between layers without explicit ADR authorization.

## Output contract

Return exactly one of the following qualified statuses:

| status | dispatch mode | meaning |
|--------|---------------|---------|
| `completed` | `scope-review` | The assigned `round_type` returned `zero_findings`, confirmed via the canonical API (`bin/sotp review results --limit 1` shows `findings: zero_findings`). |
| `completed` | `delegated-pr-finding` | The focused correction and required local verification passed. The completion report must include `dispatch_mode: delegated-pr-finding`, the affected `path:line`, requested correction, changed files, and verification result; this status does not assert `zero_findings`. |
| `blocked_cross_scope` | either | A fix requires modifying files outside this capability's scope. Include the list of out-of-scope files needed. |
| `failed` | either | Unrecoverable error (CI failure, reviewer crash, task-contract gate block, etc.), or the ADR guardian-lane handoff: an ADR finding requiring an ADR edit remains recorded under the semantic freeze (§ADR baseline semantic freeze). Include error details or the finding reference. |

## Boundary with other capabilities

| aspect | review-fix-lead (this capability) | dry-fix-lead | rollback-diagnoser |
|---|---|---|---|
| output | fixes within one review scope + status report | source-code DRY refactors + status report | structured routing decision |
| scope | single review scope, bounded to `bin/sotp review files --scope <scope>` result | whole workspace (some DRY violations span layers) | read-only |
| trigger | orchestrator assigns scope + `round_type`, or delegates a focused PR finding | orchestrator assigns track-id for DFP | orchestrator passes diagnostic text |
| artifact written | source files within scope boundary | source files across workspace | none |
| verdict source | `bin/sotp review results` (reads `review.json`) | `bin/sotp dry check-approved` (reads `dry-check.json`) | none |

If the briefing asks for:

- DRY violation fixes → forward to the `dry-fix-lead` capability.
- Routing a finding to the correct rollback phase → forward to `rollback-diagnoser`.
- Source fixes requiring files outside the resolved boundary → return `blocked_cross_scope`.

For a delegated PR finding, return the scoped change and verification result to the caller; do not
commit, push, or re-run the PR from this capability. The caller performs local review convergence
and invokes the commit workflow before re-running `pr-review`. Name the finding's affected
`path:line`, requested correction, changed files, and verification result in that report.

## Rules

- Use `Read` / `Grep` / `Glob` for file inspection (Claude); `cat` / `grep` / `rg` for file
  inspection (Codex). Never open `review.json` directly.
- `Write` / `Edit` for files within the resolved modification boundary only.
- `Bash` only for `bin/sotp` CLI and `cargo make` invocations.
- Do not run `git add`, `git commit`, or `git push`.
- Do not modify `review.json` directly.
- Do not edit `<layer>-types.json` directly — the `type-designer` capability owns catalogue files.
- Do not edit `batch-plan.json` — the `impl-planner` capability is its sole writer. Although
  the file classifies into the impl-plan scope, it sits **outside this capability's resolved
  write boundary** (the boundary is the writable file set, not the scope). A finding requiring
  an estimate or batch change therefore terminates as `blocked_cross_scope`, with the finding
  and `impl-planner` named as the required owner in the report; the calling orchestrator's
  recovery is to dispatch impl-planner and relaunch the scope review.
- Use `bin/sotp` (not `./bin/sotp` and not absolute paths) in all command references.
- Use `cargo make` wrappers (e.g. `cargo make ci-rust`), not `*-local` tasks directly.
- Do not run `bin/sotp test-obligation evaluate`; it is an orchestrator-host-owned synchronous
  repair step, never a fire-and-forget launch or a commit prerequisite started by this capability.

## Session resume

When dispatched as a resumed session (orchestrator opt-in continuation of the same track and
capability), do not trust context carried over from the prior session: first check whether the
upstream artifacts of this assignment (ADR, `spec.json`, type catalogues, `impl-plan.json`, the
review briefing — as applicable) changed since that session, and re-read any that did before
continuing. All execution flags are explicitly re-specified by the dispatcher on resume; a
failed or expired resume falls back to a fresh session.
