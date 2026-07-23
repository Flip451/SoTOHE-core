<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterRuleKind | enum | modify | FieldEmpty, FieldNonEmpty, KindLayerConstraint, ReferencedRoleConstraint, TraitImplRequired, NoRoleInMethodSignature, MethodReferenceSignature, AccessorSignatureRequired, FieldElementUniqueAcrossEntries, NoExternalReferenceInMethods, NoPublicField, ForbiddenMethodReceiver, ForbidPrimitiveInTypes, DomainValueObjectInboundReferenceRequired | 🔵 | 🔵 |
| ContractMapRenderWarning | enum | add | UndefinedRoleStyle | 🔵 | 🔵 |
| RoleKind | enum | modify | ValueObject, Entity, AggregateRoot, DomainService, Specification, Factory, UseCase, Interactor, Command, Query, Dto, ErrorType, SecondaryAdapter, EventPolicy, DomainEvent, CompositionRoot, PrimaryAdapter, SpecificationPort, ApplicationService, SecondaryPort, Repository, FreeFunction, UseCaseFunction | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterRule | value_object | reference | — | 🔵 | 🔵 |
| ContractMapContent | value_object | reference | — | 🔵 | 🔵 |
| ContractMapRenderResult | value_object | add | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ContractMapRenderer | secondary_port | modify | fn render(&self, catalogues: &[CatalogueDocument], layer_order: &[LayerId], opts: &ContractMapRenderOptions) -> Result<ContractMapRenderResult, ContractMapRendererError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_linter::eval::evaluate_catalogue_lint | free_function | reference | fn(rules: &[CatalogueLinterRule], all_catalogues: &std::collections::BTreeMap<LayerId, CatalogueDocument>, target_layer_id: &LayerId, scanner: &S) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🔵 | 🔵 |

