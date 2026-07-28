<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExtractError | error_type | reference | Io | 🔵 | 🔵 |
| PersistentIndexLockError | error_type | reference | LockFailed | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodeFragmentExtractorAdapter | secondary_adapter | reference | impl CodeFragmentExtractorPort, impl Debug, impl Default | 🔵 | 🔵 |
| FastEmbedAdapter | secondary_adapter | reference | impl EmbeddingPort, impl Debug | 🔵 | 🔵 |
| FsTdddFeatureDeclarationAdapter | secondary_adapter | add | impl TdddBaselineFeatureDeclarationPort, impl TdddActualFeatureDeclarationPort, impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| LanceDbSemanticIndexAdapter | secondary_adapter | reference | impl SemanticIndexPort, impl Debug, impl Drop | 🔵 | 🔵 |
| NoopSemanticIndexPort | secondary_adapter | reference | impl SemanticIndexPort | 🔵 | 🔵 |
| NullInsertIndexProxy | secondary_adapter | reference | impl SemanticIndexPort | 🔵 | 🔵 |
| PersistentIndexLock | secondary_adapter | reference | — | 🔵 | 🔵 |
| RustdocBaselineCaptureAdapter | secondary_adapter | modify | impl RustdocBaselineCapturePort, impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| RustdocCrateAdapter | secondary_adapter | modify | impl RustdocCratePort | 🔵 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | modify | impl SchemaExporter, impl SchemaExporterPort | 🔵 | 🔵 |
| TypeSignalsExecutorAdapter | secondary_adapter | modify | impl TypeSignalsExecutorPort, impl Debug, impl Default | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::semantic_dup::extractor::extract_code_fragments | free_function | reference | fn(workspace_root: &std::path::Path) -> Result<Vec<domain::semantic_dup::CodeFragment>, ExtractError> | 🔵 | 🔵 |
| infrastructure::semantic_dup::null_insert_proxy::acquire_persistent_index_lock | free_function | reference | fn(db_path: &std::path::Path) -> Result<PersistentIndexLock, PersistentIndexLockError> | 🔵 | 🔵 |
| infrastructure::semantic_dup::null_insert_proxy::persistent_index_lock_path | free_function | reference | fn(db_path: &std::path::Path) -> std::path::PathBuf | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer | free_function | modify | fn(items_dir: &std::path::Path, track_id: &domain::TrackId, workspace_root: &std::path::Path, binding: &TdddLayerBinding, features: &[domain::tddd::CargoFeatureName]) -> Result<std::process::ExitCode, EvaluateSignalsError> | 🔵 | 🔵 |

