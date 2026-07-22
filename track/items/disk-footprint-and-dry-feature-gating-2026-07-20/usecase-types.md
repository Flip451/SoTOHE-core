<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiskMaintenanceCommand | enum | add | ConfigureSccache, ApplyCleanup | 🔵 | 🔵 |
| DiskMaintenanceCommandResponse | enum | add | SccacheConfigured, CleanupApplied | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiskMaintenanceError | error_type | add | Validation, Operation | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiskMaintenanceCommandPort | secondary_port | add | fn configure_sccache(&self, project_root: &std::path::Path) -> Result<domain::disk_maintenance::CacheSize, DiskMaintenanceError>, fn apply_cleanup(&self, project_root: std::path::PathBuf) -> Result<domain::disk_maintenance::CleanupScopeSet, DiskMaintenanceError> | 🔵 | 🔵 |
| DiskMaintenanceQueryPort | secondary_port | add | fn plan_cleanup(&self, project_root: &std::path::Path) -> Result<domain::disk_maintenance::CleanupScopeSet, DiskMaintenanceError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiskMaintenanceCommandService | application_service | add | fn execute_command(&self, command: DiskMaintenanceCommand) -> Result<DiskMaintenanceCommandResponse, DiskMaintenanceError> | 🔵 | 🔵 |
| DiskMaintenanceQueryService | application_service | add | fn plan_cleanup(&self, query: CleanupPlanQuery) -> Result<CleanupPlanResponse, DiskMaintenanceError> | 🔵 | 🔵 |
| DryCheckApprovedDriverService | application_service | reference | fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiskMaintenanceCommandInteractor | interactor | add | — | 🔵 | 🔵 |
| DiskMaintenanceQueryInteractor | interactor | add | — | 🔵 | 🔵 |
| FeatureDisabledDryGateInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CleanupPlanResponse | dto | add | — | 🔵 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CleanupPlanQuery | query | add | — | 🔵 | 🔵 |

