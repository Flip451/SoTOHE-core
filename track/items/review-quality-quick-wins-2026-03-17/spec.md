---
status: draft
version: "1.1"
---

# Spec: Review Quality Quick Wins

## Goal

Eliminate two recurring review-cycle bug patterns identified during phase1-sotp-hardening:
1. Duplicated frontmatter parsing logic causing the same bug (trim_end vs exact match) to appear 3 times
2. Hand-rolled `serde_json::Value` walking in `canonical_modules.rs` that silently drops invalid data via `filter_map` (fail-open)

## Scope

- `libs/infrastructure/src/verify/` — new frontmatter.rs module, refactor spec_attribution.rs and spec_frontmatter.rs
- `libs/infrastructure/src/verify/canonical_modules.rs` — replace `parse_canonical_rules` hand-rolled JSON walking with typed `#[derive(Deserialize)]` structs
- `project-docs/conventions/typed-deserialization.md` — new convention document

## Constraints

- No new crate dependencies (serde + serde_json already in workspace)
- No changes to domain or usecase layers
- Existing tests must continue to pass
- New shared module must have equivalent test coverage

## Acceptance Criteria

- [ ] `spec_attribution.rs` and `spec_frontmatter.rs` both delegate frontmatter parsing to a single shared function
- [ ] No duplicated `---` delimiter matching logic exists
- [ ] `canonical_modules.rs` uses `#[derive(Deserialize)]` structs instead of manual `serde_json::Value` walking
- [ ] Invalid JSON (non-string in `allowed_in`, missing fields) causes deserialization error, not silent data loss
- [ ] `project-docs/conventions/typed-deserialization.md` documents the convention: prefer typed deserialization over `serde_json::Value` manual walking in verify/guard code
- [ ] `project-docs/conventions/prefer-type-safe-abstractions.md` documents the general principle: prefer standard library abstractions (serde, syn, etc.) over hand-rolled code + lint rules to eliminate bug classes at the type level
- [ ] `project-docs/conventions/README.md` indexes both new convention docs
- [ ] `cargo make ci` passes
