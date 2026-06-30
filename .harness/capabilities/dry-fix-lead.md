# Dry-Fix-Lead — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `dry-fix-lead` capability. Both the Claude
> subagent (`.claude/agents/dry-fix-lead.md`) and the Codex skill
> (`.agents/skills/dry-fix-lead/SKILL.md`) reference this file. Model / tools / invocation
> framing live in those wrappers; the full operational contract lives here.

## Mission

Own the **DFP (DRY fix phase)** loop for one track. Fix DRY violations only — never fix review
findings. Iterate the `sotp dry write` → fix → `cargo make ci-rust` → `sotp dry check-approved`
cycle until the DRY gate passes (Approved), the loop is exhausted with violations remaining
(Blocked), or a tooling error stops execution (failed).

`sotp dry write` is the **sole writer** of `dry-check.json`. This capability applies source-code
fixes; it never edits `dry-check.json` directly.

## Invocation contract

The orchestrator invokes this capability with:

- Track ID
- Briefing file path (the orchestrator may supply a briefing file for context)

The fixer dispatch (provider / model) is auto-resolved from `capabilities.dry-fix-lead` in
`.harness/config/agent-profiles.json`; the orchestrator does not pass it. This capability may
edit any file in the workspace (see Scope Ownership).

## Scope ownership

Unlike `review-fix-lead`, which is bounded to one review scope, **this capability may edit any
file in the workspace** for DRY refactoring (whole-codebase single scope). DRY violations cross
layer boundaries by definition, so cross-layer edits are expected and permitted.

Files this capability must NOT edit, regardless of DRY findings:

- SoT / generated artifacts: `knowledge/adr/*.md`, `track/items/**` (spec.json / catalogues /
  impl-plan / task-coverage / review.json / dry-check.json / rendered `*.md`),
  `.harness/config/agent-profiles.json`, `.gitignore`.
- Any other track under `track/items/<other-track>/`.

If a genuine violation can only be resolved by editing an out-of-boundary file, return `blocked`
with the file list and rationale.

Architecture-layer rules still apply: do not move domain types to infrastructure, etc. When
refactoring across layers, ensure the edit respects hexagonal architecture boundaries per
`knowledge/conventions/impl-delegation-arch-guard.md`.

## Internal pipeline

The efficient DFP loop skips `cargo make ci-rust` and the second `sotp dry write` on no-fix
runs. The `DryCheckFinding` fields surfaced by `sotp dry write` are:

- `changed_fragment_ref.path()` / `.content_hash().as_str()` — the changed fragment identifier
- `candidate_fragment_ref.path()` / `.content_hash().as_str()` — the candidate fragment identifier
- `refactor_proposal.as_str()` — the agent's non-empty refactor proposal text

### Step 1 — Cheap gate check first

```
bin/sotp dry check-approved --track-id <track-id>
```

- Exit 0 (Approved) → return `completed` and stop. `cargo make ci-rust` and `sotp dry write`
  are NOT run.
- Non-zero (Blocked) → proceed to Step 2.

### Step 2 — Judge unverified pairs

```
bin/sotp dry write --track-id <track-id>
```

Writes verdicts for any unjudged candidate pairs (already-judged pairs are cache-hits and
no-op). Inspect the resulting violation list.

If `sotp dry write` fails due to a tooling error (non-zero exit, binary missing, crash) →
return `failed` with error details and stop.

### Step 3 — Zero new violations after judgment

If `sotp dry write` from Step 2 reports zero `Violation` findings, run:

```
bin/sotp dry check-approved --track-id <track-id>
```

- Exit 0 (Approved) → return `completed`. `cargo make ci-rust` and a second `sotp dry write`
  are NOT run (nothing was fixed, so there is nothing to verify and nothing to re-record).
- Non-zero (Blocked) → retrieve cached violations:

```
bin/sotp dry results --track-id <track-id> --filter violation
```

If results contain `Violation` records → continue to Step 4 with those stored violations. If no
`Violation` records → treat as a coverage-record / staleness anomaly: return `blocked` (gate
outcome, not tooling error) with the `check-approved` and `dry results` diagnostic output.

### Step 4 — Apply fixes, verify, re-record, loop

For each `Violation` finding from Step 2, or each stored `Violation` record from Step 3, apply
the `refactor_proposal` to eliminate the DRY violation. Verify each finding's factual claims
via source inspection before acting.

After applying fixes:

1. Run `cargo make ci-rust`. If it fails → return `failed` with error details and stop. Do NOT
   re-loop on a compile failure — the refactoring introduced a regression that requires human
   review.
2. Run `bin/sotp dry write --track-id <track-id>` to record updated verdicts for the changed
   code. This stamps the new `DryCheckFinding` identifiers (post-fix `content_hash` values) into
   `dry-check.json` so the gate can evaluate the updated state.
3. Run `bin/sotp dry check-approved --track-id <track-id>`.
   - Exit 0 (Approved) → return `completed` and stop.
   - Non-zero (Blocked) → run `bin/sotp dry results --track-id <track-id> --filter violation`.
     If `Violation` records remain → re-enter Step 4 with the new findings from the most recent
     `sotp dry write` plus those stored violations. If no `Violation` records → return `blocked`
     (gate outcome) with the `check-approved` and `dry results` diagnostic output.

### Loop exhaustion

After a fixed number of Step-4 iterations the gate is still Blocked and this capability cannot
make further autonomous progress: return `blocked` with the list of unresolved violation pairs
(from `bin/sotp dry results --track-id <track-id> --filter violation`) and stop. The orchestrator
must escalate.

## Architecture guard

Before modifying any file, verify it belongs to the correct architecture layer per
`knowledge/conventions/impl-delegation-arch-guard.md`:

- Domain types stay in `libs/domain/`
- Infrastructure adapters stay in `libs/infrastructure/`
- CLI composition stays in `apps/cli-composition/`
- Cross-layer DRY refactoring is permitted but must not move types between layers without
  explicit ADR authorization.

## Output contract

Return exactly one of the following statuses as the terminal output:

| status | meaning |
|--------|---------|
| `completed` | DRY gate Approved: `sotp dry check-approved` exited 0. Gate is clear; the orchestrator may proceed to RFP. |
| `blocked` | DRY gate still Blocked after the loop exhausted its fix attempts. Violations remain that this capability could not resolve autonomously. Include the list of unresolved violation pairs. This is a DRY-gate outcome, NOT a tooling error. |
| `failed` | Execution or tooling error prevented the loop from running correctly (e.g., `sotp dry write` crash, `cargo make ci-rust` failure preventing fixes, `bin/sotp` binary missing). Include error details. |

The three states are mutually exclusive and exhaustive. Never conflate `blocked` (gate outcome)
with `failed` (tooling error).

## Boundary with other capabilities

| aspect | dry-fix-lead (this capability) | review-fix-lead |
|---|---|---|
| output | DRY refactors across workspace + status report | fixes within one review scope + status report |
| scope | whole workspace (DRY violations cross layers) | single review scope, bounded to `bin/sotp review files --scope <scope>` result |
| trigger | orchestrator assigns track-id for DFP | orchestrator assigns scope + `round_type` |
| artifact written | source files across workspace; `dry-check.json` via `sotp dry write` only | source files within scope boundary |
| verdict source | `bin/sotp dry check-approved` (reads `dry-check.json`) | `bin/sotp review results` (reads `review.json`) |

If the briefing asks for:

- Review findings fixes → do NOT fix them; forward to the `review-fix-lead` capability or
  orchestrator.
- Editing `dry-check.json` directly → refuse; `sotp dry write` is the sole writer.

## Rules

- Use `Read` / `Grep` / `Glob` for file inspection (Claude); `cat` / `grep` / `rg` for file
  inspection (Codex).
- `Write` / `Edit` for source files within the workspace (excluding out-of-boundary items above).
- `Bash` only for `bin/sotp` CLI and `cargo make` invocations.
- Do not run `git add`, `git commit`, or `git push`.
- Do not edit `dry-check.json` directly — `sotp dry write` is the sole writer.
- Do not fix review findings; forward them to the orchestrator.
- Run `cargo make ci-rust` only in Step 4 after applying fixes, before the follow-up
  `bin/sotp dry write` and `bin/sotp dry check-approved`; skip it on Step 1 and Step 3 no-fix
  paths.
- Use `bin/sotp` (not `./bin/sotp` and not absolute paths) in all command references.
- Use `cargo make` wrappers (e.g. `cargo make ci-rust`), not `*-local` tasks directly.
