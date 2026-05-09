<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EvaluateSignalsError | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocumentCodecError | error_type | — | Json, UnsupportedSchemaVersion, InvalidEntry, CrateNameMismatch | 🟡 | 🔵 |
| CatalogueToExtendedCrateCodecError | error_type | — | InvalidTypeRef, AmbiguousIdentifier | 🟡 | 🔵 |
| BaselineRustdocCodecError | error_type | — | Json, IoError, UnsupportedFormatVersion | 🟡 | 🔵 |
| SchemaExportCodecError | error_type | reference | Json | 🔵 | 🔵 |
| TypeCatalogueCodecError | error_type | delete | Json, Validation, UnsupportedSchemaVersion, InvalidEntry | 🟡 | 🔵 |
| BaselineCodecError | error_type | delete | Json, UnsupportedSchemaVersion, MissingField | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLoader | secondary_port | modify | fn load_all(&self) -> Result<Vec<(LayerId, CatalogueDocument)>, LoadAllCataloguesError> | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueToExtendedCrateCodec | secondary_adapter | — | impl CatalogueToExtendedCratePort | 🟡 | 🔵 |
| SignalEvaluatorV2 | secondary_adapter | — | impl SignalEvaluatorPort | 🟡 | 🔵 |
| FsCatalogueLoader | secondary_adapter | modify | impl CatalogueLoader | 🟡 | 🔵 |
| FsCatalogueSpecSignalsStore | secondary_adapter | reference | impl CatalogueSpecSignalsWriter | 🔵 | 🔵 |
| FsContractMapWriter | secondary_adapter | reference | impl ContractMapWriter | 🔵 | 🔵 |
| InMemoryCatalogueLinter | secondary_adapter | modify | impl CatalogueLinter | 🟡 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | reference | impl SchemaExporter, impl SchemaExporterPort | 🔵 | 🔵 |

