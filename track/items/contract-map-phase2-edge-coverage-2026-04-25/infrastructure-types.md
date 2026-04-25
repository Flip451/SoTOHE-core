<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| LoadAllCataloguesError | error_type | reference | LayerBindings, ArchRulesParse, Io, CatalogueNotFound, Decode, TopologicalSortFailed, InvalidLayerId | 🔵 | 🔵 |
| TypeCatalogueCodecError | error_type | modify | Json, Validation, UnsupportedSchemaVersion, InvalidEntry | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsCatalogueLoader | secondary_adapter | reference | impl CatalogueLoader | 🔵 | 🔵 |
| FsContractMapWriter | secondary_adapter | reference | impl ContractMapWriter | 🔵 | 🟡 |

