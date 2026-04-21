<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Ports

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| TrackBlobReader | secondary_port | modify | fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(TypeCatalogueDocument, String)>, fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<ImplPlanDocument>, fn read_enabled_layers(&self, branch: &str) -> BlobFetchResult<Vec<String>> | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| AddTaskUseCase | use_case | modify | — | 🔵 |
| TransitionTaskUseCase | use_case | modify | — | 🔵 |

