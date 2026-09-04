<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::canonical_type_identity::CanonicalTypeIdentity | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::type_signals_codec::TypeSignalsCodecError | error_type | modify | Json, UnsupportedSchemaVersion, InvalidSchemaVersion, InvalidTimestamp, InvalidDigest, InvalidSignal, InvalidNamespace | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::catalog_gen::FsCatalogAdapter | secondary_adapter | modify | — | 🔵 | 🔵 |
| infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec | secondary_adapter | modify | — | 🔵 | 🔵 |
| infrastructure::tddd::signal_evaluator_v2::SignalEvaluatorV2 | secondary_adapter | modify | — | 🔵 | 🔵 |
| infrastructure::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader | secondary_adapter | modify | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::repository_root_for_items_dir | free_function | add | fn(items_dir: &std::path::Path) -> Result<std::path::PathBuf, std::io::error::Error> | 🔵 | 🔵 |
| infrastructure::tddd::canonical_type_identity::canonicalize_catalogue_type_ref | free_function | modify | fn(type_ref: &domain::tddd::catalogue_v2::identifiers::TypeRef, catalogue_crate: &domain::tddd::catalogue_v2::identifiers::CrateName, rustdoc_paths: &std::collections::HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>, generic_params: &[domain::tddd::catalogue_v2::identifiers::ParamName]) -> Result<CanonicalTypeIdentity, domain::tddd::new_typegraph_codec_error::NewTypeGraphCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer | free_function | modify | fn(items_dir: &std::path::Path, track_id: &domain::ids::TrackId, workspace_root: &std::path::Path, binding: &TdddLayerBinding, features: &[domain::tddd::feature_declaration::CargoFeatureName]) -> Result<std::process::ExitCode, EvaluateSignalsError> | 🔵 | 🔵 |

