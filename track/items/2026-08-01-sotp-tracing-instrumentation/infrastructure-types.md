<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandTracePolicyError | error_type | add | ZeroFileSizeLimit, ZeroRetainedFileCount | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandTraceFileSizeLimitBytes | dto | add | — | 🔵 | 🔵 |
| CommandTraceRetainedFileCount | dto | add | — | 🔵 | 🔵 |
| CommandTraceRotationPolicy | dto | add | — | 🔵 | 🔵 |
| TelemetryReportSnapshot | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsCommandTraceAdapter | secondary_adapter | add | impl CommandTraceWriterPort, impl Debug | 🔵 | 🔵 |
| TelemetryReport | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |

