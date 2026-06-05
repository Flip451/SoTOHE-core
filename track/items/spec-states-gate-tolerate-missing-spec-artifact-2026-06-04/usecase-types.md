<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BuildIndexError | error_type | reference | Embedding, Index, Io | 🔵 | 🔵 |
| DupCheckError | error_type | reference | Embedding, Index | 🔵 | 🔵 |
| FindSimilarError | error_type | reference | Embedding, Index | 🔵 | 🔵 |
| MeasureQualityError | error_type | reference | Embedding, Index, Io | 🔵 | 🔵 |
| SemanticIndexError | error_type | modify | OpenFailed, InsertFailed, DeleteFailed, SearchFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EmbeddingPort | secondary_port | modify | fn embed(&self, fragment: &domain::semantic_dup::CodeFragment) -> Result<Vec<f32>, EmbeddingError>, fn embed_batch(&self, fragments: &[domain::semantic_dup::CodeFragment]) -> Result<Vec<Vec<f32>>, EmbeddingError> | 🔵 | 🔵 |
| SemanticIndexPort | secondary_port | modify | fn insert(&self, fragment: &domain::semantic_dup::CodeFragment, embedding: &[f32]) -> Result<(), SemanticIndexError>, fn insert_batch(&self, items: &[(domain::semantic_dup::CodeFragment, Vec<f32>)]) -> Result<(), SemanticIndexError>, fn delete_by_source_path(&self, source_path: &std::path::Path) -> Result<(), SemanticIndexError>, fn search(&self, embedding: &[f32], top_k: domain::semantic_dup::TopK) -> Result<Vec<domain::semantic_dup::SimilarFragment>, SemanticIndexError> | 🔵 | 🔵 |

