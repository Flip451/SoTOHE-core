<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureError | error_type | modify | InvalidTrackId, SymlinkRejected, SymlinkGuardIo, LayerBindingsLoad, NoLayers, CaptureFailed, FeatureDeclaration | 🔵 | 🔵 |
| CatalogueImplSignalsError | error_type | modify | InvalidTrackId, LayerBindingsLoad, CatalogueLoad, BaselineLoad, ExtendedCrateConversion, SchemaExport, Evaluation, SymlinkRejected, SymlinkGuardIo, NoLayers, FeatureDeclaration | 🔵 | 🔵 |
| CodeFragmentExtractorError | error_type | reference | ExtractionFailed | 🔵 | 🔵 |
| EmbeddingError | error_type | reference | ModelLoadFailed, InferenceFailed | 🔵 | 🔵 |
| SemanticIndexError | error_type | reference | OpenFailed, InsertFailed, DeleteFailed, SearchFailed | 🔵 | 🔵 |
| TdddActualFeatureDeclarationPortError | error_type | add | Read, MissingBaselineSnapshot, BaselineSnapshotMismatch | 🔵 | 🔵 |
| TdddBaselineFeatureDeclarationPortError | error_type | add | Read, SnapshotWrite, BaselineSnapshotMismatch | 🔵 | 🔵 |
| TdddFeatureDeclarationReadError | error_type | add | MissingDeclaration, ReadDeclaration, DecodeDeclaration, UnknownCargoFeature | 🔵 | 🔵 |
| TypeSignalsError | error_type | modify | BranchTrackMismatch, LayerBindingsLoad, NoLayers, FeatureDeclaration, EvaluationFailed, InconsistentRequest | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodeFragmentExtractorPort | secondary_port | reference | fn extract(&self, workspace_root: &std::path::Path) -> Result<Vec<domain::semantic_dup::CodeFragment>, CodeFragmentExtractorError> | 🔵 | 🔵 |
| EmbeddingPort | secondary_port | reference | fn embed(&self, fragment: &domain::semantic_dup::CodeFragment) -> Result<Vec<f32>, EmbeddingError>, fn embed_batch(&self, fragments: &[domain::semantic_dup::CodeFragment]) -> Result<Vec<Vec<f32>>, EmbeddingError> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, SchemaExporterError> | 🔵 | 🔵 |
| SemanticIndexPort | secondary_port | reference | fn insert(&self, fragment: &domain::semantic_dup::CodeFragment, embedding: &[f32]) -> Result<(), SemanticIndexError>, fn insert_batch(&self, items: &[(domain::semantic_dup::CodeFragment, Vec<f32>)]) -> Result<(), SemanticIndexError>, fn delete_by_source_path(&self, source_path: &std::path::Path) -> Result<(), SemanticIndexError>, fn search(&self, embedding: &[f32], top_k: domain::semantic_dup::TopK) -> Result<Vec<domain::semantic_dup::SimilarFragment>, SemanticIndexError> | 🔵 | 🔵 |
| TdddActualFeatureDeclarationPort | secondary_port | add | fn load_for_actual(&self, track_dir: &std::path::Path, workspace_root: &std::path::Path, layers: &[domain::tddd::catalogue_v2::TdddLayerBinding]) -> Result<domain::tddd::TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> | 🔵 | 🔵 |
| TdddBaselineFeatureDeclarationPort | secondary_port | add | fn load_for_baseline(&self, track_dir: &std::path::Path, workspace_root: &std::path::Path, layers: &[domain::tddd::catalogue_v2::TdddLayerBinding]) -> Result<domain::tddd::TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError> | 🔵 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | modify | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &domain::TrackId, workspace_root: &std::path::Path, binding: &domain::tddd::catalogue_v2::TdddLayerBinding, features: &[domain::tddd::CargoFeatureName]) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureInteractor | interactor | modify | — | 🔵 | 🔵 |
| CatalogueImplSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |
| TypeSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |

