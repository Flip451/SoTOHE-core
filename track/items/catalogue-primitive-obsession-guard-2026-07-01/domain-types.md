<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterRuleKind | enum | modify | FieldEmpty, FieldNonEmpty, KindLayerConstraint, ReferencedRoleConstraint, TraitImplRequired, NoRoleInMethodSignature, MethodReferenceSignature, AccessorSignatureRequired, FieldElementUniqueAcrossEntries, NoExternalReferenceInMethods, NoPublicField, ForbiddenMethodReceiver, ForbidPrimitiveInTypes | 🔵 | 🔵 |
| PrimitiveOccurrencePosition | enum | add | NamedField, VariantField, Param, Return, Bound, TypeAliasTarget, ResultErr | 🔵 | 🔵 |
| RolePayloadField | enum | add | Invariants, Identity, ExclusiveMembers, SharedValueObjects, Emits, Handles, ReactsTo, Aggregate | 🔵 | 🔵 |
| SelfReceiver | enum | reference | Owned, SharedRef, ExclusiveRef | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FreeText | value_object | add | — | 🔵 | 🔵 |
| LayerId | value_object | reference | — | 🔵 | 🔵 |
| NonEmptyVec | value_object | reference | — | 🔵 | 🔵 |
| PrimitiveName | value_object | add | — | 🔵 | 🔵 |
| PrimitiveOccurrenceReport | value_object | add | — | 🔵 | 🔵 |
| TypeRef | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterError | error_type | modify | InvalidRuleConfig, UnknownLayer, ScanFailed | 🔵 | 🔵 |
| CatalogueLinterRuleError | error_type | modify | EmptyPermittedLayers, EmptyRequiredTraits, EmptyForbiddenRoles, InvalidRuleConfig | 🔵 | 🔵 |
| PrimitiveOccurrenceScanError | error_type | add | ParseFailure, InvalidSitePosition | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrimitiveOccurrenceScanner | secondary_port | add | fn scan(&self, type_ref: TypeRef, primitives: NonEmptyVec<PrimitiveName>, position: PrimitiveOccurrencePosition) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_linter::eval::evaluate_catalogue_lint | free_function | modify | fn(rules: &[CatalogueLinterRule], all_catalogues: &std::collections::BTreeMap<LayerId, CatalogueDocument>, target_layer_id: &LayerId, scanner: &S) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🔵 | 🔵 |

