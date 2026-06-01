<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| MissingCataloguePolicy | enum | delete | FailClosed, SkipSilently | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RustdocBaselineCapturePort | secondary_port | modify | fn capture(&self, items_dir: &std::path::Path, track_id: &str, rustdoc_workspace: &std::path::Path, binding: &TdddLayerBinding) -> Result<(), BaselineCaptureIoError> | 🔵 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | modify | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &str, workspace_root: &std::path::Path, binding: &TdddLayerBinding) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

