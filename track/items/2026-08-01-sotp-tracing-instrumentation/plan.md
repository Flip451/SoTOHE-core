<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Sotp Tracing Instrumentation

## Summary

GO-01 → T001, T002, T003, T004, T005, T006, T007, T008, T009, T010.

## Tasks (3/10 resolved)

### S1 — Typed command-trace application model

> Establish validated values, the recording usecase boundary, and typed aggregation output. IN-01, IN-02, IN-03, CN-01, AC-01, AC-02.

- [x] **T001**: Add `libs/usecase/src/telemetry/command_trace.rs` with `SotpCommandIdentity`, `CommandDurationMillis`, `CommandExitCode`, `CommandExecutionResult`, `CommandTraceRecord`, and `CommandTraceValueError`, including their validating constructors and value tests. IN-01, IN-02, CN-01, AC-01, AC-03. (`9f1fda8a075c52709e99a2bee44e4f698bed3bf2`)
- [x] **T002**: Add `CommandTraceService`, `CommandTraceWriterPort`, `CommandTraceInteractor`, and `CommandTraceWriteError` in `libs/usecase/src/telemetry/command_trace.rs`, including recording-flow result and port-contract tests. IN-01, IN-02, CN-01, CN-03, AC-01, AC-03. (`f83e49e3be1d5981ba9810f9a69ae308adfc9251`)
- [x] **T003**: Extend `TelemetryAggregateInteractor` and the usecase telemetry output with typed per-command execution metrics, bounded failure-rate data, and fail-open malformed- and unknown-schema-record skipped-count accounting, with focused aggregation-conversion tests. IN-03, CN-01, CN-04, AC-02, AC-03.

### S2 — Local JSONL persistence and rotation

> First persist command traces locally, then add pre-append rotation and bounded retention with focused AC-04 verification. IN-02, IN-04, OUT-01, CN-01, CN-02, AC-01, AC-03, AC-04.

- [ ] **T004**: Implement the `FsCommandTraceAdapter` basic local JSONL persistence boundary and its record-format success/failure tests before adding rotation behavior. IN-02, OUT-01, CN-01, AC-01, AC-03.
- [ ] **T005**: Add the typed rotation policy and extend `FsCommandTraceAdapter` with pre-append size rotation and oldest-first retained-file deletion; add focused AC-04 tests that prove active-file bounds and retained-file limits. IN-02, IN-04, OUT-01, CN-01, CN-02, AC-01, AC-03, AC-04.

### S3 — Telemetry aggregation and display

> Replace the infrastructure report DTO with its snapshot model, convert persisted command records through the existing aggregation path, and render the resulting metrics through the telemetry command. IN-03, OUT-01, CN-01, AC-02, AC-03.

- [ ] **T006**: Delete the old infrastructure `TelemetryReportOutput`, modify `TelemetryReport::aggregate`, add the layer-unique `TelemetryReportSnapshot` read model, and update `FsTelemetryReportAdapter` parser-to-usecase conversion for persisted command metrics and fail-open malformed- and unknown-schema-record skip reporting, with focused aggregation-path tests. IN-03, OUT-01, CN-01, CN-04, AC-02, AC-03.
- [ ] **T009**: Extend `TelemetryDriver::telemetry_report` and `apps/cli/src/commands/telemetry.rs::execute` to render command metrics from the existing aggregation path; add focused AC-02 CLI regression tests for frequency, duration, and failure rate. IN-03, CN-01, AC-02, AC-03.

### S4 — Tracing composition

> Build the primary driver and wire the complete local tracing dependency graph before entrypoint use. IN-01, IN-02, IN-04, OUT-01, CN-01, CN-02, AC-01, AC-03, AC-04.

- [ ] **T007**: Implement the primary `CommandTraceDriver` that submits a typed completed-command record to the usecase service and maps the result to the existing command outcome. IN-01, IN-02, CN-01, AC-01, AC-03.
- [ ] **T008**: Implement `CommandTraceCompositionRoot` to validate rotation defaults and wire the filesystem adapter, usecase flow, and primary driver before any entrypoint invokes it. IN-02, IN-04, OUT-01, CN-01, CN-02, AC-01, AC-03, AC-04.

### S5 — Cross-command entrypoint integration

> `apps/cli/src/main.rs::run_cli_with`: construct and invoke `CommandTraceCompositionRoot::command_trace_driver`; add success/failure regression tests. IN-01, IN-02, OUT-02, CN-01, AC-01, AC-03.

- [ ] **T010**: Update `apps/cli/src/main.rs::run_cli_with` to construct and invoke `CommandTraceCompositionRoot::command_trace_driver`; add focused `run_cli_with` success and failure regression tests. IN-01, IN-02, OUT-02, CN-01, AC-01, AC-03.
