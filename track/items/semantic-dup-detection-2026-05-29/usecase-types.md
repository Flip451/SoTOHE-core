<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BuildIndexOutput | value_object | — | — | 🔵 | 🔵 |
| DupCheckOutput | value_object | — | — | 🔵 | 🔵 |
| DupCheckWarning | value_object | — | — | 🔵 | 🔵 |
| FindSimilarOutput | value_object | — | — | 🔵 | 🔵 |
| QualityMetrics | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BuildIndexError | error_type | — | Embedding, Index, Io | 🔵 | 🔵 |
| DupCheckError | error_type | — | Embedding, Index | 🔵 | 🔵 |
| EmbeddingError | error_type | — | ModelLoadFailed, InferenceFailed | 🔵 | 🔵 |
| FindSimilarError | error_type | — | Embedding, Index | 🔵 | 🔵 |
| MeasureQualityError | error_type | — | Embedding, Index, Io | 🔵 | 🔵 |
| SemanticIndexError | error_type | — | OpenFailed, InsertFailed, SearchFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EmbeddingPort | secondary_port | — | fn embed(&self, fragment: &domain::semantic_dup::CodeFragment) -> Result<Vec<f32>, EmbeddingError> | 🔵 | 🔵 |
| SemanticIndexPort | secondary_port | — | fn insert(&self, fragment: &domain::semantic_dup::CodeFragment, embedding: &[f32]) -> Result<(), SemanticIndexError>, fn search(&self, embedding: &[f32], top_k: domain::semantic_dup::TopK) -> Result<Vec<domain::semantic_dup::SimilarFragment>, SemanticIndexError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BuildIndexService | application_service | — | fn build_index(&self, cmd: &BuildIndexCommand) -> Result<BuildIndexOutput, BuildIndexError> | 🔵 | 🔵 |
| DupCheckService | application_service | — | fn dup_check(&self, cmd: &DupCheckCommand) -> Result<DupCheckOutput, DupCheckError> | 🔵 | 🔵 |
| FindSimilarService | application_service | — | fn find_similar(&self, cmd: &FindSimilarCommand) -> Result<FindSimilarOutput, FindSimilarError> | 🔵 | 🔵 |
| MeasureQualityService | application_service | — | fn measure_quality(&self, cmd: &MeasureQualityCommand) -> Result<QualityMetrics, MeasureQualityError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BuildIndexInteractor | interactor | — | — | 🔵 | 🔵 |
| DupCheckInteractor | interactor | — | — | 🔵 | 🔵 |
| FindSimilarInteractor | interactor | — | — | 🔵 | 🔵 |
| MeasureQualityInteractor | interactor | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BuildIndexCommand | command | — | — | 🔵 | 🔵 |
| MeasureQualityCommand | command | — | — | 🔵 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DupCheckCommand | query | — | — | 🔵 | 🔵 |
| FindSimilarCommand | query | — | — | 🔵 | 🔵 |

