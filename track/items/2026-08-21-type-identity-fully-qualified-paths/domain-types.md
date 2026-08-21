<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DeletionRecord | enum | modify | Type, Trait, Function | 🟡 | 🔵 |
| TraitRefScope | enum | modify | SelfCrate, Workspace, External | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueEntryKey | value_object | modify | — | 🟡 | 🔵 |
| CatalogueSchemaVersion | value_object | add | — | 🟡 | 🔵 |
| FullyQualifiedItemPath | value_object | add | — | 🟡 | 🔵 |
| InherentImplDeclV2 | value_object | modify | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| NewTypeGraphCodecError | error_type | modify | InvalidTypeRef, AmbiguousIdentifier, UnresolvedIdentifier | 🟡 | 🔵 |

## Aggregate Roots

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocument | aggregate_root | modify | — | 🟡 | 🔵 |

