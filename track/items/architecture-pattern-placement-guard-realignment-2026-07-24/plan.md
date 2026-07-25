<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Architecture Pattern Placement Guard Realignment

## Summary

GO-01: T002-T006.
GO-02: T001-T003 and T007.
Dependencies: T002 → T003; T004 → T005; T004/T005 → T006; T001-T006 → T007.
AC-08: T004-T007. AC-09: T006-T007.

## Tasks (7/7 resolved)

### import-and-policy — Importer safeguard and semantic policy

> Update schema-export parameter normalization and regression tests (T001; IN-07, CN-01, AC-05, AC-07).
> Update type-design conventions, capability guidance, and reviewer prompts (T002; IN-01, IN-02, IN-03, IN-06, IN-07, AC-01, AC-02, AC-04, AC-05).

- [x] **T001**: Update `libs/infrastructure/src/schema_export/extract.rs` and `libs/infrastructure/src/schema_export.rs` with parameter-name normalization and regression tests. IN-07, CN-01, AC-05, AC-07.
- [x] **T002**: Update `knowledge/conventions/type-designer-kind-selection.md`, `.harness/capabilities/type-designer.md`, and `.harness/custom/review-prompts/{types,usecase,cli_driver}.md` for placement and review evidence. IN-01, IN-02, IN-03, IN-06, IN-07, OUT-01, OUT-03, CN-03, CN-04, AC-01, AC-02, AC-04, AC-05.

### catalogue-enforcement — Catalogue-lint enforcement

> Update catalogue-lint rules, configuration, presets, fixtures, and regression tests (T003; IN-01, IN-02, IN-03, IN-07, CN-03, AC-01, AC-02, AC-05, AC-06).
> T003 depends on T002's policy contract.

- [x] **T003**: Update catalogue-lint rule evaluation in `libs/domain/src/tddd/catalogue_linter.rs` and `libs/domain/src/tddd/catalogue_linter_eval.rs`, workflow enforcement in `libs/usecase/src/catalogue_lint_workflow.rs`, `.harness/catalogue-lint/{config.json,presets/ddd-strict.json}`, and regression tests in `apps/cli/tests/cli_catalogue_lint.rs` and `libs/infrastructure/tests/catalogue_lint_shipped_config.rs`. IN-01, IN-02, IN-03, IN-07, OUT-03, CN-01, CN-03, AC-01, AC-02, AC-05, AC-06, AC-07.

### clock-boundary — Usecase-owned clock boundary

> Update usecase ADR-baseline clock boundary and unit tests (T004; IN-04, IN-05, IN-06, AC-03, AC-04).
> Update infrastructure ADR-baseline clock adapter, exports, and unit tests (T005; IN-04, IN-05, AC-03).
> T005 depends on T004.

- [x] **T004**: Update `libs/usecase/src/adr_baseline.rs` and unit tests for the clock boundary. IN-04, IN-05, IN-06, OUT-02, CN-01, AC-03, AC-04, AC-07.
- [x] **T005**: Update `SystemClockAdapter` and `timestamp_now` in `libs/infrastructure/src/adr_baseline.rs`, its exports in `libs/infrastructure/src/lib.rs`, and unit tests in `libs/infrastructure/src/adr_baseline/tests.rs`. IN-04, IN-05, OUT-02, CN-01, AC-03, AC-07.

### driver-composition-and-verification — Driver/composition rewiring and verification

> Update ADR-baseline driver and composition-root clock wiring (T006; IN-03, IN-04, IN-05, CN-02, AC-02, AC-03, AC-09).
> Add ADR-baseline clock-flow and CLI-contract integration tests, then run validation (T007; IN-03, IN-04, IN-05, IN-07, AC-02, AC-03, AC-05, AC-06, AC-09).
> T006 depends on T004 and T005; T007 depends on T001 through T006.

- [x] **T006**: Update `AdrBaselineDriver` in `apps/cli-driver/src/adr_baseline.rs` and `AdrBaselineCompositionRoot` clock wiring in `apps/cli-composition/src/adr_baseline.rs`. IN-03, IN-04, IN-05, OUT-02, CN-01, CN-02, AC-02, AC-03, AC-07, AC-09.
- [x] **T007**: Add ADR-baseline clock-flow integration tests in `apps/cli-composition/tests/adr_baseline_clock_flow.rs` and CLI-contract integration tests in `apps/cli/tests/adr_baseline_cli_contract.rs`, then run workspace and track validation. IN-03, IN-04, IN-05, IN-07, OUT-01, OUT-02, CN-01, CN-02, AC-02, AC-03, AC-05, AC-06, AC-07, AC-09.
