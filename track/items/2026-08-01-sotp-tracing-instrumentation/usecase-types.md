<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandDurationMillis | value_object | add | — | 🔵 | 🔵 |
| CommandExecutionCount | value_object | add | — | 🔵 | 🔵 |
| CommandFailureRateBasisPoints | value_object | add | — | 🔵 | 🔵 |
| SotpCommandIdentity | value_object | add | — | 🔵 | 🔵 |
| TelemetrySkippedLineCount | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandTraceValueError | error_type | add | EmptyCommandIdentity, ZeroExecutions, FailureCountExceedsExecutions, FailureRateOutOfRange | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryEmitDynamicPort | secondary_port | modify | fn emit_active(&self, items_dir: &std::path::Path, source_track_id: Option<&str>, subcommand: String, exit_code: i32, duration_ms: u64, error_chain: Option<String>) -> Result<(), TelemetryEmitDynamicPortError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryAggregateService | application_service | modify | — | 🔵 | 🔵 |
| TelemetryArchivedService | application_service | add | fn emit_archived(&self, items_dir: &std::path::Path, track_id: &str, subcommand: String, exit_code: i32, duration_ms: u64) -> Result<(), TelemetryAggregateServiceError> | 🔵 | 🔵 |
| TelemetryEmitService | application_service | add | fn emit_completed(&self, items_dir: &std::path::Path, source_track_id: Option<String>, subcommand: String, exit_code: i32, duration_ms: u64, error_chain: Option<String>) -> Result<(), TelemetryAggregateServiceError> | 🔵 | 🔵 |
| TelemetryReportService | application_service | add | fn report(&self, track_id: &str, items_dir: &std::path::Path) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryAggregateInteractor | interactor | modify | — | 🔵 | 🔵 |
| TelemetryArchiveInteractor | interactor | add | — | 🔵 | 🔵 |
| TelemetryEmitInteractor | interactor | modify | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandExecutionMetric | dto | add | — | 🔵 | 🔵 |
| TelemetryReportOutput | dto | modify | — | 🔵 | 🔵 |

