<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Composition Root Pure DI Realignment

## Summary

GO-01 is mapped to T001, T002, and T003.
T001 updates catalogue-lint before review-policy work.
T002 updates `.harness/custom/review-prompts/cli_composition.md` after the lint vocabulary.
T003 updates and regression-tests the shipped greeting placeholder after the policy work.

## Tasks (2/3 resolved)

### enforcement — Catalogue-lint enforcement

> Update the catalogue-lint rule path and regression cases. IN-03/CN-03/AC-04. T001.

- [x] **T001**: Update `CatalogueLinterRule`, `CatalogueLinterRuleKind`, `evaluate_catalogue_lint`, and `LintRuleKind`; add catalogue-lint regression cases. IN-03/OUT-02/CN-03/AC-04. (`d2887360daf79a0882c860259082db3b25b3bb5f`)

### review-policy — Composition review policy

> Update priority categories in `.harness/custom/review-prompts/cli_composition.md`. IN-02/CN-01/CN-02/AC-03. T002.

- [x] **T002**: Update the `invoke leak`, public-surface exposure, and `PrimaryAdapter` allowance priority categories in `.harness/custom/review-prompts/cli_composition.md`. IN-02/CN-01/CN-02/AC-03.

### shipped-positive-example — Shipped pure-DI positive example

> Update and regression-test `run_greeting`, `GreetDriver`, and greeting `main`; verify ADR D4. IN-01/IN-04/OUT-01/CN-01/CN-02/CN-04/AC-01/AC-02/AC-05. T003.

- [ ] **T003**: Update `run_greeting`, `GreetDriver`, and greeting `main` in the three `overlay/apps/cli*/src/{lib.rs,main.rs}` files; add their regression tests; and verify D4 in `knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md`. IN-01/IN-04/OUT-01/CN-01/CN-02/CN-04/AC-01/AC-02/AC-05.
