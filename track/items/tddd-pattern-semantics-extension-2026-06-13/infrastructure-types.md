<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocumentCodecError | error_type | modify | Json, Io, SchemaVersionRequiresMigration, UnsupportedSchemaVersion, InvalidEntry, CrateNameMismatch, CrossCrateFunctionPath | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocumentCodec | secondary_adapter | modify | impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| FsLintConfigLoader | secondary_adapter | — | impl Debug, impl LintConfigLoader | 🟡 | 🔵 |
| InMemoryCatalogueLinter | secondary_adapter | delete | impl Default, impl CatalogueLinter | 🔵 | 🔵 |

