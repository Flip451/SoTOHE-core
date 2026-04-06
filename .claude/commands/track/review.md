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
  1. Check `profiles.<active_profile>.provider_model_overrides.<provider>`.
  2. Fall back to `providers.<provider>.default_model`.
  3. If neither is set (e.g., `claude` provider has no `default_model`), `{model}` is not needed — skip the `--model` flag.
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

If the total changed files are small (≤ 5 files) AND all belong to **a single group**,
collapse into a single reviewer invocation instead of splitting — parallel overhead is
not worthwhile. Use the actual group name from the partition (e.g., `other` for planning
artifacts). Do NOT use a synthetic group name like `all` — `record-round` only recognizes
group names produced by `partition()`: named groups from the active policy
(base `track/review-scope.json` or per-track `review-groups.json` override)
plus the implicit `other` fallback group.

If files span **multiple groups**, use the normal parallel pattern even for ≤ 5 files.
Auto-record records exactly one scope per invocation, so multi-scope collapsed reviews
would leave some groups unrecorded.

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

Auto-record is always on (v2). Verdicts are written directly to `review.json` after each
Codex run. The scope file list is automatically injected into the prompt so each reviewer
focuses on the correct files. Parallel per-scope reviews are safe: each scope's hash is
computed from its own files only.

```
Agent 1: cargo make track-local-review -- --model {fast_model} --round-type {fast|final} --group domain --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-domain.md
Agent 2: cargo make track-local-review -- --model {fast_model} --round-type {fast|final} --group infrastructure --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-infrastructure.md
Agent 3: cargo make track-local-review -- --model {fast_model} --round-type {fast|final} --group usecase --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-usecase.md
Agent 4: cargo make track-local-review -- --model {fast_model} --round-type {fast|final} --group cli --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-cli.md
Agent 5: cargo make track-local-review -- --model {fast_model} --round-type {fast|final} --group harness-policy --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-harness-policy.md
Agent 6: cargo make track-local-review -- --model {fast_model} --round-type {fast|final} --group other --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-other.md
```

For the **final confirmation round**, replace `{fast_model}` with `{model}`.

**When the provider is `claude`** (e.g., `claude-heavy` profile):

> See Step 1 **Provider support matrix** — claude auto-record is not yet implemented.

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

**Channel-scoped fail-closed contract**: A trusted `zero_findings` verdict requires ALL of
the following channels to succeed. Partial success on any single channel is not sufficient:

| Channel | What constitutes success | Failure mode |
|---------|------------------------|--------------|
| stdout | Valid JSON verdict as the last line | Missing, malformed, or semantically inconsistent JSON |
| exit code | 0 (zero_findings) or 2 (findings_remain) | 1 (error), 3 (escalation), timeout |
| review.json | Verdict persisted by auto-record | Write failure, missing file, stale hash |

Controlling stdout alone while leaving stderr as an uncontrolled fallback is a known bypass
class. Do not treat stderr output as a verdict source.

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

**Auto-record (v2, Codex-only, always on)**: Step 2e is handled automatically by the
`bin/sotp review codex-local` wrapper. Verdicts are written to `review.json` after each
Codex invocation. Manual `record-round` has been removed to prevent verdict falsification.

> Claude subagent verdicts are **not auto-recorded**. See Step 1 **Provider support matrix**.

**Error handling**:
- If auto-record fails (exit 1, verdict not printed): the round must be retried.
  Unrecorded verdicts are not trusted.

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

**Note**: `cargo make add-all` (staging) is NOT required between review rounds.
The review hash (`rvw1:` prefix) is computed from worktree file contents, not the git index
(see `review_adapters.rs` / ADR §5). Staging is only needed before `/track:commit`.

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

**CRITICAL: fast model の zero_findings はレビュー完了の根拠にならない。**

fast model レビューの役割は **安価に高速に明白な間違いを検出すること** であり、実装品質や設計品質の保証ではない。
たとえ fast model が何度連続で zero_findings を返しても、それはレビュー完了を意味しない。
レビュー完了は **full model の zero_findings** によってのみ確認される。

fast model の zero_findings を根拠にレビュー終了を提案してはならない。
fast model の zero_findings を根拠に「実質完了」「レビューは十分」と判断してはならない。

Resolve models from `.claude/agent-profiles.json` using the `reviewer` capability:
- `{fast_model}`: `provider_model_overrides` for reviewer, then `providers.<reviewer_provider>.fast_model`, then `default_model`. If none exist, skip `--model`.
- `{model}`: `provider_model_overrides`, then `providers.<reviewer_provider>.default_model`. If none exist, skip `--model`.

Execution:
- **Iterative rounds**: Use the `reviewer` capability with `{fast_model}` for rapid feedback. Purpose: catch obvious errors cheaply before consuming full model budget.
- **Final round (confirmation)**: When the fast model reports `zero_findings`, run one more round with `{model}` for a thorough, comprehensive review. The full model may find any category of issue the fast model missed (logic errors, test gaps, security concerns, etc.) — it is not limited to design-level findings.
- If the full model also reports `zero_findings`: proceed to Step 4. **Only this constitutes review completion.**
- If the full model finds new issues: fix and return to the fast model loop.

### Early-completion pipelining

When groups are reviewed in parallel and some groups complete with `zero_findings` while
others have `findings_remain`:

- **Parallel fixes** (v2): start fixes immediately for completed scopes
  without waiting for others. `group_scope_hash` is computed per-group from that group's
  scope files only, so modifying files in one group's scope does not affect another
  group's hash. Launch the next review round per-group as fixes are ready.
  Avoid modifying files that belong to a still-running group's scope.

### Loop guard

- Soft round guideline: if fast reviewer exceeds **5 rounds**, consider whether splitting the remaining work into smaller tasks would be more effective. Continue beyond 5 rounds if each round makes clear progress, but be alert to diminishing returns on large diffs.
- If the same finding recurs 3 times with no code change addressing it, stop and report to the user.
- The final full-model confirmation round does not count toward the loop guard.
- Between rounds, always run `cargo make ci-rust` to ensure fixes don't break the build.

## Step 4: Final validation

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
