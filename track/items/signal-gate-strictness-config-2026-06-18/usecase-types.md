<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalLayerReaderError | error_type | — | Io, TrackIdUnresolved | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalLayerReader | secondary_port | — | fn active_track_id(&self) -> Result<domain::TrackId, SignalLayerReaderError>, fn enabled_layers(&self, track_id: domain::TrackId) -> Result<Vec<domain::tddd::LayerId>, SignalLayerReaderError>, fn catalogue_bytes(&self, track_id: domain::TrackId, layer: domain::tddd::LayerId) -> Result<Option<Vec<u8>>, SignalLayerReaderError> | 🔵 | 🔵 |
| TrackBlobReader | secondary_port | modify | fn read_spec_document(&self, branch: String, track_id: domain::TrackId) -> BlobFetchResult<domain::spec::SpecDocument>, fn read_type_catalogue(&self, branch: String, track_id: domain::TrackId, layer: domain::tddd::LayerId) -> BlobFetchResult<Option<String>>, fn read_impl_plan(&self, branch: String, track_id: domain::TrackId) -> BlobFetchResult<domain::ImplPlanDocument>, fn read_enabled_layers(&self, branch: String, track_id: domain::TrackId) -> BlobFetchResult<Vec<domain::tddd::LayerId>>, fn read_catalogue_for_spec_ref_check(&self, branch: String, track_id: domain::TrackId, layer: domain::tddd::LayerId) -> BlobFetchResult<String>, fn read_catalogue_spec_signals_document(&self, branch: String, track_id: domain::TrackId, layer: domain::tddd::LayerId) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument>, fn read_catalogue_spec_signal_opted_in_layers(&self, branch: String, track_id: domain::TrackId) -> BlobFetchResult<Vec<domain::tddd::LayerId>>, fn read_type_signals(&self, branch: String, track_id: domain::TrackId, layer: domain::tddd::LayerId) -> BlobFetchResult<domain::TypeSignalsDocument>, fn read_adr_verify_report(&self, branch: String) -> BlobFetchResult<domain::AdrVerifyReport> | 🟡 | 🔵 |

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
| usecase::merge_gate::check_strict_merge_gate | use_case_function | modify | fn(branch: String, reader: R, gate_matrix: domain::SignalGateMatrix) -> domain::verify::VerifyOutcome | 🟡 | 🔵 |
| usecase::signal::calc_catalog_spec | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::calc_impl_catalog | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::check_catalog_spec | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| usecase::signal::check_impl_catalog | use_case_function | — | fn(reader: &R, per_layer_fn: F) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |

