<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryEvent | enum | — | TrackSubcommand, GateEval, ReviewRound, ExternalSubprocess, HookBlock, AdvisoryHookFired, NonZeroExit | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryReportError | error_type | — | Io, TrackNotFound | 🟡 | 🔵 |
| TelemetryWriteError | error_type | — | Serialize, Io | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseDurationSummary | dto | — | — | 🟡 | 🔵 |
| TelemetryConfig | dto | — | — | 🟡 | 🔵 |
| TelemetryErrorEntry | dto | — | — | 🟡 | 🔵 |
| TelemetryHookBlockEntry | dto | — | — | 🟡 | 🔵 |
| TelemetryReportOutput | dto | — | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryReport | secondary_adapter | — | impl Debug | 🟡 | 🔵 |
| TelemetryWriter | secondary_adapter | — | impl Debug | 🟡 | 🔵 |

