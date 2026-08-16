<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryEvent | enum | modify | TrackSubcommand, GateEval, ReviewRound, ExternalSubprocess, HookBlock, AdvisoryHookFired, NonZeroExit | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| StructuredReviewRoundDto | dto | add | — | 🔵 | 🔵 |
| TelemetryReportSnapshot | dto | modify | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ClaudeReviewer | secondary_adapter | modify | impl Reviewer, impl ResolvedReviewer | 🔵 | 🔵 |
| CodexReviewer | secondary_adapter | modify | impl Reviewer, impl ResolvedReviewer | 🔵 | 🔵 |
| ReviewYieldRecordingReviewer | secondary_adapter | add | impl Reviewer | 🔵 | 🔵 |
| TelemetryWriter | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |

