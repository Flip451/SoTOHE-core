<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# レビュー指示書のカテゴリ閉列挙を半開形式へ改める

## Summary

GO-01: T001 and T002. IN-01/IN-02/IN-03/IN-04/IN-05/AC-01/AC-02/AC-03/AC-04.
GO-02: T003. IN-06/AC-05.
All Phase 2 catalogues are empty; this documentation-only implementation has no catalogue entries to attribute.

## Tasks (3/3 resolved)

### S1 — Code-layer half-open reviewer briefings

> Update `.harness/custom/review-prompts/domain.md`, `usecase.md`, `infrastructure.md`, `cli.md`, `cli_composition.md`, `cli_driver.md`, and `harness-policy.md`. IN-01/IN-02/IN-03/OUT-02/OUT-03/CN-01/CN-03/AC-01/AC-02.

- [x] **T001**: Update `.harness/custom/review-prompts/domain.md`, `usecase.md`, `infrastructure.md`, `cli.md`, `cli_composition.md`, `cli_driver.md`, and `harness-policy.md`. IN-01/IN-02/IN-03/OUT-02/OUT-03/CN-01/CN-03/AC-01/AC-02. (`6bb4c34e726b68e53a956bbed07766ae199284f1`)

### S2 — Document-inspection and SoT-briefing boundary

> Individually adjudicate and update the existing `.harness/custom/review-prompts/adr.md`, `spec.md`, `types.md`, and `impl-plan.md`. IN-02/IN-03/IN-04/IN-05/OUT-01/OUT-02/CN-02/AC-02/AC-03/AC-04.

- [x] **T002**: Individually adjudicate and update the existing `.harness/custom/review-prompts/adr.md`, `spec.md`, `types.md`, and `impl-plan.md`. IN-02/IN-03/IN-04/IN-05/OUT-01/OUT-02/CN-02/AC-02/AC-03/AC-04. (`6bb4c34e726b68e53a956bbed07766ae199284f1`)

### S3 — Maintainer contradiction check

> Add a briefing role-statement/category check to `.claude/rules/09-maintainer-checklist.md`. IN-06/AC-05.

- [x] **T003**: Add a briefing role-statement/category check to `.claude/rules/09-maintainer-checklist.md`. IN-06/AC-05. (`6bb4c34e726b68e53a956bbed07766ae199284f1`)
