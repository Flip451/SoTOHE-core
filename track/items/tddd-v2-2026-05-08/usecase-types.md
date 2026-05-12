<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueImplSignalsError | error_type | — | InvalidTrackId, LayerBindingsLoad, CatalogueLoad, BaselineLoad, ExtendedCrateConversion, SchemaExport, Evaluation, SymlinkRejected, NoLayers | 🔵 | 🔵 |
| RenderContractMapError | error_type | modify | CatalogueLoaderFailed, ContractMapWriterFailed, EmptyCatalogue, LayerNotFound, InvalidTrackId | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecSignalsWriter | secondary_port | reference | fn write_catalogue_spec_signals(&self, track_id: &TrackId, layer_id: &str, doc: &CatalogueSpecSignalsDocument) -> Result<(), RepositoryError> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, String> | 🔵 | 🔵 |
| SpecElementHashReader | secondary_port | reference | fn read_spec_element_hashes(&self, branch: &str, track_id: &str) -> BlobFetchResult<BTreeMap<SpecElementId, ContentHash>> | 🔵 | 🔵 |
| TrackBlobReader | secondary_port | modify | fn read_type_signals(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<domain::TypeSignalsDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(Vec<u8>, String)>, fn read_catalogue_for_spec_ref_check(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(domain::tddd::catalogue_v2::CatalogueDocument, String)> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueImplSignalsService | application_service | — | fn run(&self, track_id: String, workspace_root: std::path::PathBuf, layer: Option<String>) -> Result<String, CatalogueImplSignalsError> | 🔵 | 🔵 |
| PreCommitTypeSignalsService | application_service | reference | fn run(&self, track_id: String, workspace_root: PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueImplSignalsInteractor | interactor | — | — | 🔵 | 🔵 |
| PreCommitTypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapCommand | command | modify | — | 🔵 | 🔵 |

## Use Case Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::merge_gate::check_strict_merge_gate | use_case_function | modify | fn(branch: &str) -> bool | 🟡 | 🔵 |

