<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryConfig | dto | modify | — | 🔵 | 🔵 |
| TelemetryReportSnapshot | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsTelemetryEmitDynamicAdapter | secondary_adapter | modify | impl Default, impl TelemetryEmitDynamicPort | 🔵 | 🔵 |
| TelemetryReport | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::telemetry::context::resolve_telemetry_track_id | free_function | add | fn(items_dir: &std::path::Path) -> Option<String> | 🔵 | 🔵 |

