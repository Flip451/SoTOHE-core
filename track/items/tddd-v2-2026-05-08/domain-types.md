<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActionContradictionKind | enum | delete | AddExistingType, DeleteMissingType | 🔵 | 🔵 |
| CatalogueLinterRuleKind | enum | reference | FieldEmpty, FieldNonEmpty, KindLayerConstraint | 🔵 | 🔵 |
| ContractRole | enum | — | SpecificationPort, ApplicationService, SecondaryPort | 🔵 | 🔵 |
| DataRole | enum | — | ValueObject, Entity, AggregateRoot, DomainService, Specification, Factory, UseCase, Interactor, Command, Query, Dto, ErrorType, SecondaryAdapter | 🔵 | 🔵 |
| FunctionRole | enum | — | FreeFunction, UseCaseFunction | 🔵 | 🔵 |
| ItemAction | enum | — | Add, Modify, Reference, Delete | 🔵 | 🔵 |
| MemberDeclaration | enum | reference | Variant, Field | 🔵 | 🔵 |
| SelfReceiver | enum | — | Owned, SharedRef, ExclusiveRef | 🔵 | 🔵 |
| SignalRegion | enum | — | SIntersectC_Match_Add, SIntersectC_Match_Modify, SIntersectC_Match_Reference, SIntersectC_Mismatch_Reference, SIntersectC_Mismatch_Add, SIntersectC_Mismatch_Modify, SMinusC_Reference, SMinusC_Add, SMinusC_Modify, DIntersectC, DMinusC, CMinusSUnionD | 🔵 | 🔵 |
| ThreeWaySignalKind | enum | — | Skip, Blue, Yellow, Red | 🔵 | 🔵 |
| TypeAction | enum | delete | Add, Modify, Reference, Delete | 🔵 | 🔵 |
| TypeDefinitionKind | enum | delete | Typestate, Enum, ValueObject, ErrorType, SecondaryPort, ApplicationService, UseCase, Interactor, Dto, Command, Query, Factory, SecondaryAdapter, DomainService, FreeFunction | 🔵 | 🔵 |
| TypeKindV2 | enum | — | UnitStruct, TupleStruct, PlainStruct, Enum, TypeAlias | 🔵 | 🔵 |
| VariantPayload | enum | — | Unit, Tuple, Struct | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActionContradiction | value_object | delete | — | 🔵 | 🔵 |
| CatalogueDocument | value_object | — | — | 🔵 | 🔵 |
| CatalogueLintViolation | value_object | reference | — | 🔵 | 🔵 |
| CatalogueLinterRule | value_object | reference | — | 🔵 | 🔵 |
| ConsistencyReport | value_object | delete | — | 🔵 | 🔵 |
| ContractMapRenderOptions | value_object | modify | — | 🔵 | 🔵 |
| CrateName | value_object | — | — | 🔵 | 🔵 |
| EnumVariantDeclaration | value_object | reference | — | 🔵 | 🔵 |
| FieldDecl | value_object | — | — | 🔵 | 🔵 |
| FieldName | value_object | — | — | 🔵 | 🔵 |
| FunctionBaselineEntry | value_object | delete | — | 🔵 | 🔵 |
| FunctionEntry | value_object | — | — | 🔵 | 🔵 |
| FunctionName | value_object | — | — | 🔵 | 🔵 |
| FunctionNode | value_object | delete | — | 🔵 | 🔵 |
| FunctionPath | value_object | — | — | 🔵 | 🔵 |
| Identifier | value_object | — | — | 🔵 | 🔵 |
| LayerId | value_object | reference | — | 🔵 | 🔵 |
| MethodDeclaration | value_object | modify | — | 🔵 | 🔵 |
| MethodGenericParam | value_object | — | — | 🔵 | 🔵 |
| MethodName | value_object | — | — | 🔵 | 🔵 |
| ModulePath | value_object | — | — | 🔵 | 🔵 |
| ParamDeclaration | value_object | modify | — | 🔵 | 🔵 |
| ParamName | value_object | — | — | 🔵 | 🔵 |
| TdddLayerBinding | value_object | — | — | 🔵 | 🔵 |
| ThreeWayEvaluationReport | value_object | — | — | 🔵 | 🔵 |
| ThreeWaySignal | value_object | — | — | 🔵 | 🔵 |
| TraitBaselineEntry | value_object | delete | — | 🔵 | 🔵 |
| TraitEntry | value_object | — | — | 🔵 | 🔵 |
| TraitImplBaselineEntry | value_object | delete | — | 🔵 | 🔵 |
| TraitImplDecl | value_object | delete | — | 🔵 | 🔵 |
| TraitImplDeclV2 | value_object | — | — | 🔵 | 🔵 |
| TraitImplEntry | value_object | delete | — | 🔵 | 🔵 |
| TraitName | value_object | — | — | 🔵 | 🔵 |
| TraitNode | value_object | delete | — | 🔵 | 🔵 |
| TypeBaselineEntry | value_object | delete | — | 🔵 | 🔵 |
| TypeCatalogueDocument | value_object | delete | — | 🔵 | 🔵 |
| TypeCatalogueEntry | value_object | delete | — | 🔵 | 🔵 |
| TypeEntry | value_object | — | — | 🔵 | 🔵 |
| TypeName | value_object | — | — | 🔵 | 🔵 |
| TypeNode | value_object | delete | — | 🔵 | 🔵 |
| TypeRef | value_object | — | — | 🔵 | 🔵 |
| TypeSignal | value_object | reference | — | 🔵 | 🔵 |
| TypestateMarker | value_object | — | — | 🔵 | 🔵 |
| TypestateTransitions | value_object | modify | — | 🔵 | 🔵 |
| VariantDecl | value_object | — | — | 🔵 | 🔵 |
| VariantName | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocumentError | error_type | — | CrateNameMismatch, DuplicateTypeName, DuplicateTraitName, DuplicateFunctionPath, InvalidIdentifier | 🔵 | 🔵 |
| CatalogueDocumentLoaderError | error_type | — | NotFound, Io, Decode | 🔵 | 🔵 |
| CatalogueLinterError | error_type | reference | InvalidRuleConfig | 🔵 | 🔵 |
| GenericArgsError | error_type | — | Empty, StartsWithAngleBracket, UnbalancedAngleBrackets | 🔵 | 🔵 |
| IdentifierError | error_type | — | Empty, InvalidCharacters, InvalidSegment, InvalidFunctionPath | 🔵 | 🔵 |
| NewTypeGraphCodecError | error_type | — | InvalidTypeRef, AmbiguousTypeName | 🔵 | 🔵 |
| Phase1Error | error_type | — | ActionContradiction, UnresolvedTypeRef, DanglingId | 🔵 | 🔵 |
| RustdocCratePortError | error_type | — | NotFound, Io, ParseFailed, CaptureFailed | 🔵 | 🔵 |
| SymlinkGuardError | error_type | — | SymlinkFound, Io | 🔵 | 🔵 |
| TdddLayerBindingsError | error_type | — | LoadFailed, LayerNotFound, NoLayers | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocumentLoaderPort | secondary_port | — | fn load(&self, path: &std::path::Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> | 🔵 | 🔵 |
| CatalogueLinter | secondary_port | modify | fn run(&self, rules: &[CatalogueLinterRule], catalogue: &CatalogueDocument, layer_id: &str) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> | 🔵 | 🔵 |
| CatalogueLoader | secondary_port | modify | fn load_all(&self, track_id: &TrackId) -> Result<(Vec<LayerId>, BTreeMap<LayerId, CatalogueDocument>), CatalogueLoaderError> | 🔵 | 🔵 |
| CatalogueToExtendedCratePort | secondary_port | — | fn encode(&self, doc: CatalogueDocument) -> Result<ExtendedCrate, NewTypeGraphCodecError> | 🔵 | 🔵 |
| ContractMapWriter | secondary_port | reference | fn write(&self, track_id: &TrackId, content: &ContractMapContent) -> Result<(), ContractMapWriterError> | 🔵 | 🔵 |
| RustdocCratePort | secondary_port | — | fn load_from_path(&self, path: &std::path::Path) -> Result<rustdoc_types::Crate, RustdocCratePortError>, fn capture_current(&self, crate_name: &str) -> Result<rustdoc_types::Crate, RustdocCratePortError> | 🔵 | 🔵 |
| SchemaExporter | secondary_port | reference | fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> | 🔵 | 🔵 |
| SignalEvaluatorPort | secondary_port | — | fn evaluate(&self, a: ExtendedCrate, b: rustdoc_types::Crate, c: rustdoc_types::Crate) -> Result<ThreeWayEvaluationReport, Phase1Error> | 🔵 | 🔵 |
| SymlinkGuardPort | secondary_port | — | fn reject_symlinks_from_root(&self, path: &std::path::Path) -> Result<(), SymlinkGuardError>, fn reject_symlinks_below(&self, path: &std::path::Path, trusted_root: &std::path::Path) -> Result<(), SymlinkGuardError> | 🔵 | 🔵 |
| TdddLayerBindingsPort | secondary_port | — | fn load(&self, workspace_root: &std::path::Path, layer_filter: Option<&str>) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> | 🔵 | 🔵 |

## Domain Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExtendedCrate | domain_service | — | — | 🔵 | 🔵 |
| TypeBaseline | domain_service | delete | — | 🔵 | 🔵 |
| TypeGraph | domain_service | delete | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_spec_signal::check_catalogue_spec_ref_integrity | free_function | delete | fn(layer: &LayerId, catalogue: &TypeCatalogueDocument, spec_element_hashes: &BTreeMap<SpecElementId, ContentHash>, current_catalogue_hash: Option<&ContentHash>, signals_opt: Option<&CatalogueSpecSignalsDocument>) -> Vec<SpecRefFinding> | 🔵 | 🔵 |
| domain::tddd::catalogue_spec_signal::check_catalogue_spec_signals | free_function | delete | fn(layer_id: &str, catalogue: &TypeCatalogueDocument, spec_doc: &SpecDocument) -> CatalogueSpecSignalsDocument | 🔵 | 🔵 |
| domain::tddd::catalogue_spec_signal::evaluate_catalogue_entry_signal | free_function | modify | fn(action: ItemAction, spec_refs: &[SpecRef], informal_grounds: &[InformalGroundRef]) -> ConfidenceSignal | 🔵 | 🔵 |
| domain::tddd::consistency::check_consistency | free_function | delete | fn() -> ConsistencyReport | 🔵 | 🔵 |
| domain::tddd::consistency::check_type_signals | free_function | modify | fn(doc: &TypeSignalsDocument, strict: bool) -> VerifyOutcome | 🔵 | 🔵 |
| domain::tddd::contract_map_render::render_contract_map | free_function | modify | fn(catalogues: &BTreeMap<LayerId, CatalogueDocument>, layer_order: &[LayerId], opts: &ContractMapRenderOptions) -> ContractMapContent | 🔵 | 🔵 |
| domain::tddd::signals::evaluate_type_signals | free_function | delete | fn() -> Vec<TypeSignal> | 🔵 | 🔵 |
| domain::tddd::signals::undeclared_functions_to_signals | free_function | delete | fn() -> Vec<TypeSignal> | 🔵 | 🔵 |
| domain::tddd::signals::undeclared_to_signals | free_function | delete | fn() -> Vec<TypeSignal> | 🔵 | 🔵 |

