<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderBaselineGraphError | error_type | — | LoaderFailed, WriterFailed, EmptyBaseline, LayerNotFound, RendererFailed | 🔵 | 🔵 |
| RenderContractMapError | error_type | modify | CatalogueLoaderFailed, ContractMapWriterFailed, EmptyCatalogue, LayerNotFound, RendererFailed | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderBaselineGraph | application_service | — | fn execute(&self, cmd: &RenderBaselineGraphCommand) -> Result<RenderBaselineGraphOutput, RenderBaselineGraphError> | 🔵 | 🔵 |
| RenderContractMap | application_service | reference | fn execute(&self, cmd: &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderBaselineGraphInteractor | interactor | — | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderBaselineGraphOutput | dto | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderBaselineGraphCommand | command | — | — | 🔵 | 🔵 |
| RenderContractMapCommand | command | modify | — | 🔵 | 🔵 |

