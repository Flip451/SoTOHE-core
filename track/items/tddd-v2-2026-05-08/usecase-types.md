<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureError | error_type | — | InvalidTrackId, SymlinkRejected, LayerBindingsLoad, NoLayers, CaptureFailed | 🔵 | 🟡 |
| CatalogueImplSignalsError | error_type | — | InvalidTrackId, LayerBindingsLoad, CatalogueLoad, BaselineLoad, ExtendedCrateConversion, SchemaExport, Evaluation, SymlinkRejected, NoLayers | 🔵 | 🔵 |
| RenderContractMapError | error_type | modify | CatalogueLoaderFailed, ContractMapWriterFailed, EmptyCatalogue, LayerNotFound, InvalidTrackId | 🔵 | 🔵 |
| TypeSignalsError | error_type | — | InvalidTrackId, StatusReadFailed, TrackFrozen, LayerBindingsLoad, NoLayers, EvaluationFailed | 🟡 | 🟡 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecSignalsWriter | secondary_port | reference | fn write_catalogue_spec_signals(&self, track_id: &TrackId, layer_id: &str, doc: &CatalogueSpecSignalsDocument) -> Result<(), RepositoryError> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, String> | 🔵 | 🔵 |
| SpecElementHashReader | secondary_port | reference | fn read_spec_element_hashes(&self, branch: &str, track_id: &str) -> BlobFetchResult<BTreeMap<SpecElementId, ContentHash>> | 🔵 | 🔵 |
| TrackBlobReader | secondary_port | modify | fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::spec::SpecDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(Vec<u8>, String)>, fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::ImplPlanDocument>, fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_catalogue_for_spec_ref_check(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<(domain::tddd::catalogue_v2::CatalogueDocument, String)>, fn read_catalogue_spec_signals_document(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument>, fn read_catalogue_spec_signal_opted_in_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_type_signals(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::TypeSignalsDocument> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureService | application_service | — | fn run(&self, request: BaselineCaptureRequest) -> Result<(), BaselineCaptureError> | 🔵 | 🟡 |
| CatalogueImplSignalsService | application_service | — | fn run(&self, track_id: String, workspace_root: std::path::PathBuf, layer: Option<String>) -> Result<CatalogueImplSignalsReport, CatalogueImplSignalsError> | 🔵 | 🔵 |
| PreCommitTypeSignalsService | application_service | reference | fn run(&self, track_id: String, workspace_root: PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError> | 🔵 | 🔵 |
| ReviewCheckApprovedService | application_service | reference | fn check_approved(&self, track_id: String, items_dir: std::path::PathBuf) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> | 🔵 | 🔵 |
| TaskOperationService | application_service | reference | fn transition_task(&self, cmd: TaskTransitionCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn add_task(&self, cmd: AddTaskCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn set_override(&self, cmd: SetOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn clear_override(&self, cmd: ClearOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError> | 🔵 | 🔵 |
| TypeSignalsService | application_service | — | fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError> | 🔵 | 🟡 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureInteractor | interactor | — | — | 🔵 | 🟡 |
| CatalogueImplSignalsInteractor | interactor | — | — | 🔵 | 🔵 |
| PreCommitTypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |
| ReviewCheckApprovedInteractor | interactor | modify | — | 🟡 | 🟡 |
| TaskOperationInteractor | interactor | modify | — | 🟡 | 🟡 |
| TypeSignalsInteractor | interactor | — | — | 🔵 | 🟡 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueImplSignalsReport | dto | — | — | 🔵 | 🟡 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureRequest | command | — | — | 🔵 | 🟡 |
| RenderContractMapCommand | command | modify | — | 🔵 | 🔵 |
| TypeSignalsRequest | command | — | — | 🔵 | 🟡 |

## Use Case Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::merge_gate::check_strict_merge_gate | use_case_function | modify | fn(branch: &str, reader: &R) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |

