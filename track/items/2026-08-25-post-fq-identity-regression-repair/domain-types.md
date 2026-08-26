<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ThreeWaySignalIdentity | enum | add | CatalogueItem, Label | 🔵 | 🔵 |
| domain::tddd::catalogue_linter::ExtractedTypeRefPath | enum | modify | Path, TypeParameter, LifetimeParameter, ConstParameter, AssociatedItemLabel | 🔵 | 🔵 |
| domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace | enum | add | Type, Trait | 🔵 | 🔵 |
| domain::tddd::catalogue_v2::identifiers::FullyQualifiedItemPath | enum | modify | PlacedType, UnplacedType, PlacedTrait, UnplacedTrait | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_v2::entries::TraitEntry | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::catalogue_v2::entries::TypeEntry | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::signal_evaluator::region::ThreeWaySignal | value_object | modify | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueToExtendedCratePort | secondary_port | reference | fn encode(&self, doc: CatalogueDocument, baseline: &rustdoc_types::Crate, current: &rustdoc_types::Crate) -> Result<ExtendedCrate, NewTypeGraphCodecError> | 🔵 | 🔵 |
| SignalEvaluatorPort | secondary_port | reference | fn evaluate(&self, a: ExtendedCrate, b: rustdoc_types::Crate, c: rustdoc_types::Crate) -> Result<ThreeWayEvaluationReport, Phase1Error> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity | free_function | modify | fn(reference: &TypeRef, catalogue_crate: &CrateName, universe: &std::collections::BTreeSet<FullyQualifiedItemPath>) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> | 🔵 | 🔵 |
| domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity_for_action_in_namespace | free_function | add | fn(reference: &TypeRef, catalogue_crate: &CrateName, action: ItemAction, baseline: &std::collections::BTreeSet<FullyQualifiedItemPath>, current: &std::collections::BTreeSet<FullyQualifiedItemPath>, namespace: CatalogueItemNamespace) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> | 🔵 | 🔵 |
| domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity_in_namespace | free_function | add | fn(reference: &TypeRef, catalogue_crate: &CrateName, universe: &std::collections::BTreeSet<FullyQualifiedItemPath>, namespace: Option<CatalogueItemNamespace>) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> | 🔵 | 🔵 |

