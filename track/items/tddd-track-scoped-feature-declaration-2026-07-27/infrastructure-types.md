<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsTdddFeatureDeclarationAdapter | secondary_adapter | add | impl TdddBaselineFeatureDeclarationPort, impl TdddActualFeatureDeclarationPort, impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| RustdocBaselineCaptureAdapter | secondary_adapter | modify | impl RustdocBaselineCapturePort, impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| RustdocCrateAdapter | secondary_adapter | modify | impl RustdocCratePort | 🔵 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | modify | impl SchemaExporter, impl SchemaExporterPort | 🔵 | 🔵 |
| TypeSignalsExecutorAdapter | secondary_adapter | modify | impl TypeSignalsExecutorPort, impl Debug, impl Default | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer | free_function | modify | fn(items_dir: &std::path::Path, track_id: &domain::TrackId, workspace_root: &std::path::Path, binding: &TdddLayerBinding, features: &[domain::tddd::CargoFeatureName]) -> Result<std::process::ExitCode, EvaluateSignalsError> | 🔵 | 🔵 |

