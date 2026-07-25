<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# role × 層マトリクスを機構で強制し ValueObject の層勾配を是正する

## Summary

GO-01: T001-T003.
Dependencies: T001 specifies review evidence; T002 introduces the shipped rule set; T003 verifies the active-track behavior and shipped artifacts after T002.
All tasks are bounded below the per-task 500-line reviewability target.

## Tasks (3/3 resolved)

### policy-and-review-evidence — R1 policy and review evidence

> Update the R1 convention, type-designer capability contract, and type-review briefing with placement and ValueObject-evidence guidance (T001; IN-01, IN-02, OUT-01 through OUT-04, CN-03, AC-01 through AC-03).

- [x] **T001**: Update `knowledge/conventions/type-designer-kind-selection.md`, `.harness/capabilities/type-designer.md`, and `.harness/custom/review-prompts/types.md` with the R1 role × layer matrix and ValueObject placement-evidence review guidance. `track/items/role-layer-matrix-enforcement-2026-07-25/spec.json#IN-01`, `#IN-02`, `#OUT-01`, `#OUT-02`, `#OUT-03`, `#OUT-04`, `#CN-03`, `#AC-01`, `#AC-02`, `#AC-03`.

### shipped-rule-set — Shipped role-layer rule set

> Update the shipped config and strict preset together with the R1 layer constraints (T002; IN-01 through IN-03, OUT-02 through OUT-04, CN-01, CN-03, AC-01 through AC-04).

- [x] **T002**: Update `.harness/catalogue-lint/config.json` and `.harness/catalogue-lint/presets/ddd-strict.json` together with the R1 layer constraints, preserving their byte-identical rule set. `track/items/role-layer-matrix-enforcement-2026-07-25/spec.json#IN-01`, `#IN-02`, `#IN-03`, `#OUT-02`, `#OUT-03`, `#OUT-04`, `#CN-01`, `#CN-03`, `#AC-01`, `#AC-02`, `#AC-03`, `#AC-04`.

### regression-coverage — Active-track and shipped-config regression coverage

> After T002, update `apps/cli/tests/cli_catalogue_lint.rs` and `libs/infrastructure/tests/catalogue_lint_shipped_config.rs` to derive regression expectations from decoded config (T003; IN-01 through IN-03, OUT-01 through OUT-04, CN-01 through CN-03, AC-01 through AC-05).

- [x] **T003**: Update active-track catalogue-lint regression coverage in `apps/cli/tests/cli_catalogue_lint.rs`, shipped-config decoding/equality coverage in `libs/infrastructure/tests/catalogue_lint_shipped_config.rs`, and `libs/infrastructure/src/tddd/fs_lint_config_loader.rs` shipped-config tests to derive expectations only from production-loader decoding and config/preset equality, removing rule-kind-specific shipped-config assertions. `track/items/role-layer-matrix-enforcement-2026-07-25/spec.json#IN-01`, `#IN-02`, `#IN-03`, `#OUT-01`, `#OUT-02`, `#OUT-03`, `#OUT-04`, `#CN-01`, `#CN-02`, `#CN-03`, `#AC-01`, `#AC-02`, `#AC-03`, `#AC-04`, `#AC-05`.
