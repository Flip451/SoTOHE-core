<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DeletionRecord | enum | modify | Type, Trait, Function | 🔵 | 🔵 |
| TraitRefScope | enum | modify | SelfCrate, Workspace, External | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueEntryKey | value_object | modify | — | 🔵 | 🔵 |
| CatalogueSchemaVersion | value_object | add | — | 🔵 | 🔵 |
| FullyQualifiedItemPath | value_object | add | — | 🔵 | 🔵 |
| InherentImplDeclV2 | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| NewTypeGraphCodecError | error_type | modify | InvalidTypeRef, AmbiguousIdentifier, UnresolvedIdentifier | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueToExtendedCratePort | secondary_port | modify | fn encode(&self, doc: CatalogueDocument, baseline: &rustdoc_types::Crate, current: &rustdoc_types::Crate) -> Result<ExtendedCrate, NewTypeGraphCodecError> | 🔵 | 🔵 |

## Aggregate Roots

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocument | aggregate_root | modify | — | 🔵 | 🔵 |

