<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# CI Guardrails Phase 1.5: review guard fix + CI detection net + view-freshness

CI guardrails for Phase 1.5: fix review guard blocking planning artifacts (WF-54), add module-size/domain-strings/view-freshness verify subcommands, and clippy::too_many_lines attribute

## Phase A: Review Guard Fix (WF-54)

T001: Initialize review state (status: not_started, groups: {}) in metadata.json at track creation time
check-approved accepts NotStarted as valid (no code to review yet)
review state absent = error (make illegal states unrepresentable)

- [ ] WF-54: Initialize review state in metadata.json at track creation + check-approved accepts NotStarted

## Phase B: CI Detection Net

T002: sotp verify module-size — scan .rs files, warn at 400 lines, error at 700 lines, exclude vendor/
T003: sotp verify domain-strings — syn AST parse libs/domain/src/ for pub String fields (newtype excluded)
T004: Add #![warn(clippy::too_many_lines)] to apps/cli/src/main.rs

- [ ] sotp verify module-size: RS file line count CI gate (warn 400 / error 700, vendor/ excluded)
- [ ] sotp verify domain-strings: detect pub String fields in domain layer via syn AST parsing
- [ ] Add #![warn(clippy::too_many_lines)] to CLI crate for function-level bloat detection

## Phase C: View Freshness Gate (WF-55 P1)

T005a: sotp verify view-freshness — render plan.md from metadata.json, compare with on-disk file
T005b: registry.md を .gitignore に追加し git rm --cached で untrack (STRAT-04)
T005c: registry.md は cargo make track-sync-views で生成のみ (CI commit 不要)
T005d: verify-track-registry タスクを registry.md file check から metadata.json ベースに移行
Integrate view-freshness into cargo make ci

- [ ] WF-55 Phase 1: sotp verify view-freshness for plan.md + registry.md gitignore (STRAT-04)
