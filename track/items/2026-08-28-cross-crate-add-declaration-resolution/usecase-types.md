<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RustdocExportPlan | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EvaluationStartCaptureError | error_type | add | AuthoritativeInput | 🔵 | 🔵 |
| RustdocExportPlanValidationError | error_type | add | NoLayers, LayerLimitExceeded | 🔵 | 🔵 |
| usecase::catalogue_impl_signals::service::CatalogueImplSignalsError | error_type | modify | InvalidTrackId, LayerBindingsLoad, CatalogueLoad, BaselineLoad, ExtendedCrateConversion, EvaluationStartCapture, SchemaExport, Evaluation, SymlinkRejected, SymlinkGuardIo, NoLayers, LayerLimitExceeded, FeatureDeclaration | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EvaluationStartCapturePort | secondary_port | add | fn capture_evaluation_start(&self) -> Result<domain::tddd::type_signals_doc::ImplementationFingerprint, EvaluationStartCaptureError> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, SchemaExporterError> | 🔵 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | reference | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &domain::ids::TrackId, workspace_root: &std::path::Path, binding: &domain::tddd::catalogue_v2::catalogue_impl_signals_ports::TdddLayerBinding, features: &[domain::tddd::feature_declaration::CargoFeatureName]) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::catalogue_impl_signals::interactor::CatalogueImplSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |

