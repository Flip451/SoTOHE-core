<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanCodecError | error_type | add | InvalidJson, UnsupportedSchemaVersion, InvalidDocument | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchDeclarationDto | dto | add | — | 🔵 | 🔵 |
| BatchPlanDocumentDto | dto | add | — | 🔵 | 🔵 |
| ImplPlanTaskDto | dto | modify | — | 🔵 | 🔵 |
| ScopeLineEstimateDto | dto | add | — | 🔵 | 🔵 |
| TaskEstimateDto | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsBatchPlanReader | secondary_adapter | add | impl Debug, impl Default, impl BatchPlanReaderPort | 🟡 | 🔵 |
| FsPlannedTaskReader | secondary_adapter | add | impl Debug, impl Default, impl PlannedTaskReaderPort | 🟡 | 🔵 |
| FsReviewScopeConfigReader | secondary_adapter | add | impl Debug, impl Default, impl ScopeConfigReaderPort | 🟡 | 🔵 |
| GitScopeDiffMeasurer | secondary_adapter | add | impl Default, impl Debug, impl ScopeDiffMeasurePort | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::batch_plan_codec::decode | free_function | add | fn(json: &str) -> Result<domain::batch_plan::BatchPlanDocument, BatchPlanCodecError> | 🔵 | 🔵 |

