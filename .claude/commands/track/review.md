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
- For exact type signatures, trait definitions, module trees, and Mermaid diagrams, use `## Canonical Blocks` in `plan.md` and `knowledge/DESIGN.md` as the source of truth when reviewing implementation correctness.
- Use any auto-injected external guide summaries from `knowledge/external/guides.json` before opening cached raw guide documents.
- If `$ARGUMENTS` is provided, scope the review to the specified files/modules/concerns.
- If the selected track is branchless planning-only (`status=planned`, `branch=null`), limit review scope to planning artifacts only. Allowed diff is:
  - `track/items/<id>/`
  - `track/registry.md`
  - `track/tech-stack.md`
  - `knowledge/DESIGN.md`
- If changed files exceed that allowlist, stop and instruct the user to run `/track:activate <track-id>` before code-bearing review.

## Step 1: Resolve reviewer provider

- Read `.claude/agent-profiles.json`.
- Look up `profiles.<active_profile>.reviewer` to determine the provider (e.g., `codex`).
- Resolve `{model}`:
  1. Look up `providers.<provider>.default_model`.
  2. If not set, `{model}` is not needed — skip the `--model` flag.
- When the resolved provider has a CLI tool (e.g., Codex CLI), invoke via `cargo make track-local-review` (external subprocess).
- When the resolved provider is `claude` (e.g., `claude-heavy` profile), invoke via Claude Code subagent with `subagent_type: "Explore"` using the same briefing files and JSON verdict format. No `--model` flag is needed. Do not perform inline review in the main conversation context.

### Provider support matrix

| Provider | Auto-record to review.json | `check-approved` | Notes |
|----------|---------------------------|-------------------|-------|
| `codex` (default) | Yes (built into `bin/sotp review codex-local`) | Satisfied via recorded verdicts | Recommended for all tracks |
| `claude` (`claude-heavy`) | **No** — verdicts are not persisted | Passes only via NotStarted bypass (review.json absent + all scopes NotStarted) | Review evidence exists only in conversation context, not in review.json |

**Limitation**: With `claude-heavy`, `check-approved` passes via the NotStarted bypass because
verdicts are never written to `review.json`. This means Step 4 does not verify actual review
coverage — it only confirms that no local review was started. For auditable review evidence,
use the default Codex profile.

## Step 2: Prepare review briefings (parallel observation split)

Partition changed files into **observation groups** by architecture layer. Each group gets its own
focused briefing and reviewer invocation. All groups run **in parallel** via Agent Teams.

### 2a. Classify changed files into groups

Get the full changed file list including staged, unstaged, and untracked files.
Use `{base}` as the diff base (default: `.commit_hash` → fallback to `main`):
- `git diff {base}...HEAD --name-only` for committed changes (merge-base diff)
- `git diff --cached --name-only` for staged-only changes
- `git diff --name-only` for unstaged worktree changes
- `git ls-files --others --exclude-standard` for untracked files (e.g., new track artifacts)
- Merge all lists and deduplicate.
- Note: `review_operational` files (e.g., `review.json`) are
  automatically excluded by the infrastructure before partition. In manual fallback mode,
  the orchestrator should exclude files matching `review_operational` patterns from
  `track/review-scope.json` before assigning groups.
- Assign each remaining file to exactly one observation group:

The authoritative group definitions are in `track/review-scope.json`. Per-track overrides
can be placed at `track/items/<track-id>/review-groups.json` — when present, its `groups`
object **replaces** the base groups entirely. Check for a per-track override before using
the base definitions. The table below is a summary of the base groups — if they diverge
from `review-scope.json`, the JSON file wins.

| Group | Scope | Files matching |
|-------|-------|----------------|
| **domain** | Type design, invariants, business rules, trait signatures (ports) | `libs/domain/**` |
| **infrastructure** | I/O correctness, parsing, adapters, external dependencies | `libs/infrastructure/**` |
| **usecase** | Workflow logic, error propagation, functional correctness | `libs/usecase/**` |
| **cli** | CLI error handling, exit codes, user-facing messages | `apps/**` |
| **harness-policy** | Workflow commands, rules, agent profiles, conventions | `.claude/commands/**`, `.claude/rules/**`, `.claude/agents/**`, `.claude/agent-profiles.json`, `.claude/settings*.json`, `.claude/permission-extensions.json`, `knowledge/conventions/**`, `AGENTS.md`, `CLAUDE.md` |
| **other** | Track artifacts, scripts, config, docs not covered above | Everything else (`track/**`, `scripts/**`, `Cargo.*`, etc.) |

If a group has zero changed files, skip it (do not invoke a reviewer for empty scope).

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
- Enum-first: variant-dependent data must use enum variants, not struct + runtime validation (see .claude/rules/04-coding-principles.md § Enum-first)
- Typestate: state transitions should use typestate pattern where feasible, not status field + runtime checks (see .claude/rules/04-coding-principles.md § Typestate)
- Test coverage gaps
- Security (input validation, error information leakage)

## Architecture Verification Checklist (see knowledge/conventions/impl-delegation-arch-guard.md)
- ADR/plan で指定された型が正しい層に配置されているか
- CLI が composition root パターンに従っているか（usecase 呼び出しのみ）
- usecase ロジックが CLI に漏れていないか
- NullXxx による usecase bypass がないか（status/check-approved 用途を除く）

## Known Accepted Deviations
{any scope-specific notes, e.g. "lock.rs and hook.rs are intentionally unchanged"}

Report findings as JSON:
{"verdict":"zero_findings","findings":[]}
or
{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}
Every object field is required by the output schema. When a finding does not have a concrete
severity, file, or line, use `null` for that field instead of omitting it.
DO NOT report findings about test code using unwrap/expect — that is allowed.
DO NOT report findings about unchanged pre-existing code.
```

### 2c. Invoke review-fix-lead agents in parallel

Launch one `review-fix-lead` agent per non-empty group, **in parallel** using Agent Teams
(the Agent tool with `run_in_background: true` and `subagent_type: "review-fix-lead"`).

Each `review-fix-lead` agent autonomously handles the full review → fix → re-review loop
for its scope. This eliminates the intermediate `codex-reviewer` → orchestrator aggregation
→ `review-fix-lead` handoff. The agent loops until fast-model `zero_findings`, then reports
`completed` back to the orchestrator.

**When the provider has a CLI tool** (e.g., Codex CLI — the default profile):

Use `{fast_model}` for iterative rounds. Auto-record is always on (v2). Verdicts are
written directly to `review.json` after each Codex run. Parallel per-scope reviews are
safe: each scope's hash is computed from its own files only.

```
Agent prompt for each scope:
  You are a review-fix-lead for the {scope} scope of track {track-id}.
  Briefing: tmp/reviewer-runtime/briefing-{scope}.md
  Fast model: {fast_model}
  Scope files: {file list from Step 2a}

  Run the fix+review loop as defined in .claude/agents/review-fix-lead.md.
  Use: cargo make track-local-review -- --model {fast_model} --round-type fast --group {scope} --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md

  Report your final status: completed / blocked_cross_scope / failed.
```

**When the provider is `claude`** (e.g., `claude-heavy` profile):

> See Step 1 **Provider support matrix** — claude auto-record is not yet implemented.

Use the same `review-fix-lead` agent contract, but instead of `cargo make track-local-review`,
the agent invokes a Claude Code subagent with `subagent_type: "Explore"` to perform reviews
using the same briefing file and JSON verdict format.

Wait for all agents to complete (or timeout).

### 2d. Collect agent statuses

Each review-fix-lead agent reports one of:
- `completed` — fast model `zero_findings` achieved
- `blocked_cross_scope` — fix requires changes in another scope
- `failed` — timeout, recurring findings, or execution error

**Fail-closed**: if an agent fails or times out, treat as `findings_remain`.

**Verdict JSON and auto-record** are handled internally by the review-fix-lead agent
and the `bin/sotp review codex-local` wrapper. The orchestrator does not parse verdict
JSON directly — it only acts on the agent's status report.

### 2e. Orchestrator actions per status

- `completed` → immediately launch full model confirmation for that scope (Step 2f)
- `blocked_cross_scope` → fix cross-scope dependencies in the orchestrator, then relaunch
- `failed` / timeout → immediately relaunch or report to user depending on cause

### 2f. Full model confirmation (per-scope, immediate)

When a scope's review-fix-lead reports `completed` (fast model `zero_findings`),
**immediately** launch a full model confirmation for that scope. Do not wait for other scopes.

Launch a `review-fix-lead` agent for that scope with `{model}` (full model) and
`--round-type final`. If the full model reports `zero_findings`: that scope is done.
If the full model finds new issues: the orchestrator updates the briefing for that scope
to include the full-model findings and previously applied fixes, then relaunches
review-fix-lead for that scope, choosing `{fast_model}` or `{model}` based on its
assessment of the findings.

A scope's review is complete only when **full model `zero_findings`** is recorded for it.
Step 3 begins when all scopes have achieved full model `zero_findings`.

### Model escalation strategy

**CRITICAL: fast model の zero_findings はレビュー完了の根拠にならない。**
レビュー完了は **full model の zero_findings** によってのみ確認される。

Resolve models from `.claude/agent-profiles.json` using the `reviewer` capability:
- `{fast_model}`: `providers.<reviewer_provider>.fast_model`, then `default_model`. If neither exists, skip `--model`.
- `{model}`: `providers.<reviewer_provider>.default_model`. If not set, skip `--model`.

### Per-scope independence (throughput-first)

Each scope's review lifecycle is fully independent. Scopes do not wait for each other.
`group_scope_hash` is computed per-group from that group's scope files only, so
modifications in one scope do not affect another scope's hash.

### Loop guard

- The review-fix-lead agent loops until `zero_findings`, `blocked_cross_scope`,
  `failed`, or Agent timeout (60 minutes/scope).
- If the same finding recurs 3 times with no code change addressing it, the agent should
  return `failed` with the recurring finding details.

> **Note**: v2 escalation (concern tracking, `EscalationActive`) is not yet implemented
> (RV2-06). The Agent timeout serves as the primary infinite-loop prevention mechanism.

## Step 3: Final validation

After the reviewer reports zero findings:
1. Run `cargo make ci` (full CI, not just ci-rust) to confirm all checks pass.
2. If CI fails, fix and re-run (this does not reset the review loop counter).
3. **Review state guard verification (mandatory)**: Run `cargo make track-check-approved -- --track-id {track-id}`
   to confirm the review state is `Approved`. This is the authoritative readiness check —
   do NOT declare "Ready" based solely on reviewer stdout verdicts.
   - If `check-approved` returns exit code 0: review is complete. Proceed to "Ready".
   - If `check-approved` returns non-zero: review is NOT complete. Diagnose the cause
     (stale code hash, auto-record failure) and resolve before declaring readiness.

**NotStarted bypass** (PR-based workflow): When `review.json` does not exist AND all required
scopes are in `NotStarted` state, `check-approved` treats this as a valid bypass and returns
exit code 0. This allows commits when only the PR-based review path (`/track:pr-review`) is
used without a preceding local review. Once any local review round has been recorded (i.e.,
`review.json` exists or any scope has progressed beyond `NotStarted`), the bypass is no longer
available and full approval is required.

## Behavior

After execution, summarize:
1. Total review rounds completed
2. Reviewer groups used and parallelization (e.g., "3 parallel groups: infrastructure, usecase, cli")
3. Findings per round (count and severity breakdown, grouped by layer)
4. Fixes applied (with file references)
5. Final CI result
6. Review state guard (`check-approved`) result
7. Merge/commit readiness (Ready / Not ready with reason)
8. Recommended next command (`/track:commit <message>`)
