<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| StructShapeKindDto | enum | add | Unit, Tuple, Plain | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogDraftError | error_type | add | Incomplete, Codec | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ImplInfoDto | dto | modify | — | 🔵 | 🔵 |
| TypeInfoDto | dto | modify | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsCatalogAdapter | secondary_adapter | add | impl Debug, impl Default, impl CatalogPort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::catalog_gen::scan_todo_holes | free_function | add | fn(value: &serde_json::Value) -> Vec<domain::tddd::catalog_gen::DraftHole> | 🔵 | 🔵 |
| infrastructure::tddd::catalog_gen::try_complete | free_function | add | fn(value: serde_json::Value, expected_stem: &str) -> Result<domain::tddd::catalogue_v2::CatalogueDocument, CatalogDraftError> | 🔵 | 🔵 |

