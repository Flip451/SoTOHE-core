<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RefreshCatalogueSpecSignalsError | error_type | — | NonActiveTrack, BranchTrackMismatch, CatalogueNotFound, FetchError, InvalidCatalogueHash, WriteFailed | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| TrackBlobReader | secondary_port | modify | fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(TypeCatalogueDocument, String)>, fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<ImplPlanDocument>, fn read_enabled_layers(&self, branch: &str) -> BlobFetchResult<Vec<String>>, fn read_catalogue_for_spec_ref_check(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(TypeCatalogueDocument, String)>, fn read_catalogue_spec_signals_document(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<CatalogueSpecSignalsDocument> | 🔵 |
| CatalogueSpecSignalsWriter | secondary_port | — | fn write_catalogue_spec_signals(&self, track_id: &TrackId, layer_id: &str, doc: &CatalogueSpecSignalsDocument) -> Result<(), RepositoryError> | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RefreshCatalogueSpecSignals | application_service | — | fn execute(&self, cmd: &RefreshCatalogueSpecSignalsCommand) -> Result<(), RefreshCatalogueSpecSignalsError> | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RefreshCatalogueSpecSignalsInteractor | interactor | — | — | 🔵 |
| VerifyCatalogueSpecRefsInteractor | interactor | — | — | 🟡 |

## Commands

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| RefreshCatalogueSpecSignalsCommand | command | — | — | 🔵 |

