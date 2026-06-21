<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalLayerReaderError | error_type | — | Io, TrackIdUnresolved | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| LiveSoTChain | secondary_port | — | fn calc_live(input: &<Self>::Input<'_>) -> Result<<Self>::LiveCalc, <Self>::CalcError> | 🔵 | 🔵 |
| LoadablePersistedChain | secondary_port | — | fn calc(input: &<Self>::Input<'_>) -> Result<<Self>::Persisted, <Self>::CalcError>, fn load(input: &<Self>::Input<'_>) -> Result<<Self>::Persisted, <Self>::CalcError>, fn check_freshness(input: &<Self>::Input<'_>, persisted: &<Self>::Persisted) -> Result<(), <Self>::StaleError> | 🔵 | 🔵 |
| PersistedSoTChain | secondary_port | — | — | 🔵 | 🔵 |
| SignalLayerReader | secondary_port | — | fn active_track_id(&self) -> Result<domain::TrackId, SignalLayerReaderError>, fn enabled_layers(&self, track_id: domain::TrackId) -> Result<Vec<domain::tddd::LayerId>, SignalLayerReaderError>, fn catalogue_bytes(&self, track_id: domain::TrackId, layer: domain::tddd::LayerId) -> Result<Option<Vec<u8>>, SignalLayerReaderError> | 🔵 | 🔵 |
| SoTChain | secondary_port | — | fn check(input: &<Self>::Input<'_>, strictness: domain::Strictness) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| TrackBlobReader | secondary_port | modify | fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::spec::SpecDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(Vec<u8>, String)>, fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::ImplPlanDocument>, fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_catalogue_for_spec_ref_check(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<(domain::tddd::catalogue_v2::CatalogueDocument, String, std::collections::HashMap<String, domain::ContentHash>)>, fn read_catalogue_spec_signals_document(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument>, fn read_catalogue_spec_signal_opted_in_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_type_signals(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::TypeSignalsDocument>, fn read_adr_verify_report(&self, _branch: String) -> BlobFetchResult<domain::AdrVerifyReport> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyAdrSignals | application_service | reference | fn verify(&self, command: VerifyAdrSignalsCommand) -> Result<domain::AdrVerifyReport, VerifyAdrSignalsError> | 🔵 | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrUserChain | use_case | — | — | 🔵 | 🔵 |
| CatalogSpecChain | use_case | — | — | 🔵 | 🔵 |
| ImplCatalogChain | use_case | — | — | 🔵 | 🔵 |
| SpecAdrChain | use_case | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyAdrSignalsCommand | command | modify | — | 🔵 | 🔵 |

## Use Case Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::merge_gate::check_strict_merge_gate | use_case_function | modify | fn(branch: &str, reader: &R, gate_matrix: &domain::SignalGateMatrix) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::calc_catalog_spec | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::calc_impl_catalog | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::check_catalog_spec | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::check_impl_catalog | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::resolve_spec_json_path | use_case_function | — | fn(reader: &R, workspace_root: &std::path::Path, override_path: Option<std::path::PathBuf>) -> Result<std::path::PathBuf, SignalLayerReaderError> | 🔵 | 🔵 |

