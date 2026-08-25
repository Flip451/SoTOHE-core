<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 規約を機構と突き合わせて改訂する

## Summary

GO-01 → T001, T002, T003, T004, T005, T006, T007, T008.

## Tasks (6/8 resolved)

### S1 — Enforcement taxonomy and convention inventory

> Establish D1's enforcement-destination taxonomy and annotate the existing governance, workflow, security, persistence, language, tooling, shell, and attribution convention rules. IN-01; CN-01; AC-01.

- [x] **T001**: Update `knowledge/conventions/enforce-by-mechanism.md` with D1's required enforcement-destination annotation rule, then inventory and annotate normative requirements in `dry-check-workflow.md`, `filesystem-persistence-guard.md`, `language-policy.md`, `nightly-dev-tool.md`, `no-backward-compat.md`, `security.md`, `shell-parsing.md`, `source-attribution.md`, and `workflow-ceremony-minimization.md`; validate the finite convention set in harness-policy review. D1; IN-01; CN-01; OUT-02; AC-01. (`3dd6cbcef47f9df61863db4250faac19e09f66e0`)

### S2 — Type and boundary convention alignment

> Revise the type and boundary convention set through four ordered operations, with file-level annotation ownership allocated across them. IN-01; IN-02; IN-06; IN-07; IN-08; OUT-01; AC-01; AC-02; AC-06; AC-07; AC-08.

- [x] **T002**: Revise `knowledge/conventions/type-designer-kind-selection.md` and `knowledge/conventions/prefer-type-safe-abstractions.md` for necessity-driven abstraction, and annotate the normative requirements in `prefer-type-safe-abstractions.md` with T001's taxonomy. D2; IN-01; IN-02; OUT-01; AC-01; AC-02. (`3dd6cbcef47f9df61863db4250faac19e09f66e0`)
- [x] **T006**: Revise `knowledge/conventions/type-designer-kind-selection.md` and `knowledge/conventions/coding-principles.md` for port injection and facade policy, and annotate the normative requirements in `coding-principles.md` with T001's taxonomy. D3; IN-01; IN-06; OUT-01; AC-01; AC-06.
- [ ] **T007**: Revise `knowledge/conventions/type-designer-kind-selection.md` and `knowledge/conventions/typed-deserialization.md` for the Command boundary, and annotate the normative requirements in `typed-deserialization.md` with T001's taxonomy. D4; IN-01; IN-07; OUT-01; AC-01; AC-07.
- [ ] **T008**: Revise `knowledge/conventions/type-designer-kind-selection.md` and `knowledge/conventions/tddd-product-correctness.md` for the layer-property matrix, and annotate the normative requirements in both files with T001's taxonomy. D5; IN-01; IN-08; OUT-01; AC-01; AC-08.

### S3 — Obligation-driven testing convention

> Rewrite `knowledge/conventions/testing.md` and annotate normative requirements with T001's taxonomy. IN-03; AC-03.

- [x] **T003**: Rewrite `knowledge/conventions/testing.md` and annotate normative requirements with T001's taxonomy. IN-01; IN-03; AC-01; AC-03.

### S4 — Environment-assumption declaration

> Create `knowledge/conventions/environment-assumptions.md`, annotate its rules with T001's taxonomy, annotate the existing normative Maintenance Rules in `knowledge/conventions/README.md` with the same taxonomy, and update the convention index. IN-01; IN-04; CN-02; OUT-03; AC-01; AC-04.

- [x] **T004**: Create `knowledge/conventions/environment-assumptions.md`, annotate its rules with T001's taxonomy, annotate the existing normative Maintenance Rules in `knowledge/conventions/README.md` with the same taxonomy, and update the convention index. IN-01; IN-04; CN-02; OUT-03; AC-01; AC-04. (`3dd6cbcef47f9df61863db4250faac19e09f66e0`)

### S5 — Review boundary meta-questions

> Update the configured code-scope prompt files under `.harness/custom/review-prompts/` and `.harness/custom/review-prompts/spec.md` with the required review questions. IN-05; CN-03; OUT-03; AC-05.

- [x] **T005**: Update `.harness/custom/review-prompts/domain.md`, `.harness/custom/review-prompts/usecase.md`, `.harness/custom/review-prompts/infrastructure.md`, `.harness/custom/review-prompts/cli.md`, `.harness/custom/review-prompts/cli_composition.md`, `.harness/custom/review-prompts/cli_driver.md`, and `.harness/custom/review-prompts/spec.md`; add the required review questions. IN-05; CN-03; OUT-03; AC-05.
