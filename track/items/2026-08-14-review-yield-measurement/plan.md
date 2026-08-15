<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# review-yield measurement

## Summary

GO-01 → T001, T002, T004, T005. Test-obligation binding reconciliation belongs to T001 and T002, which own the enrolled catalogue entries.

## Tasks (4/4 resolved)

### S1 — Review-yield aggregation model

> Completed usecase relocation and reconciliation of test-obligation binding records for its attributed catalogue entries. IN-01; IN-02; AC-01; AC-02; CO-01; CO-02.

- [x] **T001**: Finalize libs/usecase/src/telemetry/review_yield.rs (ReviewYieldMetric, ReviewYieldValue) and libs/usecase/src/telemetry.rs (TelemetryReportOutput, TelemetryReportPort), including focused regressions and reconciliation of test-obligation binding records for its attributed catalogue entries. IN-01; IN-02; AC-01; AC-02; CO-01; CO-02. (`27d63c5a`)

### S2 — Structured-review telemetry persistence and report projection

> Completed infrastructure relocation and reconciliation of test-obligation binding records for its attributed catalogue entries. IN-01; IN-02; AC-01; AC-02; AC-03; CO-01; CO-02.

- [x] **T002**: Finalize libs/infrastructure/src/review_v2/review_yield.rs (ReviewYieldRecordingReviewer), libs/infrastructure/src/telemetry/review_yield.rs (StructuredReviewRoundDto), and libs/infrastructure/src/telemetry/report.rs (TelemetryReport::aggregate), including persistence and projection regressions and reconciliation of test-obligation binding records for its attributed catalogue entries. IN-01; IN-02; AC-01; AC-02; AC-03; CO-01; CO-02. (`27d63c5a`)

### S4 — Structured-review telemetry wiring

> Completed composition relocation. IN-01; OUT-01; OUT-02; AC-01; AC-03; CO-01; CO-02.

- [x] **T004**: Finalize apps/cli-composition/src/review_v2/mod.rs and apps/cli-composition/src/telemetry_wiring.rs (emit_review_round) by removing composition-side capture, decoration, and emission, including focused composition regressions. IN-01; OUT-01; OUT-02; AC-01; AC-03; CO-01; CO-02. (`27d63c5a`)

### S5 — Telemetry report rendering and command handling

> Completed telemetry report rendering and command handling. IN-02; OUT-02; AC-02; CO-02.

- [x] **T005**: Finalize apps/cli-driver/src/telemetry.rs (TelemetryDriver::telemetry_report, format_report) and apps/cli/src/commands/telemetry.rs (execute) to render telemetry reports and handle report, including focused command and renderer regressions. IN-02; OUT-02; AC-02; CO-02. (`27d63c5a`)
