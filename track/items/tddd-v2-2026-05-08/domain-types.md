<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActionContradictionKind | enum | delete | — | 🔵 | 🔵 |
| CatalogueLinterRuleKind | enum | reference | — | 🔵 | 🔵 |
| ContractRole | enum | — | — | 🔵 | 🔵 |
| DataRole | enum | — | — | 🔵 | 🔵 |
| FunctionRole | enum | — | — | 🔵 | 🔵 |
| ItemAction | enum | — | — | 🔵 | 🔵 |
| MemberDeclaration | enum | reference | — | 🔵 | 🔵 |
| SelfReceiver | enum | — | — | 🔵 | 🔵 |
| SignalRegion | enum | — | — | 🔵 | 🔵 |
| ThreeWaySignalKind | enum | — | — | 🔵 | 🔵 |
| TypeAction | enum | reference | — | 🔵 | 🔵 |
| TypeDefinitionKind | enum | modify | — | 🟡 | 🔵 |
| TypeKindV2 | enum | — | — | 🔵 | 🔵 |
| TypestateTransitionsSpec | enum | — | — | 🔵 | 🔵 |
| VariantPayload | enum | — | — | 🔵 | 🔵 |

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
| TraitImplDecl | value_object | reference | — | 🔵 | 🔵 |
| TraitImplDeclV2 | value_object | — | — | 🔵 | 🔵 |
| TraitImplEntry | value_object | delete | — | 🔵 | 🔵 |
| TraitName | value_object | — | — | 🔵 | 🔵 |
| TraitNode | value_object | delete | — | 🔵 | 🔵 |
| TypeBaselineEntry | value_object | delete | — | 🔵 | 🔵 |
| TypeCatalogueDocument | value_object | reference | — | 🔵 | 🔵 |
| TypeCatalogueEntry | value_object | reference | — | 🔵 | 🔵 |
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
| CatalogueDocumentError | error_type | — | — | 🔵 | 🔵 |
| CatalogueDocumentLoaderError | error_type | — | — | 🔵 | 🔵 |
| CatalogueLinterError | error_type | reference | — | 🔵 | 🔵 |
| GenericArgsError | error_type | — | — | 🔵 | 🔵 |
| IdentifierError | error_type | — | — | 🔵 | 🔵 |
| NewTypeGraphCodecError | error_type | — | — | 🔵 | 🔵 |
| Phase1Error | error_type | — | — | 🔵 | 🔵 |
| RustdocCratePortError | error_type | — | — | 🔵 | 🔵 |
| TdddLayerBindingsError | error_type | — | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocumentLoaderPort | secondary_port | — | — | 🔵 | 🔵 |
| CatalogueLinter | secondary_port | modify | — | 🟡 | 🔵 |
| CatalogueLoader | secondary_port | modify | — | 🟡 | 🔵 |
| CatalogueToExtendedCratePort | secondary_port | — | — | 🔵 | 🔵 |
| ContractMapWriter | secondary_port | reference | — | 🔵 | 🔵 |
| RustdocCratePort | secondary_port | — | — | 🔵 | 🔵 |
| SchemaExporter | secondary_port | reference | — | 🔵 | 🔵 |
| SignalEvaluatorPort | secondary_port | — | — | 🔵 | 🔵 |
| TdddLayerBindingsPort | secondary_port | — | — | 🔵 | 🔵 |

## Domain Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExtendedCrate | domain_service | — | — | 🔵 | 🔵 |
| TypeBaseline | domain_service | delete | — | 🔵 | 🔵 |
| TypeGraph | domain_service | delete | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::catalogue_spec_signal::check_catalogue_spec_ref_integrity | free_function | reference | — | 🔵 | 🔵 |
| domain::tddd::catalogue_spec_signal::check_catalogue_spec_signals | free_function | reference | — | 🔵 | 🔵 |
| domain::tddd::catalogue_spec_signal::evaluate_catalogue_entry_signal | free_function | modify | — | 🔵 | 🔵 |
| domain::tddd::consistency::check_consistency | free_function | delete | — | 🔵 | 🔵 |
| domain::tddd::consistency::check_type_signals | free_function | modify | — | 🟡 | 🔵 |
| domain::tddd::contract_map_render::render_contract_map | free_function | modify | — | 🟡 | 🔵 |
| domain::tddd::signals::evaluate_type_signals | free_function | delete | — | 🔵 | 🔵 |
| domain::tddd::signals::undeclared_functions_to_signals | free_function | delete | — | 🔵 | 🔵 |
| domain::tddd::signals::undeclared_to_signals | free_function | delete | — | 🔵 | 🔵 |

