<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EvaluateSignalsError | error_type | modify | AuthoritativeInput, Evaluation, CacheWrite | 🔵 | 🔵 |
| TypeSignalsCodecError | error_type | modify | Json, UnsupportedSchemaVersion, InvalidSchemaVersion, InvalidTimestamp, InvalidDigest, InvalidSignal | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsBaseMergeCleanupAdapter | secondary_adapter | add | impl BaseMergeCleanupPort | 🔵 | 🔵 |
| FsBaseMergeContextAdapter | secondary_adapter | add | impl BaseMergeContextPort | 🔵 | 🔵 |
| FsBaseMergeGitAdapter | secondary_adapter | add | impl BaseMergeGitPort | 🔵 | 🔵 |
| FsGitStashAdapter | secondary_adapter | add | impl GitStashPort, impl Default | 🔵 | 🔵 |
| TypeSignalsExecutorAdapter | secondary_adapter | reference | impl Debug, impl Default, impl TypeSignalsExecutorPort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::type_signals_codec::decode | free_function | reference | fn(json: &str) -> Result<domain::tddd::type_signals_doc::TypeSignalsDocument, TypeSignalsCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_codec::encode | free_function | reference | fn(doc: &domain::tddd::type_signals_doc::TypeSignalsDocument) -> Result<String, TypeSignalsCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer | free_function | modify | fn(items_dir: &std::path::Path, track_id: &domain::TrackId, workspace_root: &std::path::Path, binding: &TdddLayerBinding, features: &[domain::tddd::CargoFeatureName]) -> Result<std::process::ExitCode, EvaluateSignalsError> | 🔵 | 🔵 |

