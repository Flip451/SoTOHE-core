<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Sotp Tracing Instrumentation

## Summary

GO-01 → T001, T003, T006, T010.
IN-01 / IN-02 / IN-04 / CN-02 / AC-01 / AC-03–AC-05 → T010.
IN-03 / CN-03 / AC-02 → T003, T006.

## Tasks (4/4 resolved)

### S1 — Existing telemetry event schema and aggregation

> libs/usecase/src/telemetry/command_trace.rs, libs/usecase/src/telemetry.rs::TelemetryAggregateInteractor::report, and libs/infrastructure/src/telemetry/report.rs::TelemetryReport::aggregate — implement and regression-test telemetry schema and aggregation work. IN-01–IN-03, OUT-01–OUT-02, CN-01, CN-03, AC-01–AC-02, AC-05.

- [x] **T001**: libs/usecase/src/telemetry/command_trace.rs — implement and regression-test command-telemetry value objects. IN-01, IN-02, OUT-01, OUT-02, CN-01, AC-01, AC-05. (`9f1fda8a075c52709e99a2bee44e4f698bed3bf2`)
- [x] **T003**: libs/usecase/src/telemetry.rs::TelemetryAggregateInteractor::report — implement and regression-test telemetry aggregation. IN-03, CN-01, CN-03, AC-02. (`88362dca5c29c4038c95d8d9ad2657a677e7f186`)
- [x] **T006**: libs/infrastructure/src/telemetry/report.rs::TelemetryReport::aggregate — implement and regression-test telemetry-report aggregation. IN-03, CN-01, CN-03, AC-02. (`1dc7b80fca414e70b70705d5313ffec72ceefed6`)

### S2 — Branch-bound CLI lifecycle regression coverage

> apps/cli/src/main.rs::run_cli_with_context! (pre-dispatch context capture, command dispatch, eligibility policy, and data-only EmitCompletedCommand submission), telemetry_completion_eligible, and commands::track::execute_with_error_chain; apps/cli/src/main.rs::command_identity_from_args; apps/cli-driver/src/telemetry.rs::TelemetryInput, TelemetryDriver::handle, and items_dir_from_args; libs/usecase/src/telemetry.rs::TelemetryEmitService::emit_completed, TelemetryEmitInteractor, TelemetryArchiveInteractor, and TelemetryAggregateInteractor; libs/infrastructure/src/telemetry/report_adapter.rs::FsTelemetryEmitDynamicAdapter::emit_active; apps/cli-composition/src/telemetry.rs::TelemetryCompositionRoot (wiring-only reference) — implement and regression-test the branch-bound completion lifecycle, existing telemetry.jsonl sink, archive timing, and composition boundaries. IN-01, IN-02, IN-04, OUT-01–OUT-04, CN-01–CN-02, AC-01, AC-03–AC-05.

- [x] **T010**: apps/cli/src/main.rs::run_cli_with_context! (pre-dispatch context capture, command dispatch, eligibility policy, and data-only EmitCompletedCommand submission), telemetry_completion_eligible, and commands::track::execute_with_error_chain; apps/cli/src/main.rs::command_identity_from_args; apps/cli-driver/src/telemetry.rs::TelemetryInput, TelemetryDriver::handle, and items_dir_from_args; libs/usecase/src/telemetry.rs::TelemetryEmitService::emit_completed, TelemetryEmitInteractor, TelemetryArchiveInteractor, and TelemetryAggregateInteractor; libs/infrastructure/src/telemetry/report_adapter.rs::FsTelemetryEmitDynamicAdapter::emit_active; apps/cli-composition/src/telemetry.rs::TelemetryCompositionRoot (wiring-only reference) — implement and regression-test the branch-bound completion lifecycle, existing telemetry.jsonl sink, archive timing, and composition boundaries. IN-01, IN-02, IN-04, OUT-01–OUT-04, CN-01–CN-02, AC-01, AC-03–AC-05.
