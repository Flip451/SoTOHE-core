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
| **infra-domain** | Error type design, trait signatures, architecture direction | `libs/infrastructure/**`, `libs/domain/**` |
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
(the Agent tool with `run_in_background: true`).

**Agent tool usage constraint**: When spawning agents, instruct them to use the `Read` tool
(not `Bash(grep ...)`, `Bash(cat ...)`, etc.) for reading output files and extracting results.
Commands in the `FORBIDDEN_ALLOW` list trigger permission prompts and block automation.

**When the provider has a CLI tool** (e.g., Codex CLI — the default profile):

Use `{fast_model}` for iterative rounds and `{model}` for the final confirmation round (see Model escalation strategy).

```
Agent 1: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-infra-domain.md
Agent 2: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-usecase.md
Agent 3: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-cli.md
Agent 4: cargo make track-local-review -- --model {fast_model} --briefing-file tmp/reviewer-runtime/briefing-other.md
```

For the **final confirmation round**, replace `{fast_model}` with `{model}` in the commands above.

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
1. Assess severity (P1 / P2 / P3).
2. P3 findings from pre-existing unchanged code: note but do not fix.
3. P1 and P2: implement the fix.
4. If the finding requires a new test, add it.
5. Run `cargo make ci` (or `cargo make ci-rust` for fast inner loop) to verify fixes compile and pass.

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

### Loop guard

- Maximum 5 rounds with the fast reviewer. If findings persist after 5 fast rounds, stop and report remaining issues to the user for manual decision.
- The final full-model confirmation round does not count toward the loop guard limit.
- Between rounds, always run `cargo make ci-rust` to ensure fixes don't break the build.

## Step 4: Final validation

After the reviewer reports zero findings:
1. Run `cargo make ci` (full CI, not just ci-rust) to confirm all checks pass.
2. If CI fails, fix and re-run (this does not reset the review loop counter).

## Behavior

After execution, summarize:
1. Total review rounds completed
2. Reviewer groups used and parallelization (e.g., "3 parallel groups: infra-domain, usecase, cli")
3. Findings per round (count and severity breakdown, grouped by layer)
4. Fixes applied (with file references)
5. Final CI result
6. Merge/commit readiness (Ready / Not ready with reason)
7. Recommended next command (`/track:commit <message>`)
