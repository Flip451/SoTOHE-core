<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Workflow byproduct disk hygiene: scope diff untracked-directory measurement failure and template export test /tmp scaffold leak

## Summary

GO-01 → T001, T002, T003, T004, T005.

## Tasks (4/5 resolved)

### S1 — Scope-diff configuration

> Update the scope-diff config targets and adapter regressions. D1; IN-01; CN-01; AC-01; AC-02.

- [x] **T001**: Update `libs/infrastructure/src/scope_diff_measure.rs`, the scope-diff config under `.harness/config/`, and focused adapter regression tests. D1; IN-01; CN-01; AC-01; AC-02. (`956d32a3c1270d1d8b37dc4da41cda168e9e340a`)

### S2 — Untracked directory filtering

> Update the scope-diff adapter filtering target and regressions. D2; IN-02; CN-02; AC-03.

- [x] **T002**: Update `libs/infrastructure/src/scope_diff_measure.rs` and focused adapter regression tests for untracked entry filtering. D2; IN-02; CN-02; AC-03.

### S3 — Scaffold placement

> Update integration and in-process scaffold test targets. D3; IN-03; CN-03; AC-04.

- [x] **T003**: Update scaffold setup in `apps/cli/tests/` and the in-process export regression in `apps/cli-composition/src/template_export/mod.rs`. D3; IN-03; CN-03; AC-04. (`956d32a3c1270d1d8b37dc4da41cda168e9e340a`)

### S4 — Host-first scaffold lifecycle

> Update the host-first scaffold test target and regression coverage. D4; IN-04; AC-05.

- [x] **T004**: Update `apps/cli/tests/consumer_scaffold_host_first.rs` and its focused regression coverage. D4; IN-04; AC-05.

### S5 — Template-export transplant

> Update template-export transplant regression targets. D5; IN-05; OS-01; CN-04; AC-06; AC-07.

- [ ] **T005**: Update template-export transplant regressions in `apps/cli/tests/` and `apps/cli-composition/src/template_export/mod.rs`. D5; IN-05; OS-01; CN-04; AC-06; AC-07.
