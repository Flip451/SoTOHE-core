<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RenderContractMapError | error_type | — | CatalogueLoaderFailed, ContractMapWriterFailed, EmptyCatalogue, LayerNotFound | 🟡 |

## Application Services

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RenderContractMap | application_service | — | fn execute(&self, cmd: &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError> | 🟡 |

## Interactors

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RenderContractMapInteractor | interactor | — | — | 🟡 |

## DTOs

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RenderContractMapOutput | dto | — | — | 🟡 |

## Commands

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RenderContractMapCommand | command | — | — | 🟡 |

