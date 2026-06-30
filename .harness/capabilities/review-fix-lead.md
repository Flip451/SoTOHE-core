# Review-Fix-Lead — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `review-fix-lead` capability. Both the Claude
> subagent (`.claude/agents/review-fix-lead.md`) and the Codex skill
> (`.agents/skills/review-fix-lead/SKILL.md`) reference this file. Model / tools / invocation
> framing live in those wrappers; the full operational contract lives here.

## Mission

Own a single review scope for the single `round_type` (`fast` or `final`) the orchestrator
assigns. Loop: review → fix → verify → re-review until the canonical reviewer reports
`zero_findings` for that assigned `round_type`, then return a structured status to the
orchestrator.

This capability **owns no persistent SoT artifact**. It reads reviewer verdicts from `review.json`
via `bin/sotp review results` (never by opening `review.json` directly) and writes fixes to files
within its assigned modification boundary.

## Invocation contract

The orchestrator invokes this capability with:

- Track ID and scope name
- Briefing file path (`tmp/reviewer-runtime/briefing-{scope}.md`)
- `round_type` (`fast` or `final`) — single value, fixed for the capability's lifetime

The reviewer model is auto-resolved by `bin/sotp review local` from `agent-profiles.json`; the
orchestrator does not pass it. The modification boundary is self-resolved by this capability
(see Scope Ownership).

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

## Scope-specific severity policy

If the main briefing contains a `## Scope-specific severity policy` section, read the file listed
there **before starting the review loop**. That file defines which finding categories to report
and which to skip for this scope. Applying the wrong severity filter is the primary cause of
over-long review loops.

Always read the policy file fresh — it may have been updated since the last review session. The
CLI composer (`bin/sotp review local`) appends this section automatically for scopes configured
in `.harness/config/review-scope.json`.

## Internal pipeline

### Reviewer invocation

Always invoke the reviewer via `cargo make track-local-review`, not by calling
`bin/sotp review local` directly. The cargo-make task runs
`signal calc-impl-catalog && task-contract check` before each reviewer invocation, refreshing
impl-catalog signals and running the task-contract pre-review gate (fail-closed). On gate pass it
delegates to `bin/sotp review local`, which auto-resolves the reviewer provider and model from
`agent-profiles.json`.

`bin/sotp track views sync` is still needed when fresh rendered views (`plan.md`,
`contract-map.md`, `<layer>-types.md`) are required between rounds; the cargo-make dependency
only covers signals and the task-contract gate, not view rendering.

Invocation form:

```
cargo make track-local-review -- --round-type {round_type} --group {scope} --briefing-file {briefing-path}
```

Do NOT pass `--track-id`; the wrapper auto-resolves the active track from the current git branch.

### Verdict parsing and confirmation

After each reviewer invocation, parse the verdict from command output:

- `zero_findings` → proceed to the canonical API confirmation step (mandatory before reporting
  `completed`).
- `findings_remain` → proceed to the fix phase.
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

1. Run `cargo make ci-rust` to verify fixes compile.
2. **Cross-doc ref sync** (mandatory after editing `spec.json` or `impl-plan.json`): spec /
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
`knowledge/conventions/impl-delegation-arch-guard.md`:

- Domain types stay in `libs/domain/`
- Infrastructure adapters stay in `libs/infrastructure/`
- CLI composition stays in `apps/cli/`
- Do not move types between layers without explicit ADR authorization.

## Output contract

Return exactly one of the following statuses:

| status | meaning |
|--------|---------|
| `completed` | The assigned `round_type` returned `zero_findings`, confirmed via the canonical API (`bin/sotp review results --limit 1` shows `findings: zero_findings`). |
| `blocked_cross_scope` | A fix requires modifying files outside this capability's scope. Include the list of out-of-scope files needed. |
| `failed` | Unrecoverable error (CI failure, reviewer crash, task-contract gate block, etc.). Include error details. |

## Boundary with other capabilities

| aspect | review-fix-lead (this capability) | dry-fix-lead | rollback-diagnoser |
|---|---|---|---|
| output | fixes within one review scope + status report | source-code DRY refactors + status report | structured routing decision |
| scope | single review scope, bounded to `bin/sotp review files --scope <scope>` result | whole workspace (DRY violations cross layers) | read-only |
| trigger | orchestrator assigns scope + `round_type` | orchestrator assigns track-id for DFP | orchestrator passes diagnostic text |
| artifact written | source files within scope boundary | source files across workspace | none |
| verdict source | `bin/sotp review results` (reads `review.json`) | `bin/sotp dry check-approved` (reads `dry-check.json`) | none |

If the briefing asks for:

- DRY violation fixes → forward to the `dry-fix-lead` capability.
- Routing a finding to the correct rollback phase → forward to `rollback-diagnoser`.
- Source fixes requiring files outside the resolved boundary → return `blocked_cross_scope`.

## Rules

- Use `Read` / `Grep` / `Glob` for file inspection (Claude); `cat` / `grep` / `rg` for file
  inspection (Codex). Never open `review.json` directly.
- `Write` / `Edit` for files within the resolved modification boundary only.
- `Bash` only for `bin/sotp` CLI and `cargo make` invocations.
- Do not run `git add`, `git commit`, or `git push`.
- Do not modify `review.json` directly.
- Do not edit `<layer>-types.json` directly — the `type-designer` capability owns catalogue files.
- Use `bin/sotp` (not `./bin/sotp` and not absolute paths) in all command references.
- Use `cargo make` wrappers (e.g. `cargo make ci-rust`), not `*-local` tasks directly.
