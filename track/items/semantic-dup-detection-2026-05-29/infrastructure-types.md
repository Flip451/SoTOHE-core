<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExtractError | error_type | — | Io | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FastEmbedAdapter | secondary_adapter | — | impl EmbeddingPort, impl Debug | 🟡 | 🔵 |
| LanceDbSemanticIndexAdapter | secondary_adapter | — | impl SemanticIndexPort, impl Debug, impl Drop | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::semantic_dup::extractor::extract_code_fragments | free_function | — | fn(workspace_root: &std::path::Path) -> Result<Vec<domain::semantic_dup::CodeFragment>, ExtractError> | 🔵 | 🔵 |

