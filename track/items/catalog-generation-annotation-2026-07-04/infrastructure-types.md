<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogDraftError | error_type | add | Incomplete, Codec | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsCatalogAdapter | secondary_adapter | add | impl Debug, impl Default, impl CatalogPort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::catalog_gen::scan_todo_holes | free_function | add | fn(value: &serde_json::Value) -> Vec<domain::tddd::catalog_gen::DraftHole> | 🔵 | 🔵 |
| infrastructure::tddd::catalog_gen::try_complete | free_function | add | fn(value: serde_json::Value) -> Result<domain::tddd::catalogue_v2::CatalogueDocument, CatalogDraftError> | 🔵 | 🔵 |

