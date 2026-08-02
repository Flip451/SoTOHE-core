<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Sotp Tracing Instrumentation

## Summary

GO-01 → T001, T002, T003, T004, T005, T006, T007, T008, T009, T010.

## Tasks (7/10 resolved)

### S1 — Typed command-trace application model

> Establish validated values, the recording usecase boundary, and typed aggregation output. IN-01, IN-02, IN-03, CN-01, CN-03, AC-01, AC-02.

- [x] **T001**: Add `libs/usecase/src/telemetry/command_trace.rs` with `SotpCommandIdentity`, `CommandDurationMillis`, `CommandExitCode`, `CommandExecutionResult`, `CommandTraceRecord`, and `CommandTraceValueError`, including their validating constructors and value tests. IN-01, IN-02, CN-01, AC-01, AC-03. (`9f1fda8a075c52709e99a2bee44e4f698bed3bf2`)
- [x] **T002**: Add `CommandTraceService`, `CommandTraceWriterPort`, `CommandTraceInteractor`, and `CommandTraceWriteError` in `libs/usecase/src/telemetry/command_trace.rs`, including recording-flow result and port-contract tests. IN-01, IN-02, CN-01, CN-03, AC-01, AC-03. (`f83e49e3be1d5981ba9810f9a69ae308adfc9251`)
- [x] **T003**: Extend `TelemetryAggregateInteractor` and the usecase telemetry output with typed per-command execution metrics, bounded failure-rate data, and fail-open malformed- and unknown-schema-record skipped-count accounting, with focused aggregation-conversion tests. IN-03, CN-01, CN-04, AC-02, AC-03. (`88362dca5c29c4038c95d8d9ad2657a677e7f186`)

### S2 — Typed local trace-rotation policy

> Construct and validate the positive filesystem rotation bounds before adapter work. IN-04, CN-02.

- [x] **T004**: Implement validated rotation-policy construction only: `CommandTraceFileSizeLimitBytes`, `CommandTraceRetainedFileCount`, `CommandTracePolicyError`, and `CommandTraceRotationPolicy`, with positive-bound and policy-construction tests. IN-04, CN-02. (`1402599b252c80d0a20221852e0f8498d1f1fc88`)

### S3 — Atomic local JSONL persistence and rotation

> Complete the terminal filesystem writer contract using the validated policy: local persistence, typed failures, pre-append rotation, bounded retention, and focused verification. IN-02, IN-04, OUT-01, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04.

- [x] **T005**: Complete the terminal `FsCommandTraceAdapter` as the local JSONL `CommandTraceWriterPort` implementation, building on the uncommitted append-only adapter: typed write-failure propagation, JSONL persistence, pre-append rotation, and oldest-first retention, with record-format, write-failure, AC-01, and AC-04 tests. IN-02, IN-04, OUT-01, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04. (`faf97afdf260eecce2ecaa30487c1e5f5e1e120f`)

### S4 — Telemetry aggregation and display

> Replace the infrastructure report DTO with its snapshot model, convert persisted command records through the existing aggregation path, and render the resulting metrics through the telemetry command. IN-03, OUT-01, CN-01, CN-04, AC-02, AC-03.

- [x] **T006**: Delete the old infrastructure `TelemetryReportOutput`, modify `TelemetryReport::aggregate`, add the layer-unique `TelemetryReportSnapshot` read model, and update `FsTelemetryReportAdapter` parser-to-usecase conversion for persisted command metrics and fail-open malformed- and unknown-schema-record skip reporting, with focused aggregation-path tests. IN-03, OUT-01, CN-01, CN-04, AC-02, AC-03. (`1dc7b80fca414e70b70705d5313ffec72ceefed6`)
- [ ] **T009**: Extend `TelemetryDriver::telemetry_report` and `apps/cli/src/commands/telemetry.rs::execute` to render command metrics from the existing aggregation path; add focused AC-02 CLI regression tests for frequency, duration, and failure rate. IN-03, CN-01, AC-02, AC-03.

### S5 — Tracing composition

> Build the primary driver and wire the complete local tracing dependency graph before entrypoint use. IN-02, IN-04, OUT-01, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04.

- [x] **T007**: Update `CommandTraceDriver::handle(outcome, record) -> CommandOutcome` to invoke local trace recording and render trace diagnostics; add focused success/failure regression tests. IN-02, AC-01, AC-03.
- [ ] **T008**: Implement `CommandTraceCompositionRoot` to validate rotation defaults and wire the complete filesystem adapter, usecase flow, and primary driver before any entrypoint invokes it. IN-02, IN-04, OUT-01, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04.

### S6 — Cross-command entrypoint integration

> `apps/cli/src/main.rs::run_cli_with`: construct and invoke `CommandTraceCompositionRoot::command_trace_driver`; add success/failure regression tests. IN-01, IN-02, OUT-02, CN-01, CN-03, AC-01, AC-03.

- [ ] **T010**: Update `apps/cli/src/main.rs::run_cli_with` to construct and invoke `CommandTraceCompositionRoot::command_trace_driver`; add focused `run_cli_with` success and failure regression tests. IN-01, IN-02, OUT-02, CN-01, CN-03, AC-01, AC-03.
