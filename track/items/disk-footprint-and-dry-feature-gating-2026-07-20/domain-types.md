<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CleanupExecutionMode | enum | add | DryRun, Apply | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CacheSize | value_object | add | — | 🔵 | 🔵 |
| CleanupScope | value_object | add | — | 🔵 | 🔵 |
| CleanupScopeSet | value_object | add | — | 🔵 | 🔵 |
| DiskMaintenanceConfig | value_object | add | — | 🔵 | 🔵 |
| DiskMaintenanceOperationDetail | value_object | add | — | 🔵 | 🔵 |
| InvalidDiskMaintenanceInput | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiskMaintenanceValidationError | error_type | add | InvalidCacheSize, InvalidCleanupScope, EmptyCleanupScopes, DuplicateCleanupScope | 🔵 | 🔵 |

