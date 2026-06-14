<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterRuleKind | enum | modify | FieldEmpty, FieldNonEmpty, KindLayerConstraint, ReferencedRoleConstraint, TraitImplRequired, NoRoleInMethodSignature, MethodReferenceSignature, AccessorSignatureRequired, FieldElementUniqueAcrossEntries, NoExternalReferenceInMethods, NoPublicField, ForbiddenMethodReceiver | 🔵 | 🔵 |
| ContractRole | enum | modify | SpecificationPort, ApplicationService, SecondaryPort, Repository | 🔵 | 🔵 |
| DataRole | enum | modify | ValueObject, Entity, AggregateRoot, DomainService, Specification, Factory, UseCase, Interactor, Command, Query, Dto, ErrorType, SecondaryAdapter, DomainEvent, EventPolicy | 🔵 | 🔵 |
| InvariantPredicate | enum | — | SelfMethod | 🔵 | 🔵 |
| RoleKind | enum | — | ValueObject, Entity, AggregateRoot, DomainService, Specification, Factory, UseCase, Interactor, Command, Query, Dto, ErrorType, SecondaryAdapter, DomainEvent, EventPolicy, SpecificationPort, ApplicationService, SecondaryPort, Repository | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLintViolation | value_object | modify | — | 🔵 | 🔵 |
| CatalogueLinterRule | value_object | modify | — | 🔵 | 🔵 |
| IdentityAccessor | value_object | — | — | 🔵 | 🔵 |
| InvariantDecl | value_object | — | — | 🔵 | 🔵 |
| InvariantName | value_object | — | — | 🔵 | 🔵 |
| NonEmptyVec | value_object | — | — | 🔵 | 🔵 |
| RuleTarget | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterError | error_type | modify | InvalidRuleConfig | 🔵 | 🔵 |
| CatalogueLinterRuleError | error_type | modify | EmptyPermittedLayers, EmptyRequiredTraits, EmptyForbiddenRoles, EmptyTargetField | 🔵 | 🔵 |
| CatalogueLoaderError | error_type | reference | CatalogueNotFound, LayerDiscoveryFailed, DecodeFailed, SymlinkRejected, IoError, TopologicalSortFailed | 🔵 | 🔵 |
| ConstructionError | error_type | — | EmptyCollection | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinter | secondary_port | delete | — | 🔵 | 🔵 |
| CatalogueLoader | secondary_port | reference | fn load_all(&self, track_id: &TrackId) -> Result<(Vec<LayerId>, BTreeMap<LayerId, CatalogueDocument>), CatalogueLoaderError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_linter::eval::evaluate_catalogue_lint | free_function | — | fn(rules: &[CatalogueLinterRule], catalogue: &CatalogueDocument, layer_id: &LayerId) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🔵 | 🔵 |
| domain::tddd::catalogue_linter::preset::ddd_strict_preset | free_function | — | fn() -> Result<Vec<CatalogueLinterRule>, CatalogueLinterError> | 🔵 | 🔵 |

