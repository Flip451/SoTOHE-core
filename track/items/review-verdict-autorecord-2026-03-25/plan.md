<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# RVW-10/11 Review verdict auto-record + diff scope enforcement

RVW-10: verdict falsification prevention via --auto-record mode in codex-local (record-round called internally after verdict extraction).
RVW-11: diff scope enforcement via structural finding filtering (normalized repo-relative path matching, fail-closed for unknown paths).

## Scope filtering (RVW-11)

DiffScope + RepoRelativePath in usecase layer. DiffScopeProvider port with GitDiffScopeProvider adapter.
Path normalization: strip ./ and \ separators, convert absolute to repo-relative.
Unknown paths treated as in-scope (fail-closed). Exact match only (no suffix match - ambiguous in monorepo).

- [ ] Usecase: scope.rs - DiffScope, RepoRelativePath, DiffScopeProvider port, classify/partition/apply_scope_filter + unit tests (TDD: classify InScope/OutOfScope/UnknownPath, partition, apply_scope_filter with adjusted verdict)
- [ ] Infrastructure: GitDiffScopeProvider adapter (merge-base diff, renames, deletions, untracked files) + integration tests (TDD: diff against real git repo fixture)

## Typed record-round (RVW-10 prerequisite)

Add record_round_typed() usecase entrypoint that accepts parsed domain types (RoundType, ReviewGroupName, Verdict, Vec<ReviewConcern>, etc.) directly.
Keep existing string-based record_round() as external CLI adapter only.

- [ ] Usecase: record_round_typed() - typed entrypoint accepting parsed domain types directly (keep string-based record_round as CLI adapter) + unit tests (TDD: valid/invalid inputs, protocol delegation)

## Auto-record CLI integration (RVW-10)

Extend CodexLocalArgs with auto-record flags. Validate all args before spawning Codex (fail fast).
After verdict extraction: compute DiffScope, apply scope filter, extract concerns, call record_round_typed internally.
Exit codes: 0=zero_findings, 2=findings_remain, 1=error, 3=escalation.

- [ ] CLI: Extended CodexLocalArgs (--auto-record, --track-id, --round-type, --group, --expected-groups, --items-dir, --diff-base) with pre-spawn validation + unit tests (TDD: arg validation, requires constraints)
- [ ] CLI: auto-record execution flow in codex_local.rs (verdict -> scope filter -> concerns -> record_round_typed) + integration tests (TDD: mock codex binary, verify record-round called with correct verdict)

## Orchestrator integration

Update review.md orchestrator command and Makefile.toml to pass --auto-record --diff-base main.
Backward compatible: --auto-record disabled preserves current behavior.

- [ ] Integration: Makefile.toml + orchestrator command (.claude/commands/track/review.md) update to use --auto-record --diff-base
