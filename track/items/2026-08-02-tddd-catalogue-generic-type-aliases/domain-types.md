<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeKindV2 | enum | modify | Struct, Enum, TypeAlias | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterError | error_type | modify | DuplicateTypeAliasGenericParameter, InvalidRuleConfig, UnknownLayer, ScanFailed | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_linter::evaluate_catalogue_lint | free_function | add | fn(rules: &[CatalogueLinterRule], all_catalogues: &std::collections::BTreeMap<LayerId, CatalogueDocument>, target_layer_id: &LayerId, scanner: &S) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🔵 | 🔵 |

