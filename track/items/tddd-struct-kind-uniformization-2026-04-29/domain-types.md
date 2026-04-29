<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeDefinitionKind | enum | modify | Typestate, Enum, ValueObject, ErrorType, SecondaryPort, ApplicationService, UseCase, Interactor, Dto, Command, Query, Factory, SecondaryAdapter, FreeFunction, DomainService | 🟡 | 🔵 |
| CatalogueLinterRuleKind | enum | — | FieldEmpty, FieldNonEmpty, KindLayerConstraint | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterRule | value_object | — | — | 🟡 | 🔵 |
| CatalogueLintViolation | value_object | — | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinterRuleError | error_type | — | EmptyTargetKind, EmptyTargetField, EmptyPermittedLayers | 🟡 | 🔵 |
| CatalogueLinterError | error_type | — | InvalidRuleConfig | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLinter | secondary_port | — | fn run(&self, rules: &[CatalogueLinterRule], catalogue: &TypeCatalogueDocument, layer_id: &str) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🟡 | 🔵 |

