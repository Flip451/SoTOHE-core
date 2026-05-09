<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
flowchart LR
    classDef secondary_adapter fill:#fafafa,stroke:#999,stroke-dasharray: 4 4
    classDef command fill:#e3f2fd,stroke:#1976d2
    classDef query fill:#f3e5f5,stroke:#8e24aa
    classDef factory fill:#fff8e1,stroke:#f9a825
    classDef free_function fill:#f1f8e9,stroke:#558b2f
    classDef domain_service fill:#fce4ec,stroke:#c62828
    subgraph domain [domain]
        L6_domain_Identifier(Identifier)
        L6_domain_TypeName(TypeName)
        L6_domain_TraitName(TraitName)
        L6_domain_FieldName(FieldName)
        L6_domain_MethodName(MethodName)
        L6_domain_ParamName(ParamName)
        L6_domain_VariantName(VariantName)
        L6_domain_CrateName(CrateName)
        L6_domain_FunctionName(FunctionName)
        L6_domain_ModulePath(ModulePath)
        L6_domain_TypeRef(TypeRef)
        L6_domain_FunctionPath(FunctionPath)
        L6_domain_DataRole{{DataRole}}
        L6_domain_ContractRole{{ContractRole}}
        L6_domain_FunctionRole{{FunctionRole}}
        L6_domain_ItemAction{{ItemAction}}
        L6_domain_SelfReceiver{{SelfReceiver}}
        L6_domain_Layer{{Layer}}
        L6_domain_CompositePattern{{CompositePattern}}
        L6_domain_VariantPayload{{VariantPayload}}
        L6_domain_TypeKindV2{{TypeKindV2}}
        L6_domain_FieldDecl(FieldDecl)
        L6_domain_VariantDecl(VariantDecl)
        L6_domain_TraitImplDeclV2(TraitImplDeclV2)
        L6_domain_TypeEntry(TypeEntry)
        L6_domain_TraitEntry(TraitEntry)
        L6_domain_FunctionEntry(FunctionEntry)
        L6_domain_CatalogueDocument(CatalogueDocument)
        L6_domain_CatalogueDocumentError>CatalogueDocumentError]
        L6_domain_ExtendedCrate(ExtendedCrate)
        L6_domain_Phase1Error>Phase1Error]
        L6_domain_SignalRegion{{SignalRegion}}
        L6_domain_ThreeWaySignalKind{{ThreeWaySignalKind}}
        L6_domain_ThreeWaySignal(ThreeWaySignal)
        L6_domain_ThreeWayEvaluationReport(ThreeWayEvaluationReport)
        L6_domain_NewTypeGraphCodecError>NewTypeGraphCodecError]
        L6_domain_CatalogueToExtendedCratePort[[CatalogueToExtendedCratePort]]
        L6_domain_SignalEvaluatorPort[[SignalEvaluatorPort]]
        L6_domain_TypeDefinitionKind{{TypeDefinitionKind}}
        L6_domain_TypeCatalogueDocument(TypeCatalogueDocument)
        L6_domain_TypeCatalogueEntry(TypeCatalogueEntry)
        L6_domain_TypeBaseline(TypeBaseline)
        L6_domain_TypeBaselineEntry(TypeBaselineEntry)
        L6_domain_TraitBaselineEntry(TraitBaselineEntry)
        L6_domain_FunctionBaselineEntry(FunctionBaselineEntry)
        L6_domain_TraitImplBaselineEntry(TraitImplBaselineEntry)
        L6_domain_TypeGraph(TypeGraph)
        L6_domain_TypeNode(TypeNode)
        L6_domain_TraitNode(TraitNode)
        L6_domain_FunctionNode(FunctionNode)
        L6_domain_EnumVariantDeclaration(EnumVariantDeclaration)
        L6_domain_MemberDeclaration{{MemberDeclaration}}
        L6_domain_TypeAction{{TypeAction}}
        L6_domain_TypestateTransitions{{TypestateTransitions}}
        L6_domain_TraitImplDecl(TraitImplDecl)
        L6_domain_ParamDeclaration(ParamDeclaration)
        L6_domain_MethodDeclaration(MethodDeclaration)
        L6_domain_ConsistencyReport(ConsistencyReport)
        L6_domain_ContractMapWriter[[ContractMapWriter]]
        L6_domain_CatalogueLinter[[CatalogueLinter]]
        L6_domain_SchemaExporter[[SchemaExporter]]
    end
    subgraph usecase [usecase]
        L7_usecase_PreCommitTypeSignalsService[/PreCommitTypeSignalsService\]
        L7_usecase_PreCommitTypeSignalsInteractor[\PreCommitTypeSignalsInteractor/]
        L7_usecase_CatalogueSpecSignalsWriter[[CatalogueSpecSignalsWriter]]
        L7_usecase_SchemaExporterPort[[SchemaExporterPort]]
    end
    subgraph infrastructure [infrastructure]
        L14_infrastructure_CatalogueDocumentCodecError>CatalogueDocumentCodecError]
        L14_infrastructure_CatalogueToExtendedCrateCodecError>CatalogueToExtendedCrateCodecError]
        L14_infrastructure_CatalogueToExtendedCrateCodec[CatalogueToExtendedCrateCodec]:::secondary_adapter
        L14_infrastructure_BaselineRustdocCodecError>BaselineRustdocCodecError]
        L14_infrastructure_SignalEvaluatorV2[SignalEvaluatorV2]:::secondary_adapter
        L14_infrastructure_SchemaExportCodecError>SchemaExportCodecError]
        L14_infrastructure_TypeCatalogueCodecError>TypeCatalogueCodecError]
        L14_infrastructure_BaselineCodecError>BaselineCodecError]
        L14_infrastructure_FsCatalogueLoader[FsCatalogueLoader]:::secondary_adapter
        L14_infrastructure_CatalogueLoader[[CatalogueLoader]]
        L14_infrastructure_EvaluateSignalsError(EvaluateSignalsError)
        L14_infrastructure_FsCatalogueSpecSignalsStore[FsCatalogueSpecSignalsStore]:::secondary_adapter
        L14_infrastructure_FsContractMapWriter[FsContractMapWriter]:::secondary_adapter
        L14_infrastructure_InMemoryCatalogueLinter[InMemoryCatalogueLinter]:::secondary_adapter
        L14_infrastructure_RustdocSchemaExporter[RustdocSchemaExporter]:::secondary_adapter
    end
    L14_infrastructure_CatalogueLoader -->|"load_all"| L6_domain_CatalogueDocument
    L14_infrastructure_CatalogueToExtendedCrateCodec -.impl.-> L6_domain_CatalogueToExtendedCratePort
    L14_infrastructure_FsCatalogueLoader -->|"load_all"| L6_domain_CatalogueDocument
    L14_infrastructure_FsCatalogueLoader -.impl.-> L14_infrastructure_CatalogueLoader
    L14_infrastructure_FsCatalogueSpecSignalsStore -.impl.-> L7_usecase_CatalogueSpecSignalsWriter
    L14_infrastructure_FsContractMapWriter -.impl.-> L6_domain_ContractMapWriter
    L14_infrastructure_InMemoryCatalogueLinter -->|"run(catalogue)"| L6_domain_CatalogueDocument
    L14_infrastructure_InMemoryCatalogueLinter -.impl.-> L6_domain_CatalogueLinter
    L14_infrastructure_RustdocSchemaExporter -.impl.-> L6_domain_SchemaExporter
    L14_infrastructure_RustdocSchemaExporter -.impl.-> L7_usecase_SchemaExporterPort
    L14_infrastructure_SignalEvaluatorV2 -.impl.-> L6_domain_SignalEvaluatorPort
    L6_domain_CatalogueLinter -->|"run(catalogue)"| L6_domain_CatalogueDocument
    L6_domain_CatalogueToExtendedCratePort -->|"encode"| L6_domain_ExtendedCrate
    L6_domain_CatalogueToExtendedCratePort -->|"encode"| L6_domain_NewTypeGraphCodecError
    L6_domain_CatalogueToExtendedCratePort -->|"encode(doc)"| L6_domain_CatalogueDocument
    L6_domain_CompositePattern -->|"::Newtype"| L6_domain_TypeRef
    L6_domain_CompositePattern -->|"::TypestateState"| L6_domain_MethodName
    L6_domain_CompositePattern -->|"::TypestateState"| L6_domain_TypeName
    L6_domain_SignalEvaluatorPort -->|"evaluate"| L6_domain_Phase1Error
    L6_domain_SignalEvaluatorPort -->|"evaluate"| L6_domain_ThreeWayEvaluationReport
    L6_domain_SignalEvaluatorPort -->|"evaluate(a)"| L6_domain_ExtendedCrate
    L6_domain_TypeKindV2 -->|"::Enum"| L6_domain_VariantDecl
    L6_domain_TypeKindV2 -->|"::Struct"| L6_domain_CompositePattern
    L6_domain_TypeKindV2 -->|"::Struct"| L6_domain_FieldDecl
    L6_domain_TypeKindV2 -->|"::TypeAlias"| L6_domain_TypeRef
    L6_domain_VariantPayload -->|"::Struct"| L6_domain_FieldDecl
    L6_domain_VariantPayload -->|"::Tuple"| L6_domain_TypeRef
    L7_usecase_PreCommitTypeSignalsInteractor -.impl.-> L7_usecase_PreCommitTypeSignalsService
```
