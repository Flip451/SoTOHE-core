<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyCacheScope | enum | — | SpecAdr, CatalogueSpec | 🔵 | 🔵 |
| RefVerifyScope | enum | — | Chain1, Chain2, All | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueEntryRef | value_object | — | — | 🔵 | 🔵 |
| RefVerifyConfig | value_object | — | — | 🔵 | 🔵 |
| RefVerifyPair | value_object | — | — | 🔵 | 🔵 |
| RefVerifyParallelism | value_object | — | — | 🔵 | 🔵 |
| RefVerifyPercent | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyError | error_type | — | InvalidConfig, TrackNotActive, VerifierPort, CachePersistence, SemanticFailuresConfirmed, HumanEscalationRequired | 🔵 | 🔵 |
| RefreshCatalogueSpecSignalsError | error_type | modify | NonActiveTrack, BranchTrackMismatch, CatalogueNotFound, FetchError, MissingEntryHash, InvalidCatalogueHash, WriteFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifierPort | secondary_port | — | fn verify_pair(&self, claim: String, evidence: String, cache_scope: &RefVerifyCacheScope, tier: domain::tddd::semantic_verify::ModelTier) -> Result<domain::tddd::semantic_verify::SemanticVerdict, RefVerifyError> | 🔵 | 🔵 |
| RefVerifyCachePort | secondary_port | — | fn load_entries(&self, cmd: &RefVerifyCommand, cache_scope: &RefVerifyCacheScope) -> Result<Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>, RefVerifyError>, fn save_entries(&self, cmd: &RefVerifyCommand, cache_scope: &RefVerifyCacheScope, entries: Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>) -> Result<(), RefVerifyError> | 🔵 | 🔵 |
| RefVerifyPairSourcePort | secondary_port | — | fn load_pairs(&self, cmd: &RefVerifyCommand, config: &RefVerifyConfig) -> Result<Vec<RefVerifyPair>, RefVerifyError> | 🔵 | 🔵 |
| TrackBlobReader | secondary_port | modify | fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::spec::SpecDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(Vec<u8>, String)>, fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::ImplPlanDocument>, fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_catalogue_for_spec_ref_check(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<(domain::tddd::catalogue_v2::CatalogueDocument, String, std::collections::HashMap<String, domain::ContentHash>)>, fn read_catalogue_spec_signals_document(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument>, fn read_catalogue_spec_signal_opted_in_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_type_signals(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::TypeSignalsDocument> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyApplicationService | application_service | — | fn execute(&self, cmd: &RefVerifyCommand) -> Result<(), RefVerifyError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifySemanticRefsInteractor | interactor | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyCommand | command | — | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::catalogue_traversal::iter_catalogue_entries | free_function | — | fn(catalogue: &domain::tddd::catalogue_v2::CatalogueDocument) -> impl Iterator<Item = CatalogueEntryRef<'_>> | 🔵 | 🔵 |

