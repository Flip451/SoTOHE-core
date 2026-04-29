---
description: Run review for current track implementation.
---

Canonical command for review in the track workflow.

Implements the review → fix → review cycle mandated by `CLAUDE.md`:
> Before committing code changes, run the `reviewer` capability review cycle
> (review → fix → review → ... → no findings). Do not commit until the reviewer
> reports zero findings.

Arguments: none. The current branch (`track/<id>` or `plan/<id>`) determines the review target.

## Step 0: Gather context

- Extract the track id from the current git branch (`track/<id>` or `plan/<id>`).
  If the branch matches neither pattern, stop and instruct the user to switch first.
- Read the current track's `spec.md`, `plan.md`, and `metadata.json`.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `spec.md` (or `plan.md` for legacy tracks without `spec.json`).
- For exact type signatures, trait definitions, module trees, and Mermaid diagrams, use `## Canonical Blocks` in `plan.md` and `knowledge/DESIGN.md` as the source of truth when reviewing implementation correctness.
- If the selected track is branchless planning-only (`status=planned`, `branch=null`), limit review scope to planning artifacts only. Allowed diff is:
  - `track/items/<id>/`
  - `track/registry.md`
  - `track/tech-stack.md`
  - `knowledge/DESIGN.md`
- If changed files exceed that allowlist, stop and instruct the user to run `/track:activate <track-id>` before code-bearing review.

## Step 1: Resolve reviewer model

- Read `.harness/config/agent-profiles.json`.
- Look up `capabilities.reviewer.provider` → `{reviewer_provider}` (e.g., `codex`).
- Look up `capabilities.reviewer.model` → `{model}` (full model).
- Look up `capabilities.reviewer.fast_model` → `{fast_model}` (iterative rounds; falls back to `model` if absent).
- Invoke via `cargo make track-local-review` with explicit `--model` flag.
- **Note**: The `claude` provider path (`claude-heavy` profile) is not yet supported.

## Step 2: Prepare review briefings (parallel observation split)

Partition changed files into **observation groups** by architecture layer. Each group gets its own
focused briefing and reviewer invocation. All groups run **in parallel** via Agent Teams.

### 2a. Determine review scope via `track-review-results`

Run `cargo make track-review-results -- --track-id {track-id}` to get the authoritative
per-scope review state. This command computes diff-based scope hashes and reports which
groups need review:

```
cargo make track-review-results -- --track-id {track-id}
```

Output example:
```
  [✓] cli: approved
  [-] domain: required (not started)
  [.] harness-policy: not required (empty)
  [-] other: required (stale hash)
  [-] plan-artifacts: required (stale hash)
  [.] usecase: not required (empty)
```

Current named groups (v2 `track/review-scope.json`): `domain`, `usecase`,
`infrastructure`, `cli`, `plan-artifacts`, `harness-policy`. The implicit
`other` group collects everything not matched by a named group.

Use this output to determine which groups need reviewer invocation:
- `required (not started)` — needs review (new changes, no review yet)
- `required (stale hash)` — needs review (files changed since last review)
- `required (findings remain)` — needs review (previous round had findings; re-review required)
- `approved` — already reviewed and up-to-date, skip
- `not required (empty)` — no changed files in this group, skip

**Do NOT manually classify files into groups** — `track-review-results` handles partition,
hash computation, and approval state tracking. Only invoke reviewers for groups that
report `required`.

### 2b. Build per-group briefing

For each group reporting `required` in Step 2a, build a briefing file at `tmp/reviewer-runtime/briefing-{group}.md`.
The per-group scope file list is automatically injected by `cargo make track-local-review` (via `CodexReviewer::build_full_prompt`) — the briefing only needs design intent and review checklist.

**Do NOT hand-author the `## Scope-specific severity policy` section.** If the
group has a `briefing_file` configured in `track/review-scope.json` (currently
`plan-artifacts` — `track/review-prompts/plan-artifacts.md`), the CLI composer
automatically appends a `## Scope-specific severity policy` section pointing
the reviewer at that file. Duplicating the policy in the per-group briefing is
not just redundant — it creates drift when the severity policy md is updated
without touching this template. Responsibility split:

- `track/review-prompts/<scope>.md` owns severity policy wording (edited as a
  standalone file; versioned with the config)
- `tmp/reviewer-runtime/briefing-{scope}.md` owns per-round design intent and
  review checklist (authored here, per-round)
- `sotp review codex-local` owns the wiring between the two (automatic)

```markdown
# Review Briefing: {track-id} — {group} layer

## Design Intent
{3-5 bullet points from spec.md / plan.md}

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

Launch one `review-fix-lead` agent per group reporting `required`, **in parallel** using Agent Teams
(the Agent tool with `run_in_background: true` and `subagent_type: "review-fix-lead"`).

Each `review-fix-lead` agent autonomously handles the full review → fix → re-review loop
for its scope. This eliminates the intermediate `codex-reviewer` → orchestrator aggregation
→ `review-fix-lead` handoff. The agent loops until fast-model `zero_findings`, then reports
`completed` back to the orchestrator.

**When the provider has a CLI tool** (e.g., Codex CLI — the default profile):

Use `{fast_model}` for iterative rounds. Auto-record is always on (v2). Verdicts are
written directly to `review.json` after each Codex run. Parallel per-scope reviews are
safe: each scope's hash is computed from its own files only.

To compute per-scope file lists for the agent prompt:
- **Named groups**: changed files matching the group's glob patterns from `track/review-scope.json`.
- **`other` group**: full changed file list (`git diff {base}...HEAD --name-only` + staged + unstaged + untracked, where `{base}` = `.commit_hash` or `main`) minus files matched by named-group patterns, `review_operational` patterns, and `other_track` patterns (all from `track/review-scope.json`). Exception: current track artifacts (`track/items/{track-id}/**`) are NOT excluded by `other_track`.

```
Agent prompt for each scope:
  You are a review-fix-lead for the {scope} scope of track {track-id}.
  Briefing: tmp/reviewer-runtime/briefing-{scope}.md
  Fast model: {fast_model}
  Scope files (files this agent is allowed to modify):
  {per-scope file list computed above}

  Run the fix+review loop as defined in .claude/agents/review-fix-lead.md.
  Use: cargo make track-local-review -- --model {fast_model} --round-type fast --group {scope} --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md

  Report your final status: completed / blocked_cross_scope / failed.
```

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

Resolve models from `.harness/config/agent-profiles.json` using the `reviewer` capability:
- `{fast_model}`: `capabilities.reviewer.fast_model`, then `capabilities.reviewer.model`. If neither exists, skip `--model`.
- `{model}`: `capabilities.reviewer.model`. If not set, skip `--model`.

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
