<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryEvent | enum | reference | TrackSubcommand, GateEval, ReviewRound, ExternalSubprocess, HookBlock, AdvisoryHookFired, NonZeroExit | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfigError | error_type | modify | Io, Parse, UnsupportedSchemaVersion, InvalidThreshold, InvalidParallelism, InvalidReasoningEffort | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfig | dto | modify | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexDryChecker | secondary_adapter | modify | impl Debug, impl DryCheckAgentPort | 🟡 | 🔵 |
| FsDryCheckCoverageAdapter | secondary_adapter | — | impl DryCheckCoveragePort, impl Debug | 🟡 | 🔵 |

