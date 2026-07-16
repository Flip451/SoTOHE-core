<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EdgeOwnership | enum | add | None, Unique, Multiple | 🔵 | 🔵 |
| RoundType | enum | reference | Fast, Final | 🔵 | 🔵 |
| ScopeName | enum | reference | Main, Other | 🔵 | 🔵 |
| TypeSignalsLoadResult | enum | modify | Current, Stale, Missing | 🔵 | 🔵 |
| TypeSignalsReuseDecision | enum | add | SkipEvaluation, ReevaluateWithoutExtraction, ReextractAndEvaluate | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDeclarationHash | value_object | add | — | 🔵 | 🔵 |
| ImplementationInputHash | value_object | add | — | 🔵 | 🔵 |
| LayerId | value_object | reference | — | 🔵 | 🔵 |
| Sha256Digest | value_object | add | — | 🔵 | 🔵 |
| TrackBranch | value_object | reference | — | 🔵 | 🔵 |
| TrackId | value_object | reference | — | 🔵 | 🔵 |
| TypeSignalsDocument | value_object | modify | — | 🔵 | 🔵 |
| TypeSignalsSchemaVersion | value_object | add | — | 🔵 | 🔵 |
| WaiverCacheEntry | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| Sha256DigestError | error_type | add | InvalidLength, InvalidHex | 🔵 | 🔵 |
| TypeSignalsSchemaVersionError | error_type | add | Zero | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SchemaExporter | secondary_port | reference | fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> | 🔵 | 🔵 |

## Domain Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ObligationsDocument | domain_service | modify | — | 🔵 | 🔵 |
| TestBindingsDocument | domain_service | modify | — | 🔵 | 🔵 |
| TestObligation | domain_service | modify | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::type_signals_doc::decide_type_signals_reuse | free_function | add | fn(recorded_declaration_hash: &CatalogueDeclarationHash, recorded_implementation_input_hash: &ImplementationInputHash, current_declaration_hash: &CatalogueDeclarationHash, current_implementation_input_hash: Option<&ImplementationInputHash>) -> TypeSignalsReuseDecision | 🔵 | 🔵 |

