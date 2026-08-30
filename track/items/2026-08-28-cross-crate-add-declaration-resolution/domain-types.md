<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::type_signals_doc::TypeSignalsReuseDecision | enum | modify | SkipEvaluation, ReextractAndEvaluate | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AuthoritativeRustdocContext | value_object | add | — | 🔵 | 🔵 |
| CapturedRustdocJson | value_object | add | — | 🔵 | 🔵 |
| CargoProfileName | value_object | add | — | 🔵 | 🔵 |
| ExpectedRustdocJsonPath | value_object | add | — | 🔵 | 🔵 |
| ImplementationFingerprint | value_object | add | — | 🔵 | 🔵 |
| ResolutionFingerprint | value_object | add | — | 🔵 | 🔵 |
| ResolvedCargoTargetDirectory | value_object | add | — | 🔵 | 🔵 |
| RustdocExecutionIdentity | value_object | add | — | 🔵 | 🔵 |
| RustdocJsonHash | value_object | add | — | 🔵 | 🔵 |
| RustdocSnapshot | value_object | add | — | 🔵 | 🔵 |
| domain::tddd::type_signals_doc::TypeSignalsCacheKey | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::type_signals_doc::TypeSignalsDocument | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::type_signals_doc::TypeSignalsReuseInput | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RustdocExecutionIdentityError | error_type | add | TargetDirectoryNotAbsolute, ExpectedJsonOutsideTargetDirectory, EmptyProfile | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueToExtendedCratePort | secondary_port | modify | fn encode(&self, target_layer: &LayerId, track_catalogues: &std::collections::BTreeMap<LayerId, CatalogueDocument>, rustdoc_contexts: &std::collections::BTreeMap<LayerId, AuthoritativeRustdocContext>) -> Result<ExtendedCrate, NewTypeGraphCodecError> | 🔵 | 🔵 |
| RustdocCratePort | secondary_port | modify | fn load_from_path(&self, path: &std::path::Path) -> Result<CapturedRustdocJson, RustdocCratePortError>, fn capture_current(&self, crate_name: &CrateName, features: &[CargoFeatureName]) -> Result<RustdocSnapshot, RustdocCratePortError> | 🔵 | 🔵 |
| SchemaExporter | secondary_port | reference | fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::type_signals_doc::construct_captured_rustdoc_json | free_function | add | fn(bytes: &[u8], decode: fn(&[u8]) -> Result<rustdoc_types::Crate, RustdocCratePortError>) -> Result<CapturedRustdocJson, RustdocCratePortError> | 🔵 | 🔵 |
| domain::tddd::type_signals_doc::construct_rustdoc_snapshot | free_function | add | fn(identity: RustdocExecutionIdentity, bytes: &[u8], decode: fn(&[u8]) -> Result<rustdoc_types::Crate, RustdocCratePortError>) -> Result<RustdocSnapshot, RustdocCratePortError> | 🔵 | 🔵 |
| domain::tddd::type_signals_doc::decide_type_signals_reuse | free_function | modify | fn(input: &TypeSignalsReuseInput) -> TypeSignalsReuseDecision | 🔵 | 🔵 |

