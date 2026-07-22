<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 型契約パイプラインの規範と機構を実挙動に整合させる

## Summary

Organizes ADR D1-D4 implementation into four independently reviewable tasks.
GO-01 is delivered by T001 (placement convention), T002 (catalogue lint), T003 (empty-layer baseline graph), and T004 (contract-map styles).
Orders convention, lint, generator, and renderer work for focused commits.

## Tasks (0/4 resolved)

### S1 — ValueObject placement policy and enforcement

> Groups T001 and T002.

- [ ] **T001**: Update the type-designer kind-selection convention for IN-01, CN-01, and AC-01.
- [ ] **T002**: Add the inbound-reference rule to `libs/domain/src/tddd/catalogue_linter.rs` and `catalogue_linter_eval.rs`, and wire its typed configuration through `libs/usecase/src/catalogue_lint_workflow.rs`. Add evaluator and workflow regression coverage for AC-02. IN-02, CN-01, CN-02, AC-02.

### S2 — Empty-layer generation canonicalization

> Groups T003.

- [ ] **T003**: Align the empty-layer 12a receipt in `.harness/capabilities/type-designer.md` and `.agents/skills/type-designer/SKILL.md` with the baseline-graph workflow and output documentation in `libs/usecase/src/baseline_graph_workflow.rs` and `libs/infrastructure/src/tddd/baseline_graph_writer_adapter.rs`. Add corresponding regression coverage in those workflow and writer modules for AC-03. IN-03, CN-03, AC-03.

### S3 — Contract-map style completeness and visibility

> Groups T004.

- [ ] **T004**: Complete `.harness/config/contract-map-style.toml` and update `RoleKind` in `libs/domain/src/tddd/catalogue_linter_role.rs` plus the contract-map renderer result flow in `libs/domain/src/tddd/contract_map_renderer.rs`, `libs/usecase/src/contract_map_workflow.rs`, and `libs/infrastructure/src/tddd/contract_map_renderer_adapter/`. Add renderer and workflow regression coverage for AC-04. IN-04, CN-04, AC-04.
