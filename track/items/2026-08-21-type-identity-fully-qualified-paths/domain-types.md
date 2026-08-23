<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DeletionRecord | enum | modify | Type, Trait, Function | 🔵 | 🔵 |
| TraitRefScope | enum | modify | SelfCrate, Workspace, External | 🔵 | 🔵 |
| domain::tddd::catalogue_linter::ExtractedTypeRefPath | enum | add | Reference, GenericConstructor | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueEntryKey | value_object | modify | — | 🔵 | 🔵 |
| CatalogueSchemaVersion | value_object | add | — | 🔵 | 🔵 |
| FullyQualifiedItemPath | value_object | add | — | 🔵 | 🔵 |
| InherentImplDeclV2 | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::catalogue_linter::CatalogueLintViolation | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueIdentityResolutionError | error_type | add | AmbiguousIdentifier, UnresolvedIdentifier | 🔵 | 🔵 |
| NewTypeGraphCodecError | error_type | modify | InvalidTypeRef, AmbiguousIdentifier, UnresolvedIdentifier | 🔵 | 🔵 |
| domain::tddd::catalogue_linter::CatalogueLinterError | error_type | modify | DuplicateTypeAliasGenericParameter, InvalidTypeAliasGenericParameterName, ConflictingTypeAliasGenericParameters, InvalidRuleConfig, UnknownLayer, ScanFailed, PathExtractionFailed, IdentityResolutionFailed | 🔵 | 🔵 |
| domain::tddd::catalogue_linter::TypeRefPathExtractionError | error_type | add | InvalidTypeRef | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueToExtendedCratePort | secondary_port | modify | fn encode(&self, doc: CatalogueDocument, baseline: &rustdoc_types::Crate, current: &rustdoc_types::Crate) -> Result<ExtendedCrate, NewTypeGraphCodecError> | 🔵 | 🔵 |
| domain::tddd::catalogue_linter::TypeRefPathExtractorPort | secondary_port | add | fn extract(&self, type_ref: &TypeRef) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_linter::evaluate_catalogue_lint | free_function | modify | fn(rules: &[CatalogueLinterRule], all_catalogues: &std::collections::BTreeMap<LayerId, CatalogueDocument>, target_layer_id: &LayerId, scanner: &S, extractor: &E) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🔵 | 🔵 |
| domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity | free_function | add | fn(reference: &TypeRef, catalogue_crate: &CrateName, universe: &std::collections::BTreeSet<FullyQualifiedItemPath>) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> | 🔵 | 🔵 |

## Aggregate Roots

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocument | aggregate_root | modify | — | 🔵 | 🔵 |

