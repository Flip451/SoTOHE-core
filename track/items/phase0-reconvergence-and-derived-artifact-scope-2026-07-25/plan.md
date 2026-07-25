<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Phase 0 Reconvergence Lane and Derived-Artifact Review Scope

## Summary

GO-01: T1, T2, T3.
GO-02: T4.

## Tasks (0/4 resolved)

### phase0-convention — Phase 0 reconvergence convention

> Update the convention and its maintained guidance surfaces. IN-01, CN-01, CN-02, OS-03, AC-01, AC-03, AC-07.

- [ ] **T1**: Update `knowledge/conventions/pre-track-adr-authoring.md`; synchronize the maintainer guidance in `CLAUDE.md` and the user guidance in `README.md` with that convention. IN-01, CN-01, CN-02, OS-03, AC-01, AC-03, AC-07.

### phase0-workflow-adapters — Phase 0 workflow delegation and adapters

> Update the plan and adr2pr workflow surfaces to reference the convention. IN-02, CN-01, CN-02, OS-03, AC-02, AC-03, AC-07.

- [ ] **T2**: Update `.harness/workflows/track/plan.md`, `.claude/commands/track/plan.md`, and `.agents/skills/track-plan/SKILL.md` to reference the governing convention. IN-02, CN-01, CN-02, OS-03, AC-02, AC-03, AC-07.
- [ ] **T3**: Update `.harness/workflows/track/adr2pr.md`, `.claude/commands/track/adr2pr.md`, and `.agents/skills/track-adr2pr/SKILL.md` to reference the governing convention. IN-02, CN-01, CN-02, OS-03, AC-02, AC-03, AC-07.

### review-operational-classification — Derived obligation review classification

> Update the review-scope configuration and its regression coverage. IN-03, OS-01, OS-02, CN-03, AC-04, AC-05, AC-06.

- [ ] **T4**: Update `.harness/config/review-scope.json` and add `test_shipped_review_scope_classifies_obligations_as_operational_and_test_bindings_as_content` to `libs/infrastructure/tests/review_scope_shipped_config.rs` as the review-scope classification regression test. IN-03, OS-01, OS-02, CN-03, AC-04, AC-05, AC-06.
