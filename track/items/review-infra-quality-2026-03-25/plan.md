<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# RVW-13/15/17 Review infrastructure quality hardening

RVW-15: GitDiffScopeProvider contract tests with tempdir git fixtures.
RVW-17/18: codex-reviewer agent tools: restriction verification and remediation.
RVW-13: --auto-record end-to-end validation via this track's own review cycle.
Signal system integration: /track:plan skill auto-evaluates signals after spec.json creation.

## GitDiffScopeProvider contract tests (RVW-15)

Create tempdir-based git fixture tests for GitDiffScopeProvider.
Cover: merge-base diff, staged changes, unstaged worktree modifications, untracked files, renames, deletes.
Verify error propagation: non-zero git exits return DiffScopeProviderError, not empty scope.

- [x] GitDiffScopeProvider contract tests — tempdir git fixture covering merge-base diff, staged changes, unstaged worktree changes, untracked files, renames, deletes, and error propagation

## codex-reviewer agent verification (RVW-17/18)

Test codex-reviewer agent with tools: Bash(cargo make track-local-review:*) restriction.
If tools: frontmatter is ignored by Claude Code, implement alternative structural enforcement.
Document findings in verification.md.

- [x] codex-reviewer agent tools: restriction verification — confirm Bash(cargo make track-local-review:*) actually limits tools, fix if not

## --auto-record end-to-end (RVW-13)

This track's own /track:review cycle uses --auto-record flags.
Validates the full flow: verdict extraction -> scope filter -> record-round internally.
No code changes needed if auto-record works correctly; document any issues found.

- [x] review.md --auto-record full migration — update /track:review to always pass --auto-record flags, validate via this track's own review cycle

## Signal system integration in /track:plan

Add sotp track signals invocation after spec.json creation in /track:plan skill.
Ensures signals field is populated automatically instead of remaining null.

- [x] /track:plan skill に spec.json 生成後の信号機評価ステップを追加 — sotp track signals を自動実行して signals フィールドを埋める
