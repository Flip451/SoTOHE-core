<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Architecture Pattern Placement Guard Realignment

## Summary

GO-01 is delivered by semantic and lint realignment in T002-T003 and by the representative ClockPort flow in T004-T006.
GO-02 is delivered by T001's importer safeguard, synchronized policy/enforcement work in T002-T003, and T007's explicit end-to-end verification.
T001-T003 establish importer and enforcement primitives before the representative consumers; T004 precedes T005, T006 wires the completed boundary, and T007 verifies it.
AC-08 is preserved by keeping the representative implementation within the existing crate topology in T004-T007; AC-09 is preserved by T006's CLI-boundary work and T007's composed CLI-contract regression test.

## Tasks (0/7 resolved)

### import-and-policy — Importer safeguard and semantic policy

> Target `libs/infrastructure/src/schema_export/extract.rs` and `libs/infrastructure/src/schema_export.rs`; reapply stable destructured-parameter normalization and test identifier preservation plus tuple-pattern fallback (T001; IN-07, CN-01, AC-05, AC-07).
> Target `knowledge/conventions/type-designer-kind-selection.md`, `.harness/capabilities/type-designer.md`, `.harness/custom/review-prompts/types.md`, and `.harness/custom/review-prompts/cli_driver.md`; align semantic placement, role-matrix, Primary Adapter, and CQRS-review guidance (T002; IN-01, IN-02, IN-03, IN-06, IN-07, AC-01, AC-02, AC-04, AC-05).

- [ ] **T001**: Own `libs/infrastructure/src/schema_export/extract.rs` and `libs/infrastructure/src/schema_export.rs`: reapply rustdoc destructured-parameter pattern-name normalization before `ParamName` validation, preserve already-valid identifiers unchanged, generate stable positional names for tuple/destructured patterns, and add identifier-preservation plus tuple-pattern fallback regression tests. Direct anchors: IN-07, CN-01, AC-05, AC-07.
- [ ] **T002**: Own `knowledge/conventions/type-designer-kind-selection.md`, `.harness/capabilities/type-designer.md`, `.harness/custom/review-prompts/types.md`, and `.harness/custom/review-prompts/cli_driver.md`: replace structural domain-placement guidance with semantic-first evidence and the revised role matrix; document the permitted Primary Adapter usecase Command/Query/Response/boundary-DTO surface and prohibited leaks; require recorded CQRS asymmetry evidence; retain staged-audit limits without changing support workflows or crate topology. Direct anchors: IN-01, IN-02, IN-03, IN-06, IN-07, OUT-01, OUT-03, CN-03, CN-04, AC-01, AC-02, AC-04, AC-05.

### catalogue-enforcement — Catalogue-lint enforcement

> Target `catalogue_linter.rs`, `catalogue_linter_eval.rs`, `catalogue_linter_eval_helpers.rs`, `catalogue_linter_role.rs`, `catalogue_lint_workflow.rs`, `catalogue_lint_workflow_serde.rs`, active config, distributed preset, and their fixtures/tests; remove `DomainValueObjectInboundReferenceRequired` end-to-end and narrow Primary Adapter structural signature rules (T003; IN-01, IN-02, IN-03, IN-07, CN-03, AC-01, AC-02, AC-05, AC-06).
> T003 depends on T002's policy contract.

- [ ] **T003**: Own `libs/domain/src/tddd/catalogue_linter.rs`, `libs/domain/src/tddd/catalogue_linter_eval.rs`, `libs/domain/src/tddd/catalogue_linter_eval_helpers.rs`, `libs/domain/src/tddd/catalogue_linter_role.rs`, `libs/usecase/src/catalogue_lint_workflow.rs`, `libs/usecase/src/catalogue_lint_workflow_serde.rs`, `.harness/catalogue-lint/config.json`, `.harness/catalogue-lint/presets/ddd-strict.json`, and their regression fixtures/tests: narrow Primary Adapter signature enforcement to deterministic Entity/AggregateRoot/infrastructure/transport-leak boundaries while allowing usecase boundary contracts; remove and test the complete removal of `DomainValueObjectInboundReferenceRequired` from domain representation and evaluator, usecase representation and serde, active config, distributed preset, and regression fixtures; retain deterministic role/layer checks. Direct anchors: IN-01, IN-02, IN-03, IN-07, OUT-03, CN-01, CN-03, AC-01, AC-02, AC-05, AC-06, AC-07.

### clock-boundary — Usecase-owned clock boundary

> Target `libs/usecase/src/adr_baseline.rs`; add ClockPort-driven interactor acquisition and bind `test_adr_baseline_snapshot_uses_clock_port_timestamp` plus `test_adr_baseline_snapshot_propagates_clock_port_failure` with fake ports, retaining reference contracts (T004; IN-04, IN-05, IN-06, AC-03, AC-04).
> Target `libs/infrastructure/src/adr_baseline.rs`, `libs/infrastructure/src/adr_baseline/tests.rs`, and module exports; implement unit SystemClockAdapter, delete the obsolete timestamp function, and bind `test_system_clock_adapter_now_returns_valid_timestamp` only to feasible construction, timestamp production, and RFC3339 validation; T004 covers injected ClockPort failure propagation (T005; IN-04, IN-05, AC-03).
> T005 depends on T004.

- [ ] **T004**: Own `libs/usecase/src/adr_baseline.rs` and its unit tests: add `ClockPort`; make `AdrBaselineInteractor` acquire snapshot time through that port; adapt `AdrBaselineCommand` and `AdrBaselineError`; preserve the existing `AdrBaselineService`, `AdrBaselineSourcePort`, `AdrBaselineStorePort`, and `AdrBaselineTimestampError` contracts; and bind `test_adr_baseline_snapshot_uses_clock_port_timestamp` plus `test_adr_baseline_snapshot_propagates_clock_port_failure` with injected fake ports, without ceremonial CQRS splitting. Direct anchors: IN-04, IN-05, IN-06, OUT-02, CN-01, AC-03, AC-04, AC-07.
- [ ] **T005**: Own `libs/infrastructure/src/adr_baseline.rs`, `libs/infrastructure/src/adr_baseline/tests.rs`, and required infrastructure module exports: implement the unit `SystemClockAdapter: ClockPort`, remove `infrastructure::adr_baseline::timestamp_now`, and bind `test_system_clock_adapter_now_returns_valid_timestamp` to successful construction plus timestamp production and validation from the adapter's own `chrono::Utc::now()` RFC3339 representation. Do not add an adapter injection seam or require deterministic adapter failure coverage; T004 owns injected `ClockPort` failure propagation. Direct anchors: IN-04, IN-05, OUT-02, CN-01, AC-03, AC-07.

### driver-composition-and-verification — Driver/composition rewiring and verification

> Target `apps/cli-driver/src/adr_baseline.rs` and `apps/cli-composition/src/adr_baseline.rs`; remove delivery-layer time acquisition and wire SystemClockAdapter only in composition without altering the CLI contract (T006; IN-03, IN-04, IN-05, CN-02, AC-02, AC-03, AC-09).
> Target the new `apps/cli-composition/tests/adr_baseline_clock_flow.rs` and `apps/cli/tests/adr_baseline_cli_contract.rs`; add `test_adr_baseline_composed_clock_path_preserves_cli_contract` for the composed production happy path and `test_adr_baseline_check_review_missing_track_preserves_failure_cli_contract` for a deterministic public-CLI failure exit/stdout/stderr regression, then run the named workspace and SoTOHE validation commands; T004 covers injected ClockPort failure propagation (T007; IN-03, IN-04, IN-05, IN-07, AC-02, AC-03, AC-05, AC-06, AC-09).
> T006 depends on T004 and T005; T007 depends on T001 through T006.

- [ ] **T006**: Own `apps/cli-driver/src/adr_baseline.rs` and `apps/cli-composition/src/adr_baseline.rs`: remove timestamp-provider ownership from `AdrBaselineDriver`, keep it limited to typed input-to-usecase-boundary conversion and service invocation, and wire `SystemClockAdapter` only in `AdrBaselineCompositionRoot` while preserving the external CLI contract. Direct anchors: IN-03, IN-04, IN-05, OUT-02, CN-01, CN-02, AC-02, AC-03, AC-07, AC-09.
- [ ] **T007**: Own the new integration test files `apps/cli-composition/tests/adr_baseline_clock_flow.rs` and `apps/cli/tests/adr_baseline_cli_contract.rs`: `test_adr_baseline_composed_clock_path_preserves_cli_contract` exercises the composed ClockPort → SystemClockAdapter → AdrBaselineInteractor → AdrBaselineDriver production path and asserts unchanged CLI-observable happy-path output; `test_adr_baseline_check_review_missing_track_preserves_failure_cli_contract` invokes the public `sotp adr-baseline check-review` command against a deterministic missing-track fixture and asserts its failure exit state, empty stdout, and stable stderr diagnostic. Keep injected-clock failure propagation coverage in T004; this composition path has no injectable clock. Independently validate T001 through T006 with `cargo test --workspace`, `bin/sotp verify plan-artifact-refs`, `bin/sotp task-contract coverage`, and `bin/sotp task-contract check`; this task owns no unit-test edits in the T004, T005, or T006 source files. Direct anchors: IN-03, IN-04, IN-05, IN-07, OUT-01, OUT-02, CN-01, CN-02, AC-02, AC-03, AC-05, AC-06, AC-07, AC-09.
