<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CargoFeatureName | value_object | add | — | 🔵 | 🔵 |
| TdddFeatureDeclaration | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CargoFeatureNameError | error_type | add | InvalidFeatureName | 🔵 | 🔵 |
| TdddFeatureDeclarationError | error_type | add | MissingLayer, UnexpectedLayer, DuplicateFeature | 🔵 | 🔵 |
| TdddFeatureLookupError | error_type | add | MissingLayer | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RustdocBaselineCapturePort | secondary_port | modify | fn capture(&self, items_dir: &std::path::Path, track_id: &TrackId, rustdoc_workspace: &std::path::Path, binding: &TdddLayerBinding, features: &[CargoFeatureName]) -> Result<(), BaselineCaptureIoError> | 🟡 | 🔵 |
| RustdocCratePort | secondary_port | modify | fn load_from_path(&self, path: &std::path::Path) -> Result<rustdoc_types::Crate, RustdocCratePortError>, fn capture_current(&self, crate_name: &CrateName, features: &[CargoFeatureName]) -> Result<rustdoc_types::Crate, RustdocCratePortError> | 🟡 | 🔵 |
| SchemaExporter | secondary_port | reference | fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> | 🔵 | 🔵 |

