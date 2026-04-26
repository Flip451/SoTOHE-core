<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapError | error_type | reference | CatalogueLoaderFailed, ContractMapWriterFailed, EmptyCatalogue, LayerNotFound | 🔵 | 🔵 |
| RefreshCatalogueSpecSignalsError | error_type | reference | NonActiveTrack, BranchTrackMismatch, CatalogueNotFound, FetchError, InvalidCatalogueHash, WriteFailed | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMap | application_service | reference | fn execute(&self, cmd: &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError> | 🔵 | 🔵 |
| RefreshCatalogueSpecSignals | application_service | reference | fn execute(&self, cmd: &RefreshCatalogueSpecSignalsCommand) -> Result<(), RefreshCatalogueSpecSignalsError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapInteractor | interactor | reference | — | 🔵 | 🔵 |
| RefreshCatalogueSpecSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapOutput | dto | reference | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapCommand | command | reference | — | 🔵 | 🔵 |
| RefreshCatalogueSpecSignalsCommand | command | reference | — | 🔵 | 🔵 |

