<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapError | error_type | reference | CatalogueLoaderFailed, ContractMapWriterFailed, EmptyCatalogue, LayerNotFound | 🔵 | 🟡 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMap | application_service | reference | fn execute(&self, cmd: &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapInteractor | interactor | modify | — | 🔵 | 🟡 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapOutput | dto | reference | — | 🔵 | 🟡 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapCommand | command | reference | — | 🔵 | 🔵 |

