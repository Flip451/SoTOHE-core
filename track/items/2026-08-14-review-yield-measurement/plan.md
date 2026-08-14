<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# review-yield measurement

## Summary

GO-01 → T001, T002, T003, T004, T005.

## Tasks (0/5 resolved)

### S1 — Review-yield aggregation model

> `libs/usecase/src/telemetry/review_yield.rs` and `libs/usecase/src/telemetry/report.rs::TelemetryReportOutput`. IN-02; AC-02; CO-02.

- [ ] **T001**: Implement `libs/usecase/src/telemetry/review_yield.rs` and extend `libs/usecase/src/telemetry/report.rs::TelemetryReportOutput`; add focused aggregation and error regressions in `libs/usecase/src/telemetry.rs`. IN-02; AC-02; CO-02.

### S2 — Structured-review telemetry persistence

> `libs/infrastructure/src/telemetry/review_yield.rs::StructuredReviewRoundDto` and `libs/infrastructure/src/telemetry/mod.rs::TelemetryEvent`. IN-01; AC-01; AC-03; CO-01.

- [ ] **T002**: Add `libs/infrastructure/src/telemetry/review_yield.rs::StructuredReviewRoundDto` and extend `libs/infrastructure/src/telemetry/mod.rs::TelemetryEvent`; add focused persistence and serialization regressions in those modules. IN-01; AC-01; AC-03; CO-01.

### S3 — Review-yield report projection

> `libs/infrastructure/src/telemetry/report.rs::TelemetryReportSnapshot` and `libs/infrastructure/src/telemetry/report_adapter.rs::FsTelemetryReportAdapter::aggregate`. IN-02; AC-02; CO-02.

- [ ] **T003**: Extend `libs/infrastructure/src/telemetry/report.rs::TelemetryReportSnapshot` and update `libs/infrastructure/src/telemetry/report_adapter.rs::FsTelemetryReportAdapter::aggregate` to project `review_yield_metrics`; add focused projection regressions in those modules. IN-02; AC-02; CO-02.

### S4 — Structured-review telemetry wiring

> `apps/cli-composition/src/review_v2/mod.rs::ReviewCompositionRoot::review_run_local_ungated` and `apps/cli-composition/src/telemetry_wiring.rs::emit_review_round`. IN-01; OUT-01; OUT-02; AC-01; AC-03; CO-01; CO-02.

- [ ] **T004**: Update `apps/cli-composition/src/review_v2/mod.rs::ReviewCompositionRoot::review_run_local_ungated` and `apps/cli-composition/src/telemetry_wiring.rs::emit_review_round`; add focused telemetry-wiring regressions in these modules. IN-01; OUT-01; OUT-02; AC-01; AC-03; CO-01; CO-02.

### S5 — Telemetry report rendering and command handling

> `apps/cli-driver/src/telemetry.rs::{TelemetryDriver::telemetry_report,format_report}` and `apps/cli/src/commands/telemetry.rs::{TelemetryCommand,execute}`. IN-02; OUT-02; AC-02; CO-02.

- [ ] **T005**: Extend `apps/cli-driver/src/telemetry.rs::{TelemetryDriver::telemetry_report,format_report}` and `apps/cli/src/commands/telemetry.rs::{TelemetryCommand,execute}`; add focused command and renderer regressions in these modules. IN-02; OUT-02; AC-02; CO-02.
