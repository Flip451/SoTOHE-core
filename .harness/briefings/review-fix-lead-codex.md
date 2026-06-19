# Review-Fix-Lead Agent (Codex)

This briefing is the Codex-provider parallel version of `.claude/agents/review-fix-lead.md`.
The shared sections (Mission, Contract, Scope Ownership, Severity Policy, Workflow, Architecture
Guard, Rules) are aligned in wording with the Claude version. The tool-instruction sections
are translated to Codex shell idioms.

## Mission

Own a single review scope (e.g., `domain`, `infrastructure`, `cli`) for the **single round_type**
the orchestrator assigns (`fast` or `final`). Autonomously loop:
review → fix → verify → re-review until the reviewer reports `zero_findings` for that assigned
round_type. Then print your final status line and exit (the orchestrator decides whether to
escalate `fast` → `final`, run another scope, or stop).

## Contract

### Input (from orchestrator prompt appended by the wrapper)

- Track ID and scope name
- Briefing file path (`tmp/reviewer-runtime/briefing-{scope}.md`)
- Round type (`fast` or `final`) — single value, fixed for the agent's lifetime
- Reviewer model name for that round
- Scope file list (files this agent is allowed to modify)
- Reviewer invocation command

### Output (structured status — final line of your output)

Print **exactly** one of the following as the last line of stdout before exiting:

```
REVIEW_FIX_STATUS: completed
REVIEW_FIX_STATUS: blocked_cross_scope
REVIEW_FIX_STATUS: failed
```

- `REVIEW_FIX_STATUS: completed` — the assigned round_type returned `zero_findings`
  (verified via canonical API per step 2.5).
- `REVIEW_FIX_STATUS: blocked_cross_scope` — a fix requires modifying files outside this
  agent's scope. Include the list of out-of-scope files needed in your output before the
  status line.
- `REVIEW_FIX_STATUS: failed` — unrecoverable error (CI failure, reviewer crash, etc.).
  Include error details in your output before the status line.

### Scope Ownership (CRITICAL)

- This agent may ONLY modify files within its assigned scope (e.g., `libs/domain/**` for
  the domain scope). See `.harness/config/review-scope.json` for group definitions.
- If a finding requires changes to files outside the scope, do NOT modify them.
  Print the out-of-scope file list, then print `REVIEW_FIX_STATUS: blocked_cross_scope`
  so the orchestrator can re-partition.
- Cross-scope edits are fail-closed: silent out-of-scope modifications are prohibited.

## Scope-specific severity policy

If the main briefing contains a `## Scope-specific severity policy` section,
you MUST read the file listed there **before starting the review**. That file defines
which finding categories to report and which to skip for this scope. Applying the wrong
severity filter is the primary cause of over-long review loops.

To read the policy file:
```sh
cat <policy-file-path>
```

Do not skip this step even if the briefing path appears to be a known file. Always read
it fresh — the policy file may have been updated since the last review session.

## Workflow

**Always invoke review via `cargo make track-local-review` (never `bin/sotp review codex-local`
directly).** The cargo-make wrapper chains `track-sync-views` before the review so the scope
hash is computed against the up-to-date rendered views (`plan.md`, `contract-map.md`,
`<layer>-types.md`). Calling the inner `bin/sotp` form directly skips sync-views.

**Read prior-round findings via `cargo make track-review-results`, never by opening
`review.json` directly.**

- Latest fast-round findings only:
  `cargo make track-review-results -- --track-id {track-id} --scope {scope} --round-type fast --limit 1`
- Latest final-round findings only:
  `cargo make track-review-results -- --track-id {track-id} --scope {scope} --round-type final --limit 1`

### Step 1: Review

Run the reviewer using the invocation command from the orchestrator prompt:
```sh
cargo make track-local-review -- --model {reviewer_model} --round-type {round_type} --group {scope} --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md
```

### Step 2: Parse verdict

Read the verdict from command output.
- `zero_findings` → proceed to step 2.5 (verify via canonical API).
- `findings_remain` → proceed to fix phase.
- Error → print `REVIEW_FIX_STATUS: failed` and exit.

### Step 2.5: Verify via canonical API (mandatory before reporting `completed`)

```sh
cargo make track-review-results -- --track-id {track-id} --scope {scope} --round-type {round_type} --limit 1
```

`--limit 1` prints the most recent round entry for the assigned round_type as a findings
block below the state-line.

**State-line vs findings block (read this carefully).** The state-line suffix
(`[+] approved` / `[-] required (...)`) reflects **merge-gate readiness for the scope**,
which combines `fast verdict` + `final verdict` + `hash freshness`. It is NOT a per-round
verdict. The findings block (`findings: zero_findings` / `findings: ...`) below the
state-line IS the authoritative signal for the assigned round_type.

For `round_type == fast`, the state-line can read `[-] required (stale hash)` even when
this fast round is `zero_findings`, because the gate also evaluates the *final* round
(older or absent in fast-only mode). That gate-level state is the orchestrator's concern,
not this agent's — do not re-loop on it.

- **`round_type == fast`** — decide from the findings block only:
  - findings block shows `findings: zero_findings` → print `REVIEW_FIX_STATUS: completed` and exit.
  - findings block lists findings → re-loop into the fix phase.
  - No matching entry for the assigned round_type (empty output below state-line) → re-loop.
- **`round_type == final`** — state-line and findings block should agree:
  - State-line shows `[+]` / `approved` AND findings block shows `findings: zero_findings`
    → print `REVIEW_FIX_STATUS: completed` and exit.
  - State-line shows `[-]` / `required` OR findings block lists findings → re-loop.
  - No matching entry for the assigned round_type (empty output below state-line) → re-loop.

### Step 3: Fix phase

- Verify each finding's factual claims by reading the relevant source files before acting:
  ```sh
  cat <file>
  grep -n '<pattern>' <file>
  rg '<pattern>' <path>
  ```
- To recall previous-round findings without re-running the reviewer:
  ```sh
  cargo make track-review-results -- --track-id {track-id} --scope {scope} --round-type {round_type} --limit N
  ```
  Keep N small (1–3) to avoid context bloat.
- P3 findings from pre-existing unchanged code: note but do not fix.
- P0/P1/P2: implement the fix within scope boundaries.
- Apply fixes using your editor (apply patch or direct file editing).
- If a fix requires out-of-scope files: print the out-of-scope file list, then
  `REVIEW_FIX_STATUS: blocked_cross_scope` and exit.
- Run `cargo make ci-rust` to verify fixes compile.
- **Cross-doc hash sync (mandatory after editing `spec.json` or `impl-plan.json`)**: spec /
  impl-plan anchor text changes cause `catalogue (<layer>-types.json)` `spec_refs[].hash`
  to go stale. Run:
  ```sh
  cargo make verify-plan-artifact-refs
  ```
  This is NOT included in `cargo make ci-rust`; call it explicitly when you edited
  `spec.json` or `impl-plan.json` in this round.
  - `[OK] All checks passed.` → proceed to step 4.
  - `SpecRef hash mismatch ...` → catalogue hash sync is needed. This requires the
    `type-designer` capability; you cannot fix `<layer>-types.json` directly.
    Return `REVIEW_FIX_STATUS: failed` with the full mismatch error and the list of
    anchors you edited so the orchestrator can delegate to type-designer.
  - Other errors → return `REVIEW_FIX_STATUS: failed` with the full verifier output.

### Step 4: Re-review

Run the reviewer again with updated briefing (include previous findings and fixes applied).
Go to step 2.

## Architecture Guard

Before modifying any file, verify it belongs to the correct architecture layer:
```sh
cat knowledge/conventions/impl-delegation-arch-guard.md
```

- Domain types stay in `libs/domain/`
- Infrastructure adapters stay in `libs/infrastructure/`
- CLI composition stays in `apps/cli/`
- Do not move types between layers without explicit ADR authorization.

## Rules

- Use `cat` / `grep` / `rg` for file inspection — do not use Claude-specific tools
- Do not run `git add`, `git commit`, or `git push`
- Do not read or modify `review.json` directly
- Do not read files under `private/` or `config/secrets/`
- Between review rounds, always run `cargo make ci-rust`
- The final line you print MUST be one of the three `REVIEW_FIX_STATUS:` forms above
