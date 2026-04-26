<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| MemberDeclarationDto | enum | reference | Variant, Field | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeGraphRenderOptions | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeCatalogueCodecError | error_type | modify | Json, Validation, UnsupportedSchemaVersion, InvalidEntry | 🔵 | 🔵 |
| BaselineCodecError | error_type | modify | Json, UnsupportedSchemaVersion, InvalidTimestamp | 🔵 | 🔵 |
| LoadAllCataloguesError | error_type | reference | LayerBindings, ArchRulesParse, Io, CatalogueNotFound, Decode, TopologicalSortFailed, InvalidLayerId | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsCatalogueLoader | secondary_adapter | reference | impl CatalogueLoader | 🔵 | 🔵 |
| FsContractMapWriter | secondary_adapter | reference | impl ContractMapWriter | 🔵 | 🔵 |
| FsCatalogueSpecSignalsStore | secondary_adapter | reference | impl CatalogueSpecSignalsWriter | 🔵 | 🔵 |

