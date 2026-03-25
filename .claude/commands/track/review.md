---
description: Run review for current track implementation.
---

Canonical command for review in the track workflow.

Implements the review → fix → review cycle mandated by `CLAUDE.md`:
> Before committing code changes, run the `reviewer` capability review cycle
> (review → fix → review → ... → no findings). Do not commit until the reviewer
> reports zero findings.

Arguments:
- Use `$ARGUMENTS` as optional review scope (files/modules/concerns).
- On a non-track branch, when reviewing a planning-only artifact, require an explicit
  track-id selector in `$ARGUMENTS` and treat the remaining text as optional scope notes.
  Do not auto-detect a branchless planning-only track by timestamp alone.

## Step 0: Gather context

- Resolve the current track in this order:
  1. If the current git branch matches `track/<id>`, use that track.
  2. Otherwise, if `$ARGUMENTS` starts with an explicit existing `<track-id>`, use `track/items/<track-id>`.
  3. Otherwise, use the latest materialized active track (non-archived, non-done, `branch != null`).
- Do not auto-select a branchless planning-only track on a non-track branch.
- Read the current track's `spec.md`, `plan.md`, and `metadata.json`.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `spec.md` (or `plan.md` for legacy tracks without `spec.json`).
- For exact type signatures, trait definitions, module trees, and Mermaid diagrams, use `## Canonical Blocks` in `plan.md` and `.claude/docs/DESIGN.md` as the source of truth when reviewing implementation correctness.
- Use any auto-injected external guide summaries from `docs/external-guides.json` before opening cached raw guide documents.
- If `$ARGUMENTS` is provided, scope the review to the specified files/modules/concerns.
- If the selected track is branchless planning-only (`status=planned`, `branch=null`), limit review scope to planning artifacts only. Allowed diff is:
  - `track/items/<id>/`
  - `track/registry.md`
  - `track/tech-stack.md`
  - `.claude/docs/DESIGN.md`
- If changed files exceed that allowlist, stop and instruct the user to run `/track:activate <track-id>` before code-bearing review.

## Step 1: Resolve reviewer provider

- Read `.claude/agent-profiles.json`.
- Look up `profiles.<active_profile>.reviewer` to determine the provider (e.g., `codex`).
- Resolve `{model}`:
  1. Check `profiles.<active_profile>.provider_model_overrides.<provider>`.
  2. Fall back to `providers.<provider>.default_model`.
  3. If neither is set (e.g., `claude` provider has no `default_model`), `{model}` is not needed — skip the `--model` flag.
- When the resolved provider has a CLI tool (e.g., Codex CLI), invoke via `cargo make track-local-review` (external subprocess).
- When the resolved provider is `claude` (e.g., `claude-heavy` profile), invoke via Claude Code subagent with `subagent_type: "Explore"` using the same briefing files and JSON verdict format. No `--model` flag is needed. Do not perform inline review in the main conversation context.

## Step 2: Prepare review briefings (parallel observation split)

Partition changed files into **observation groups** by architecture layer. Each group gets its own
focused briefing and reviewer invocation. All groups run **in parallel** via Agent Teams.

### 2a. Classify changed files into groups

Use `git diff main --name-only` (or equivalent) to get the full changed file list. Assign each
file to exactly one observation group:

| Group | Scope | Files matching |
|-------|-------|----------------|
| **domain** | Type design, invariants, business rules, trait signatures (ports) | `libs/domain/**` |
| **infra** | I/O correctness, parsing, adapters, external dependencies. Include related domain trait signatures in briefing as context. | `libs/infrastructure/**` |
| **usecase** | Workflow logic, error propagation, functional correctness | `libs/usecase/**` |
| **cli** | CLI error handling, exit codes, user-facing messages, functional regressions | `apps/cli/**` |
| **other** | Workflow docs, skill definitions, track artifacts, scripts, config | Everything else (`.claude/**`, `track/**`, `DEVELOPER_AI_WORKFLOW.md`, `scripts/**`, `Cargo.*`, etc.) |

If a group has zero changed files, skip it (do not invoke a reviewer for empty scope).

If the total changed files are small (≤ 5 files across all groups), collapse into a single
reviewer invocation instead of splitting — parallel overhead is not worthwhile.

### 2b. Build per-group briefing

For each non-empty group, build a briefing file at `tmp/reviewer-runtime/briefing-{group}.md`:

```markdown
# Review Briefing: {track-id} — {group} layer

## Design Intent
{3-5 bullet points from spec.md / plan.md}

## Changed Files (this group only)
{file list for this group}

## Review Checklist
- Logic errors, edge cases, race conditions
- No panics in library code (no unwrap/expect outside #[cfg(test)])
- Proper error propagation (thiserror, #[source], #[from])
- Architecture layer dependency direction (domain ← usecase ← infrastructure ← cli)
- Idiomatic Rust (naming, patterns)
- Test coverage gaps
- Security (input validation, error information leakage)

## Known Accepted Deviations
{any scope-specific notes, e.g. "lock.rs and hook.rs are intentionally unchanged"}

Report findings as JSON:
{"verdict":"zero_findings","findings":[]}
or
{"verdict":"findings_remain","findings":[{"message":"...","severity":"P1","file":"path","line":123}]}
DO NOT report findings about test code using unwrap/expect — that is allowed.
DO NOT report findings about unchanged pre-existing code.
```

### 2c. Invoke reviewers in parallel

Launch one reviewer per non-empty group, **in parallel** using Agent Teams
(the Agent tool with `run_in_background: true` and `subagent_type: "codex-reviewer"`).

The `codex-reviewer` agent type restricts available tools to Bash + Read + Grep + Glob,
and its system prompt enforces: run the command exactly as given, no `$?`/`2>&1`/shell
expansion, no build commands, use Read (not Bash) for reading files.

**When the provider has a CLI tool** (e.g., Codex CLI — the default profile):

Use `{fast_model}` for iterative rounds and `{model}` for the final confirmation round (see Model escalation strategy).

When `--auto-record` is passed, the reviewer wrapper calls `record-round` internally after
verdict extraction, applying diff scope filtering (RVW-11) and preventing verdict falsification
(RVW-10). This replaces the manual Step 2e. The `--diff-base` flag controls the base ref for
scope filtering (default: `main`).

```
Agent 1: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-domain.md --auto-record --track-id {track-id} --round-type {fast|final} --group domain --expected-groups {all-group-names} --diff-base main
Agent 2: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-infra.md --auto-record --track-id {track-id} --round-type {fast|final} --group infra --expected-groups {all-group-names} --diff-base main
Agent 3: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-usecase.md --auto-record --track-id {track-id} --round-type {fast|final} --group usecase --expected-groups {all-group-names} --diff-base main
Agent 4: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-cli.md --auto-record --track-id {track-id} --round-type {fast|final} --group cli --expected-groups {all-group-names} --diff-base main
Agent 5: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-other.md --auto-record --track-id {track-id} --round-type {fast|final} --group other --expected-groups {all-group-names} --diff-base main
```

For the **final confirmation round**, replace `{fast_model}` with `{model}` and `--round-type fast` with `--round-type final` in the commands above.

**When the provider is `claude`** (e.g., `claude-heavy` profile):

Launch one Claude Code subagent per group with `subagent_type: "Explore"`.
Each subagent reads its briefing file and returns a JSON verdict in the same format.

Wait for all agents to complete.

### 2d. Aggregate verdicts

Collect the JSON verdict from each reviewer agent. Apply fail-closed aggregation:

- If **any** reviewer reports `findings_remain`: overall verdict is `findings_remain`.
  Merge all findings arrays into a single list.
- If **any** reviewer fails (timeout / process_failed / last_message_missing): report the
  failure and treat overall verdict as `findings_remain` (fail-closed).
- Only if **all** reviewers report `zero_findings`: overall verdict is `zero_findings`.

The wrapper passes a machine-readable `--output-schema` automatically. The final reviewer
message must be a single JSON object, and the wrapper additionally rejects semantically
inconsistent payloads fail-closed:

```json
{"verdict":"zero_findings","findings":[]}
```

or

```json
{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}
```

Every object field is required by the output schema. When a finding does not have a concrete
severity, file, or line, use `null` for that field instead of omitting it.
`zero_findings` must use an empty `findings` array, and `findings_remain` must include at least
one finding. The wrapper prints that final JSON payload as the last stdout line.

### 2e. Record round results

**When `--auto-record` is used** (recommended): Step 2e is handled automatically by the
reviewer wrapper. The wrapper applies diff scope filtering, extracts concerns, and calls
`record-round` internally. Exit code 3 signals escalation block. No manual intervention needed.

**Fallback (without `--auto-record`)**: If `--auto-record` is not used, manually persist
the result into `metadata.json` via `sotp review record-round`:

For **each non-empty group**, run (when the `<= 5 files` collapse rule was applied and a
single reviewer invocation was used, treat all changed files as a single group named `all`
and pass `--expected-groups all`):

```bash
bin/sotp review record-round \
  --track-id {track-id} \
  --round-type {fast|final} \
  --group {group-name} \
  --verdict '{aggregated verdict JSON for this group}' \
  --expected-groups {comma-separated list of all non-empty group names} \
  --concerns {comma-separated concern slugs extracted from findings, empty for zero_findings} \
  --items-dir track/items
```

**Concern extraction**: For `findings_remain` verdicts, extract concern slugs from findings
using `findings_to_concerns()` 3-stage fallback:
1. Use the finding's `category` field if present (e.g., `"security"`, `"logic_error"`).
2. If no category, derive from the `file` field (e.g., `libs/domain/src/review.rs` → `domain.review`).
3. If neither is available, use `"other"`.
4. Deduplicate concerns before passing to `--concerns`.

For `zero_findings` verdicts, **omit the `--concerns` flag entirely** (the CLI default is empty).
Do NOT pass `--concerns ""` — the empty-string argument breaks Claude Code permission matching
for `cargo make track-record-round`.

**Error handling**:
- If `record-round` (or `--auto-record`) returns exit code 3 (`EscalationActive`): stop the
  review loop and report the escalation block to the user with the required resolution steps.
- If `record-round` fails with a stale-hash error (stderr contains "code hash mismatch"):
  the review state has been invalidated. Stop and re-run the review from Round 1 —
  proceeding would leave the review state inconsistent with the current code.
- If `record-round` fails with any other error: report but continue (fail-open for recording;
  the review cycle itself is not blocked by a recording failure).

## Step 3: Review → Fix → Review loop

### Round 1 (initial review)

Execute the parallel review as described in Step 2.

Parse the aggregated verdict:
- If `zero_findings` and this was a fast-model round: proceed to the final confirmation round (see Model escalation strategy).
- If `zero_findings` and this was the full-model confirmation round: proceed to Step 4 (done).
- If `findings_remain`: read the merged findings list and proceed to fix phase.
- If any reviewer execution failed: stop and report the failure before continuing.

### Fix phase

For each finding:

1. **Verify factual claims before acting.** If the finding asserts a fact about the codebase
   (e.g., "function X returns Y", "the runtime emits file Z", "this trait requires W"),
   use `Grep` / `Read` to confirm the claim is true. Reviewer models can hallucinate
   implementation details. Do NOT revert correct code based on an unverified claim.
2. Assess severity (P1 / P2 / P3).
3. P3 findings from pre-existing unchanged code: note but do not fix.
4. P1 and P2: implement the fix.
5. If the finding requires a new test, add it.
6. Run `cargo make ci` (or `cargo make ci-rust` for fast inner loop) to verify fixes compile and pass.

### Round N (fix verification)

After fixes are applied, invoke the reviewer again using the **same parallel pattern** from
Step 2, but update each briefing to include:

```markdown
## Previous Findings (Round N-1)
{finding summary per group}

## Fixes Applied
{fix description, test names if any}

Verify the fixes. Report any remaining bugs or new issues.
```

Parse the aggregated output:
- If `zero_findings` and this was a fast-model round: proceed to the final confirmation round (see Model escalation strategy).
- If `zero_findings` and this was the full-model confirmation round: proceed to Step 4 (done).
- If `findings_remain`: use the merged findings, then repeat fix phase → Round N+1.
- Otherwise, stop and report the reviewer execution failure.

### Model escalation strategy

Use the reviewer provider's `fast_model` for iterative fix-verify rounds and escalate to `default_model` for final confirmation.

Resolve models from `.claude/agent-profiles.json` using the `reviewer` capability:
- `{fast_model}`: `provider_model_overrides` for reviewer, then `providers.<reviewer_provider>.fast_model`, then `default_model`. If none exist, skip `--model`.
- `{model}`: `provider_model_overrides`, then `providers.<reviewer_provider>.default_model`. If none exist, skip `--model`.

Execution:
- **Iterative rounds (up to 5)**: Use the `reviewer` capability with `{fast_model}` for rapid feedback.
- **Final round (confirmation)**: When the fast model reports `zero_findings`, run one more round with `{model}` to catch deeper design issues.
- If the full model also reports `zero_findings`: proceed to Step 4.
- If the full model finds new issues: fix and return to the fast model loop.

### Early-completion pipelining

When groups are reviewed in parallel and some groups complete with `zero_findings` while
others have `findings_remain`:

- **Without `--auto-record`**: start fixes immediately for completed groups without waiting
  for others. Launch the next review round per-group as fixes are ready.
- **With `--auto-record`**: wait for ALL groups in the current round to complete recording
  before starting any fixes. Each `record-round` captures a code hash; if fixes change
  the tree between recordings, later groups will hit stale-code-hash and invalidate
  review state. Only begin fixes after all groups have recorded.

### Loop guard

- No fixed round limit for the fast reviewer. Continue fix → review cycles as long as progress is being made.
- If the same finding recurs 3 times with no code change addressing it, stop and report to the user.
- The final full-model confirmation round does not count toward the loop guard.
- Between rounds, always run `cargo make ci-rust` to ensure fixes don't break the build.

## Step 4: Final validation

After the reviewer reports zero findings:
1. Run `cargo make ci` (full CI, not just ci-rust) to confirm all checks pass.
2. If CI fails, fix and re-run (this does not reset the review loop counter).
3. **Review state guard verification (mandatory)**: Run `bin/sotp review check-approved --track-id {track-id} --items-dir track/items`
   to confirm the review state is `Approved`. This is the authoritative readiness check —
   do NOT declare "Ready" based solely on reviewer stdout verdicts.
   - If `check-approved` returns exit code 0: review is complete. Proceed to "Ready".
   - If `check-approved` returns non-zero: review is NOT complete. Diagnose the cause
     (missing `record-round` calls, stale code hash, escalation block) and resolve before
     declaring readiness.
   - This step catches cases where `record-round` was accidentally skipped or where
     the code hash was invalidated between the last review and CI.

## Behavior

After execution, summarize:
1. Total review rounds completed
2. Reviewer groups used and parallelization (e.g., "3 parallel groups: infra-domain, usecase, cli")
3. Findings per round (count and severity breakdown, grouped by layer)
4. Fixes applied (with file references)
5. Final CI result
6. Review state guard (`check-approved`) result
7. Merge/commit readiness (Ready / Not ready with reason)
8. Recommended next command (`/track:commit <message>`)
