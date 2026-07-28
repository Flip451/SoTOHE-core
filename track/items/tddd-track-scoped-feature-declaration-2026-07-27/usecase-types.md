<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureError | error_type | modify | InvalidTrackId, SymlinkRejected, SymlinkGuardIo, LayerBindingsLoad, NoLayers, CaptureFailed, FeatureDeclaration | 🔵 | 🔵 |
| CatalogueImplSignalsError | error_type | modify | InvalidTrackId, LayerBindingsLoad, CatalogueLoad, BaselineLoad, ExtendedCrateConversion, SchemaExport, Evaluation, SymlinkRejected, SymlinkGuardIo, NoLayers, FeatureDeclaration | 🔵 | 🔵 |
| TdddActualFeatureDeclarationPortError | error_type | add | Read, MissingBaselineSnapshot, BaselineSnapshotMismatch | 🔵 | 🔵 |
| TdddBaselineFeatureDeclarationPortError | error_type | add | Read, SnapshotWrite, MissingDeclarationSnapshotWithExistingBaselines, BaselineSnapshotMismatch | 🔵 | 🔵 |
| TdddFeatureDeclarationReadError | error_type | add | MissingDeclaration, ReadDeclaration, DecodeDeclaration, UnknownCargoFeature | 🔵 | 🔵 |
| TypeSignalsError | error_type | modify | BranchTrackMismatch, LayerBindingsLoad, NoLayers, FeatureDeclaration, EvaluationFailed, InconsistentRequest | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, SchemaExporterError> | 🔵 | 🔵 |
| TdddActualFeatureDeclarationPort | secondary_port | add | fn load_for_actual(&self, track_dir: &std::path::Path, workspace_root: &std::path::Path, layers: &[domain::tddd::catalogue_v2::TdddLayerBinding]) -> Result<domain::tddd::TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> | 🔵 | 🔵 |
| TdddBaselineFeatureDeclarationPort | secondary_port | add | fn load_for_baseline(&self, track_dir: &std::path::Path, workspace_root: &std::path::Path, layers: &[domain::tddd::catalogue_v2::TdddLayerBinding]) -> Result<domain::tddd::TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError> | 🔵 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | modify | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &domain::TrackId, workspace_root: &std::path::Path, binding: &domain::tddd::catalogue_v2::TdddLayerBinding, features: &[domain::tddd::CargoFeatureName]) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureInteractor | interactor | modify | — | 🔵 | 🔵 |
| CatalogueImplSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |
| TypeSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |

