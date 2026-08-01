<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Signal Report Command

## Summary

GO-01 → T001–T008.
IN-02 / CN-02 / AC-02 → T001, T004.
IN-03 / AC-03 → T001, T003, T005.
IN-04 / CN-01 / AC-04 → T001, T002, T006.
AC-01 → T001, T002, T004, T006, T008.
OUT-01 / AC-05 → T007.

## Tasks (8/8 resolved)

### S1 — Usecase boundary — typed report query

> Implement the typed report-query boundary, `SignalReportSourcePort` contract, and interactor behavior in `libs/usecase/src/signal_report/`. GO-01, IN-01–IN-04, CN-01–CN-02, AC-01–AC-04.

- [x] **T001**: Implement the catalogue-defined typed report-query boundary, `SignalReportSourcePort` contract, and interactor behavior in `libs/usecase/src/signal_report/`, with independently verifiable mock-based port and interactor tests. GO-01, IN-01–IN-04, CN-01–CN-02, AC-01–AC-04. (`43e23a96`)

### S2 — Infrastructure — report source adapter

> Implement and independently verify `SystemSignalReportSourceAdapter` in `libs/infrastructure/src/signal_report/`. IN-04, CN-01, AC-01, AC-04.

- [x] **T002**: After T001, implement and independently verify the catalogue-defined `SystemSignalReportSourceAdapter` in `libs/infrastructure/src/signal_report/`. IN-04, CN-01, AC-01, AC-04. (`43e23a96`)

### S3 — CLI driver — query conversion and rendering

> Implement filter conversion and rendering in `apps/cli-driver/src/signal_report/`. IN-02, IN-03, CN-02, AC-01–AC-03.

- [x] **T003**: After T001, implement filter-to-query conversion in `apps/cli-driver/src/signal_report/`. IN-03, AC-03. (`c2a77f82`)
- [x] **T004**: After T001 and T003, implement the `SignalReportDriver` occurrence-rendering contribution in `apps/cli-driver/src/signal_report/`. IN-02, CN-02, AC-01, AC-02. (`6eda10b1`)

### S4 — CLI transport and composition root

> Add signal-report CLI parsing, then dispatch through the composition-root report driver, and finally add `apps/cli/src/commands/track/resolve.rs` regression validation. IN-01, IN-03, OUT-01, CN-01, AC-01, AC-03–AC-05.

- [x] **T006**: After T001, T002, and T004, implement and independently verify `SignalCompositionRoot::signal_report_driver` wiring in `apps/cli-composition/`, before CLI report dispatch consumes it. CN-01, AC-01, AC-04. (`cc5ef158545673e58f2ead2999f6e41a637d1a74`)
- [x] **T005**: After T003, T004, and T006, add the parsing and command shape that makes `sotp signal report` parseable in `apps/cli/src/commands/signal/mod.rs`: `SignalReportChainArg`, `SignalReportOnlyArg`, `SignalReportArgs`, and `SignalCommand::Report`; add same-module parsing tests. IN-01, IN-03, AC-03.
- [x] **T008**: After T005, T004, and T006, add the signal-report branch of `execute` in `apps/cli/src/commands/signal/mod.rs`, constructing and using the composition-root report driver while propagating output and exit status; add focused dispatch tests. IN-01, AC-01.
- [x] **T007**: After T008, add `sotp track resolve` regression tests in `apps/cli/src/commands/track/resolve.rs`. OUT-01, AC-05.
