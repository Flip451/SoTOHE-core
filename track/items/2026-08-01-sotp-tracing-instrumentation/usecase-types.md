<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandExecutionResult | enum | add | Success, Failure | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandDurationMillis | value_object | add | — | 🔵 | 🔵 |
| CommandExecutionCount | value_object | add | — | 🔵 | 🔵 |
| CommandExitCode | value_object | add | — | 🔵 | 🔵 |
| CommandFailureRateBasisPoints | value_object | add | — | 🔵 | 🔵 |
| SotpCommandIdentity | value_object | add | — | 🔵 | 🔵 |
| TelemetrySkippedLineCount | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandTraceValueError | error_type | add | EmptyCommandIdentity, ZeroExitCode, ZeroExecutions, FailureCountExceedsExecutions, FailureRateOutOfRange | 🔵 | 🔵 |
| CommandTraceWriteError | error_type | add | Unavailable | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandTraceWriterPort | secondary_port | add | fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandTraceService | application_service | add | fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandTraceInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandExecutionMetric | dto | add | — | 🔵 | 🔵 |
| CommandTraceRecord | dto | add | — | 🔵 | 🔵 |
| TelemetryReportOutput | dto | modify | — | 🔵 | 🔵 |

