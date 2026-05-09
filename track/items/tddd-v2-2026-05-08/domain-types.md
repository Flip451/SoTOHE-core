<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DataRole | enum | — | ValueObject, Entity, AggregateRoot, DomainService, Specification, Factory, UseCase, Interactor, Command, Query, Dto, ErrorType, SecondaryAdapter | 🔵 | 🔵 |
| ContractRole | enum | — | SpecificationPort, ApplicationService, SecondaryPort | 🔵 | 🔵 |
| FunctionRole | enum | — | FreeFunction, UseCaseFunction | 🔵 | 🔵 |
| ItemAction | enum | — | Add, Modify, Reference, Delete | 🔵 | 🔵 |
| SelfReceiver | enum | — | Owned, SharedRef, ExclusiveRef | 🔵 | 🔵 |
| Layer | enum | — | Domain, Usecase, Infrastructure | 🔵 | 🔵 |
| CompositePattern | enum | — | Plain, TypestateState, Newtype | 🔵 | 🔵 |
| VariantPayload | enum | — | Unit, Tuple, Struct | 🔵 | 🔵 |
| TypeKindV2 | enum | — | Struct, Enum, TypeAlias | 🔵 | 🟡 |
| SignalRegion | enum | — | SIntersectC_Match_Add, SIntersectC_Match_Modify, SIntersectC_Match_Reference, SIntersectC_Mismatch_Reference, SIntersectC_Mismatch_Add, SIntersectC_Mismatch_Modify, SMinusC_Reference, SMinusC_Add, SMinusC_Modify, DIntersectC, DMinusC, CMinusSUnionD | 🟡 | 🔵 |
| ThreeWaySignalKind | enum | — | Skip, Blue, Yellow, Red | 🟡 | 🔵 |
| TypeDefinitionKind | enum | delete | Typestate, Enum, ValueObject, ErrorType, SecondaryPort, ApplicationService, UseCase, Interactor, Dto, Command, Query, Factory, SecondaryAdapter, DomainService, FreeFunction | 🟡 | 🔵 |
| MemberDeclaration | enum | delete | Variant, Field | 🟡 | 🔵 |
| TypeAction | enum | delete | Add, Modify, Reference, Delete | 🟡 | 🔵 |
| TypestateTransitions | enum | delete | Terminal, To | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| Identifier | value_object | — | — | 🔵 | 🔵 |
| TypeName | value_object | — | — | 🔵 | 🔵 |
| TraitName | value_object | — | — | 🔵 | 🔵 |
| FieldName | value_object | — | — | 🔵 | 🔵 |
| MethodName | value_object | — | — | 🔵 | 🔵 |
| ParamName | value_object | — | — | 🔵 | 🔵 |
| VariantName | value_object | — | — | 🔵 | 🔵 |
| CrateName | value_object | — | — | 🔵 | 🔵 |
| FunctionName | value_object | — | — | 🔵 | 🔵 |
| ModulePath | value_object | — | — | 🔵 | 🔵 |
| TypeRef | value_object | — | — | 🔵 | 🔵 |
| FunctionPath | value_object | — | — | 🔵 | 🔵 |
| FieldDecl | value_object | — | — | 🔵 | 🔵 |
| VariantDecl | value_object | — | — | 🔵 | 🔵 |
| TraitImplDeclV2 | value_object | — | — | 🔵 | 🔵 |
| TypeEntry | value_object | — | — | 🔵 | 🔵 |
| TraitEntry | value_object | — | — | 🔵 | 🔵 |
| FunctionEntry | value_object | — | — | 🔵 | 🔵 |
| CatalogueDocument | value_object | — | — | 🔵 | 🔵 |
| ThreeWaySignal | value_object | — | — | 🟡 | 🔵 |
| ThreeWayEvaluationReport | value_object | — | — | 🟡 | 🔵 |
| TypeCatalogueDocument | value_object | delete | — | 🟡 | 🔵 |
| TypeCatalogueEntry | value_object | delete | — | 🟡 | 🔵 |
| TypeBaseline | value_object | delete | — | 🟡 | 🔵 |
| TypeBaselineEntry | value_object | delete | — | 🟡 | 🔵 |
| TraitBaselineEntry | value_object | delete | — | 🟡 | 🔵 |
| FunctionBaselineEntry | value_object | delete | — | 🟡 | 🔵 |
| TraitImplBaselineEntry | value_object | delete | — | 🟡 | 🔵 |
| TypeGraph | value_object | delete | — | 🟡 | 🔵 |
| TypeNode | value_object | delete | — | 🟡 | 🔵 |
| TraitNode | value_object | delete | — | 🟡 | 🔵 |
| FunctionNode | value_object | delete | — | 🟡 | 🔵 |
| EnumVariantDeclaration | value_object | delete | — | 🟡 | 🔵 |
| TraitImplDecl | value_object | delete | — | 🟡 | 🔵 |
| ParamDeclaration | value_object | modify | — | 🔵 | 🔵 |
| MethodDeclaration | value_object | modify | — | 🔵 | 🔵 |
| ConsistencyReport | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| IdentifierError | error_type | — | Empty, InvalidCharacters, InvalidSegment, InvalidFunctionPath | 🔵 | 🔵 |
| CatalogueDocumentError | error_type | — | CrateNameMismatch, DuplicateTypeName, DuplicateTraitName, DuplicateFunctionPath, InvalidIdentifier | 🔵 | 🔵 |
| Phase1Error | error_type | — | ActionContradiction, UnresolvedTypeRef, DanglingId | 🟡 | 🔵 |
| NewTypeGraphCodecError | error_type | modify | InvalidTypeRef, AmbiguousTypeName | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueToExtendedCratePort | secondary_port | — | fn encode(&self, doc: CatalogueDocument) -> Result<ExtendedCrate, NewTypeGraphCodecError> | 🔵 | 🔵 |
| SignalEvaluatorPort | secondary_port | — | fn evaluate(&self, a: ExtendedCrate, b: Crate, c: Crate) -> Result<ThreeWayEvaluationReport, Phase1Error> | 🟡 | 🔵 |
| ContractMapWriter | secondary_port | reference | fn write(&self, track_id: &TrackId, content: &ContractMapContent) -> Result<(), ContractMapWriterError> | 🔵 | 🟡 |
| CatalogueLinter | secondary_port | modify | fn run(&self, rules: &[CatalogueLinterRule], catalogue: &CatalogueDocument, layer_id: &str) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🟡 | 🟡 |
| SchemaExporter | secondary_port | reference | fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> | 🔵 | 🟡 |

## Domain Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExtendedCrate | domain_service | modify | — | 🔵 | 🔵 |

