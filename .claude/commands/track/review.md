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

## Step 0: Gather context

- Read the latest active track's `spec.md`, `plan.md`, and `metadata.json`.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `plan.md`.
- For exact type signatures, trait definitions, module trees, and Mermaid diagrams, use `## Canonical Blocks` in `plan.md` and `.claude/docs/DESIGN.md` as the source of truth when reviewing implementation correctness.
- Use any auto-injected external guide summaries from `docs/external-guides.json` before opening cached raw guide documents.
- If `$ARGUMENTS` is provided, scope the review to the specified files/modules/concerns.

## Step 1: Resolve reviewer provider

- Read `.claude/agent-profiles.json`.
- Look up `profiles.<active_profile>.reviewer` to determine the provider (e.g., `codex`).
- Read the provider's `default_model` to get `{model}`.
- If the reviewer is `claude`, perform the review inline (no subprocess). Skip to Step 2 using Claude Code's own analysis.

## Step 2: Prepare review briefing

Build a review briefing that includes:

1. **Design intent** — 3-5 bullet points from `spec.md` / `plan.md` (invariants, constraints, key decisions).
2. **Changed files** — list all files changed in the current track (use `git diff --name-only` against the last commit before the track, or list files from `metadata.json` task descriptions and current `git status`).
3. **Review checklist** — derived from `.claude/rules/04-coding-principles.md`, `05-testing.md`, `06-security.md`, and `project-docs/conventions/`:
   - Logic errors, edge cases, race conditions
   - No panics in library code, proper error propagation
   - Idiomatic Rust (naming, patterns)
   - Architecture layer dependency violations, domain I/O purity
   - Test coverage gaps
   - Security (input validation, error information leakage)

For the Codex provider, if the briefing exceeds ~1KB:
- Write it to `tmp/codex-briefing.md` (file-based briefing pattern from `codex-system` skill).
- Use `timeout 180 codex exec --model {model} --sandbox read-only --full-auto "Read tmp/codex-briefing.md and perform the task described there." 2>&1`.

For short briefings, use inline prompts.

## Step 3: Review → Fix → Review loop

### Round 1 (initial review)

Invoke the reviewer capability:

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Review {feature}. Report ONLY bugs or logic errors. Be concise.

## Design
{design invariants from Step 2}

## Changed files
{file list from Step 2}

Check for: logic errors, doc-code inconsistencies, edge cases, security issues,
architecture violations, test coverage gaps.
" 2>&1
```

Or use the file-based briefing if content is large.

Parse the reviewer output:
- If **zero findings**: proceed to Step 4 (done).
- If **findings exist**: proceed to fix phase.

### Fix phase

For each finding:
1. Assess severity (CRITICAL / HIGH / MEDIUM / LOW / INFO).
2. INFO-level findings: note but do not fix (cosmetic, style preference).
3. LOW and above: implement the fix.
4. If the finding requires a new test, add it.
5. Run `cargo make ci` (or `cargo make ci-rust` for fast inner loop) to verify fixes compile and pass.

### Round N (fix verification)

After fixes are applied, invoke the reviewer again:

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Previous review found: {finding summary}.
Fixed by: {fix description}. Tests added: {test names if any}.
Verify the fixes in {changed files}. Any remaining bugs or new issues?
" 2>&1
```

Parse the output:
- If **zero findings**: proceed to Step 4.
- If **new findings**: repeat fix phase → Round N+1.

### Loop guard

- Maximum 5 rounds. If findings persist after 5 rounds, stop and report remaining issues to the user for manual decision.
- Between rounds, always run `cargo make ci-rust` to ensure fixes don't break the build.

## Step 4: Final validation

After the reviewer reports zero findings:
1. Run `cargo make ci` (full CI, not just ci-rust) to confirm all checks pass.
2. If CI fails, fix and re-run (this does not reset the review loop counter).

## Behavior

After execution, summarize:
1. Total review rounds completed
2. Findings per round (count and severity breakdown)
3. Fixes applied (with file references)
4. Final CI result
5. Merge/commit readiness (Ready / Not ready with reason)
6. Recommended next command (`/track:commit <message>`)
