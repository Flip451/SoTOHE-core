<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExtractError | error_type | add | Io | 🟡 | 🔵 |
| PersistentIndexLockError | error_type | add | LockFailed | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodeFragmentExtractorAdapter | secondary_adapter | add | impl CodeFragmentExtractorPort, impl Debug, impl Default | 🟡 | 🔵 |
| FastEmbedAdapter | secondary_adapter | add | impl EmbeddingPort, impl Debug | 🟡 | 🔵 |
| FsTdddFeatureDeclarationAdapter | secondary_adapter | add | impl TdddBaselineFeatureDeclarationPort, impl TdddActualFeatureDeclarationPort, impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| LanceDbSemanticIndexAdapter | secondary_adapter | add | impl SemanticIndexPort, impl Debug, impl Drop | 🟡 | 🔵 |
| NoopSemanticIndexPort | secondary_adapter | add | impl SemanticIndexPort | 🟡 | 🔵 |
| NullInsertIndexProxy | secondary_adapter | add | impl SemanticIndexPort | 🟡 | 🔵 |
| PersistentIndexLock | secondary_adapter | add | — | 🟡 | 🔵 |
| RustdocBaselineCaptureAdapter | secondary_adapter | modify | impl RustdocBaselineCapturePort, impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| RustdocCrateAdapter | secondary_adapter | modify | impl RustdocCratePort | 🔵 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | modify | impl SchemaExporter, impl SchemaExporterPort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::semantic_dup::extractor::extract_code_fragments | free_function | add | fn(workspace_root: &std::path::Path) -> Result<Vec<domain::semantic_dup::CodeFragment>, ExtractError> | 🟡 | 🔵 |
| infrastructure::semantic_dup::null_insert_proxy::acquire_persistent_index_lock | free_function | add | fn(db_path: &std::path::Path) -> Result<PersistentIndexLock, PersistentIndexLockError> | 🟡 | 🔵 |
| infrastructure::semantic_dup::null_insert_proxy::persistent_index_lock_path | free_function | add | fn(db_path: &std::path::Path) -> std::path::PathBuf | 🟡 | 🔵 |

