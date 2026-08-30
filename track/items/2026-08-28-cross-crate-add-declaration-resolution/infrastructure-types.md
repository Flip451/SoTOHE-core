<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::type_signals_codec::TypeSignalsCodecError | error_type | modify | Json, UnsupportedSchemaVersion, InvalidSchemaVersion, InvalidTimestamp, InvalidDigest, InvalidSignal, InvalidNamespace, InvalidExecutionIdentity | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::EvaluateSignalsError | error_type | modify | AuthoritativeInput, Evaluation, CacheWrite | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::schema_export::RustdocSchemaExporter | secondary_adapter | modify | — | 🔵 | 🔵 |
| infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec | secondary_adapter | modify | — | 🔵 | 🔵 |
| infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter | secondary_adapter | modify | — | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter | secondary_adapter | modify | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer | free_function | modify | fn(items_dir: &std::path::Path, track_id: &domain::ids::TrackId, workspace_root: &std::path::Path, binding: &crate::verify::tddd_layers::TdddLayerBinding, features: &[domain::tddd::feature_declaration::CargoFeatureName]) -> Result<std::process::ExitCode, EvaluateSignalsError> | 🔵 | 🔵 |

