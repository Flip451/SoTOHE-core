<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
---
config:
  layout: elk
---
flowchart LR
classDef aggregate_root fill:#ede9fe,stroke:#4c1d95,stroke-width:2px
classDef app_service fill:#ecfdf5,stroke:#059669,stroke-width:2px
classDef command fill:#fff7ed,stroke:#c2410c,stroke-width:1px
classDef domain_service fill:#fee2e2,stroke:#991b1b,stroke-width:1px
classDef dto fill:#f8fafc,stroke:#64748b,stroke-width:1px
classDef entity fill:#dbeafe,stroke:#1e40af,stroke-width:2px
classDef error_type fill:#fef2f2,stroke:#b91c1c,stroke-width:1px,stroke-dasharray:4 2
classDef factory fill:#e0f2fe,stroke:#0369a1,stroke-width:1px
classDef free_function fill:#f5f3ff,stroke:#7c3aed,stroke-width:1px
classDef function_node fill:#f5f3ff,stroke:#a78bfa,stroke-width:1px
classDef interactor fill:#f0fdfa,stroke:#0d9488,stroke-width:1px
classDef method_node fill:#f8fafc,stroke:#cbd5e1,stroke-width:1px
classDef query fill:#f0f9ff,stroke:#0369a1,stroke-width:1px
classDef secondary_adapter fill:#fafaf9,stroke:#57534e,stroke-width:1px
classDef secondary_port fill:#fafaf9,stroke:#78716c,stroke-width:1px,stroke-dasharray:4 2
classDef specification fill:#fdf4ff,stroke:#6b21a8,stroke-width:1px
classDef specification_port fill:#fdf4ff,stroke:#9333ea,stroke-width:1px,stroke-dasharray:4 2
classDef typestate_overlay stroke:#dc2626,stroke-width:3px
classDef use_case fill:#ecfeff,stroke:#0e7490,stroke-width:1px
classDef use_case_function fill:#eef2ff,stroke:#4338ca,stroke-width:1px
classDef value_object fill:#d1fae5,stroke:#065f46,stroke-width:1px
classDef variant_node fill:#fafaf9,stroke:#d6d3d1,stroke-width:1px
subgraph domain["domain"]
  direction TB
  subgraph domain_domain_module_schema["domain::schema"]
    direction TB
  subgraph T26_domain_domain_FunctionInfo["schema::FunctionInfo"]
    direction TB
    T26_domain_domain_FunctionInfo__self[FunctionInfo]
    T26_domain_domain_FunctionInfo_new([new])
    T26_domain_domain_FunctionInfo_with_module_path([with_module_path])
    T26_domain_domain_FunctionInfo_name([name])
    T26_domain_domain_FunctionInfo_docs([docs])
    T26_domain_domain_FunctionInfo_return_type_names([return_type_names])
    T26_domain_domain_FunctionInfo_has_self_receiver([has_self_receiver])
    T26_domain_domain_FunctionInfo_params([params])
    T26_domain_domain_FunctionInfo_returns([returns])
    T26_domain_domain_FunctionInfo_receiver([receiver])
    T26_domain_domain_FunctionInfo_is_async([is_async])
    T26_domain_domain_FunctionInfo_module_path([module_path])
  end
  subgraph T22_domain_domain_ImplInfo["schema::ImplInfo"]
    direction TB
    T22_domain_domain_ImplInfo__self[ImplInfo]
    T22_domain_domain_ImplInfo_new([new])
    T22_domain_domain_ImplInfo_with_trait_def_path([with_trait_def_path])
    T22_domain_domain_ImplInfo_with_target_details([with_target_details])
    T22_domain_domain_ImplInfo_target_type([target_type])
    T22_domain_domain_ImplInfo_trait_name([trait_name])
    T22_domain_domain_ImplInfo_methods([methods])
    T22_domain_domain_ImplInfo_trait_def_path([trait_def_path])
    T22_domain_domain_ImplInfo_target_module_path([target_module_path])
  end
  subgraph T29_domain_domain_StructShapeKind["schema::StructShapeKind"]
    direction TB
    T29_domain_domain_StructShapeKind__self[StructShapeKind]
    T29_domain_domain_StructShapeKind_Unit[Unit]
    T29_domain_domain_StructShapeKind_Tuple[Tuple]
    T29_domain_domain_StructShapeKind_Plain[Plain]
  end
  subgraph T22_domain_domain_TypeInfo["schema::TypeInfo"]
    direction TB
    T22_domain_domain_TypeInfo__self[TypeInfo]
    T22_domain_domain_TypeInfo_new([new])
    T22_domain_domain_TypeInfo_with_module_path([with_module_path])
    T22_domain_domain_TypeInfo_with_alias_target([with_alias_target])
    T22_domain_domain_TypeInfo_with_struct_shape([with_struct_shape])
    T22_domain_domain_TypeInfo_name([name])
    T22_domain_domain_TypeInfo_kind([kind])
    T22_domain_domain_TypeInfo_docs([docs])
    T22_domain_domain_TypeInfo_members([members])
    T22_domain_domain_TypeInfo_module_path([module_path])
    T22_domain_domain_TypeInfo_alias_target([alias_target])
    T22_domain_domain_TypeInfo_struct_shape([struct_shape])
    T22_domain_domain_TypeInfo_struct_shape_matches_kind([struct_shape_matches_kind])
  end
  end
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T30_domain_domain_CatalogEntryKind["tddd::catalog_gen::CatalogEntryKind"]
    direction TB
    T30_domain_domain_CatalogEntryKind__self[CatalogEntryKind]
    T30_domain_domain_CatalogEntryKind_Struct[Struct]
    T30_domain_domain_CatalogEntryKind_Enum[Enum]
    T30_domain_domain_CatalogEntryKind_TypeAlias[TypeAlias]
    T30_domain_domain_CatalogEntryKind_Trait[Trait]
    T30_domain_domain_CatalogEntryKind_Function[Function]
  end
  subgraph T30_domain_domain_CatalogEntryName["tddd::catalog_gen::CatalogEntryName"]
    direction TB
    T30_domain_domain_CatalogEntryName__self[CatalogEntryName]
    T30_domain_domain_CatalogEntryName_try_new([try_new])
    T30_domain_domain_CatalogEntryName_as_str([as_str])
    T30_domain_domain_CatalogEntryName_is_non_empty([is_non_empty])
  end
  subgraph T33_domain_domain_CatalogImportAction["tddd::catalog_gen::CatalogImportAction"]
    direction TB
    T33_domain_domain_CatalogImportAction__self[CatalogImportAction]
    T33_domain_domain_CatalogImportAction_Reference[Reference]
    T33_domain_domain_CatalogImportAction_Modify[Modify]
    T33_domain_domain_CatalogImportAction_Delete[Delete]
  end
  subgraph T31_domain_domain_CatalogueDocument["tddd::catalogue_v2::document::CatalogueDocument"]
    direction TB
    T31_domain_domain_CatalogueDocument__self[CatalogueDocument]
    T31_domain_domain_CatalogueDocument_new([new])
    T31_domain_domain_CatalogueDocument_validate_filename([validate_filename])
    T31_domain_domain_CatalogueDocument_schema_version([schema_version])
    T31_domain_domain_CatalogueDocument_crate_name([crate_name])
    T31_domain_domain_CatalogueDocument_layer([layer])
    T31_domain_domain_CatalogueDocument_types([types])
    T31_domain_domain_CatalogueDocument_traits([traits])
    T31_domain_domain_CatalogueDocument_functions([functions])
    T31_domain_domain_CatalogueDocument_inherent_impls([inherent_impls])
    T31_domain_domain_CatalogueDocument_trait_impls([trait_impls])
    T31_domain_domain_CatalogueDocument_deletions([deletions])
    T31_domain_domain_CatalogueDocument_insert_type([insert_type])
    T31_domain_domain_CatalogueDocument_insert_trait([insert_trait])
    T31_domain_domain_CatalogueDocument_insert_function([insert_function])
    T31_domain_domain_CatalogueDocument_push_inherent_impl([push_inherent_impl])
    T31_domain_domain_CatalogueDocument_push_trait_impl([push_trait_impl])
    T31_domain_domain_CatalogueDocument_push_deletion([push_deletion])
  end
  subgraph T28_domain_domain_DeletionRecord["tddd::catalogue_v2::deletions::DeletionRecord"]
    direction TB
    T28_domain_domain_DeletionRecord__self[DeletionRecord]
    T28_domain_domain_DeletionRecord_Type[Type]
    T28_domain_domain_DeletionRecord_Trait[Trait]
    T28_domain_domain_DeletionRecord_Function[Function]
  end
  subgraph T23_domain_domain_DocString["tddd::catalogue_v2::identifiers::DocString"]
    direction TB
    T23_domain_domain_DocString__self[DocString]
    T23_domain_domain_DocString_new([new])
    T23_domain_domain_DocString_as_str([as_str])
  end
  subgraph T23_domain_domain_DraftHole["tddd::catalog_gen::DraftHole"]
    direction TB
    T23_domain_domain_DraftHole__self[DraftHole]
    T23_domain_domain_DraftHole_new([new])
    T23_domain_domain_DraftHole_path([path])
    T23_domain_domain_DraftHole_instruction([instruction])
  end
  subgraph T27_domain_domain_DraftHolePath["tddd::catalog_gen::DraftHolePath"]
    direction TB
    T27_domain_domain_DraftHolePath__self[DraftHolePath]
    T27_domain_domain_DraftHolePath_try_new([try_new])
    T27_domain_domain_DraftHolePath_as_str([as_str])
    T27_domain_domain_DraftHolePath_is_non_empty([is_non_empty])
  end
  subgraph T27_domain_domain_FunctionEntry["tddd::catalogue_v2::entries::FunctionEntry"]
    direction TB
    T27_domain_domain_FunctionEntry__self[FunctionEntry]
    T27_domain_domain_FunctionEntry_new([new])
    T27_domain_domain_FunctionEntry_action([action])
    T27_domain_domain_FunctionEntry_role([role])
    T27_domain_domain_FunctionEntry_params([params])
    T27_domain_domain_FunctionEntry_returns([returns])
    T27_domain_domain_FunctionEntry_is_async([is_async])
    T27_domain_domain_FunctionEntry_generics([generics])
    T27_domain_domain_FunctionEntry_where_predicates([where_predicates])
    T27_domain_domain_FunctionEntry_docs([docs])
    T27_domain_domain_FunctionEntry_spec_refs([spec_refs])
    T27_domain_domain_FunctionEntry_informal_grounds([informal_grounds])
  end
  subgraph T32_domain_domain_InherentImplDeclV2["tddd::catalogue_v2::entries::InherentImplDeclV2"]
    direction TB
    T32_domain_domain_InherentImplDeclV2__self[InherentImplDeclV2]
  end
  subgraph T29_domain_domain_TodoInstruction["tddd::catalog_gen::TodoInstruction"]
    direction TB
    T29_domain_domain_TodoInstruction__self[TodoInstruction]
    T29_domain_domain_TodoInstruction_try_new([try_new])
    T29_domain_domain_TodoInstruction_as_str([as_str])
    T29_domain_domain_TodoInstruction_is_non_empty([is_non_empty])
  end
  subgraph T24_domain_domain_TraitEntry["tddd::catalogue_v2::entries::TraitEntry"]
    direction TB
    T24_domain_domain_TraitEntry__self[TraitEntry]
    T24_domain_domain_TraitEntry_new([new])
    T24_domain_domain_TraitEntry_action([action])
    T24_domain_domain_TraitEntry_role([role])
    T24_domain_domain_TraitEntry_methods([methods])
    T24_domain_domain_TraitEntry_assoc_types([assoc_types])
    T24_domain_domain_TraitEntry_assoc_consts([assoc_consts])
    T24_domain_domain_TraitEntry_supertrait_bounds([supertrait_bounds])
    T24_domain_domain_TraitEntry_generics([generics])
    T24_domain_domain_TraitEntry_where_predicates([where_predicates])
    T24_domain_domain_TraitEntry_module_path([module_path])
    T24_domain_domain_TraitEntry_docs([docs])
    T24_domain_domain_TraitEntry_spec_refs([spec_refs])
    T24_domain_domain_TraitEntry_informal_grounds([informal_grounds])
  end
  subgraph T29_domain_domain_TraitImplDeclV2["tddd::catalogue_v2::traits::TraitImplDeclV2"]
    direction TB
    T29_domain_domain_TraitImplDeclV2__self[TraitImplDeclV2]
    T29_domain_domain_TraitImplDeclV2_new([new])
  end
  subgraph T23_domain_domain_TypeEntry["tddd::catalogue_v2::entries::TypeEntry"]
    direction TB
    T23_domain_domain_TypeEntry__self[TypeEntry]
    T23_domain_domain_TypeEntry_new([new])
    T23_domain_domain_TypeEntry_action([action])
    T23_domain_domain_TypeEntry_role([role])
    T23_domain_domain_TypeEntry_kind([kind])
    T23_domain_domain_TypeEntry_methods([methods])
    T23_domain_domain_TypeEntry_generics([generics])
    T23_domain_domain_TypeEntry_where_predicates([where_predicates])
    T23_domain_domain_TypeEntry_module_path([module_path])
    T23_domain_domain_TypeEntry_docs([docs])
    T23_domain_domain_TypeEntry_spec_refs([spec_refs])
    T23_domain_domain_TypeEntry_informal_grounds([informal_grounds])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_catalog_gen["usecase::catalog_gen"]
    direction TB
  subgraph T33_usecase_usecase_CatalogAddCommand["catalog_gen::CatalogAddCommand"]
    direction TB
    T33_usecase_usecase_CatalogAddCommand__self[CatalogAddCommand]
  end
  subgraph T33_usecase_usecase_CatalogCheckQuery["catalog_gen::CatalogCheckQuery"]
    direction TB
    T33_usecase_usecase_CatalogCheckQuery__self[CatalogCheckQuery]
  end
  subgraph T34_usecase_usecase_CatalogCheckReport["catalog_gen::CatalogCheckReport"]
    direction TB
    T34_usecase_usecase_CatalogCheckReport__self[CatalogCheckReport]
  end
  subgraph T35_usecase_usecase_CatalogCheckVerdict["catalog_gen::CatalogCheckVerdict"]
    direction TB
    T35_usecase_usecase_CatalogCheckVerdict__self[CatalogCheckVerdict]
    T35_usecase_usecase_CatalogCheckVerdict_Pass[Pass]
    T35_usecase_usecase_CatalogCheckVerdict_Interim[Interim]
    T35_usecase_usecase_CatalogCheckVerdict_Blocked[Blocked]
    T35_usecase_usecase_CatalogCheckVerdict_Skipped[Skipped]
  end
  subgraph T34_usecase_usecase_CatalogCiteCommand["catalog_gen::CatalogCiteCommand"]
    direction TB
    T34_usecase_usecase_CatalogCiteCommand__self[CatalogCiteCommand]
  end
  subgraph T28_usecase_usecase_CatalogError["catalog_gen::CatalogError"]
    direction TB
    T28_usecase_usecase_CatalogError__self[CatalogError]
    T28_usecase_usecase_CatalogError_FileExists[FileExists]
    T28_usecase_usecase_CatalogError_FileMissing[FileMissing]
    T28_usecase_usecase_CatalogError_DuplicateEntry[DuplicateEntry]
    T28_usecase_usecase_CatalogError_AnchorNotFound[AnchorNotFound]
    T28_usecase_usecase_CatalogError_InvalidRole[InvalidRole]
    T28_usecase_usecase_CatalogError_ParseFragment[ParseFragment]
    T28_usecase_usecase_CatalogError_SchemaInvalid[SchemaInvalid]
    T28_usecase_usecase_CatalogError_DraftIncomplete[DraftIncomplete]
    T28_usecase_usecase_CatalogError_Port[Port]
  end
  subgraph T34_usecase_usecase_CatalogGateContext["catalog_gen::CatalogGateContext"]
    direction TB
    T34_usecase_usecase_CatalogGateContext__self[CatalogGateContext]
    T34_usecase_usecase_CatalogGateContext_Phase2[Phase2]
    T34_usecase_usecase_CatalogGateContext_Commit[Commit]
    T34_usecase_usecase_CatalogGateContext_Merge[Merge]
  end
  subgraph T36_usecase_usecase_CatalogImportCommand["catalog_gen::CatalogImportCommand"]
    direction TB
    T36_usecase_usecase_CatalogImportCommand__self[CatalogImportCommand]
  end
  subgraph T33_usecase_usecase_CatalogInitReport["catalog_gen::CatalogInitReport"]
    direction TB
    T33_usecase_usecase_CatalogInitReport__self[CatalogInitReport]
  end
  subgraph T33_usecase_usecase_CatalogInteractor["catalog_gen::CatalogInteractor"]
    direction TB
    T33_usecase_usecase_CatalogInteractor__self[CatalogInteractor]
    T33_usecase_usecase_CatalogInteractor_new([new])
  end
  subgraph T34_usecase_usecase_CatalogWriteReport["catalog_gen::CatalogWriteReport"]
    direction TB
    T34_usecase_usecase_CatalogWriteReport__self[CatalogWriteReport]
  end
  subgraph R27_usecase_usecase_CatalogPort["catalog_gen::CatalogPort"]
    direction TB
    R27_usecase_usecase_CatalogPort__self[CatalogPort]
    R27_usecase_usecase_CatalogPort_init([init])
    R27_usecase_usecase_CatalogPort_add([add])
    R27_usecase_usecase_CatalogPort_import([import])
    R27_usecase_usecase_CatalogPort_cite([cite])
    R27_usecase_usecase_CatalogPort_check([check])
  end
  subgraph R30_usecase_usecase_CatalogService["catalog_gen::CatalogService"]
    direction TB
    R30_usecase_usecase_CatalogService__self[CatalogService]
    R30_usecase_usecase_CatalogService_init([init])
    R30_usecase_usecase_CatalogService_add([add])
    R30_usecase_usecase_CatalogService_import([import])
    R30_usecase_usecase_CatalogService_cite([cite])
    R30_usecase_usecase_CatalogService_check([check])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_tddd["infrastructure::tddd"]
    direction TB
  subgraph T47_infrastructure_infrastructure_CatalogDraftError["tddd::catalog_gen::CatalogDraftError"]
    direction TB
    T47_infrastructure_infrastructure_CatalogDraftError__self[CatalogDraftError]
    T47_infrastructure_infrastructure_CatalogDraftError_Incomplete[Incomplete]
    T47_infrastructure_infrastructure_CatalogDraftError_Codec[Codec]
  end
  subgraph T46_infrastructure_infrastructure_FsCatalogAdapter["tddd::catalog_gen::FsCatalogAdapter"]
    direction TB
    T46_infrastructure_infrastructure_FsCatalogAdapter__self[FsCatalogAdapter]
    T46_infrastructure_infrastructure_FsCatalogAdapter_new([new])
  end
  F80_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__scan_todo_holes[[scan_todo_holes]]
  F77_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__try_complete[[try_complete]]
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_catalog_gen["cli_driver::catalog_gen"]
    direction TB
  subgraph T37_cli_driver_cli_driver_CatalogAddInput["catalog_gen::CatalogAddInput"]
    direction TB
    T37_cli_driver_cli_driver_CatalogAddInput__self[CatalogAddInput]
  end
  subgraph T39_cli_driver_cli_driver_CatalogCheckInput["catalog_gen::CatalogCheckInput"]
    direction TB
    T39_cli_driver_cli_driver_CatalogCheckInput__self[CatalogCheckInput]
  end
  subgraph T38_cli_driver_cli_driver_CatalogCiteInput["catalog_gen::CatalogCiteInput"]
    direction TB
    T38_cli_driver_cli_driver_CatalogCiteInput__self[CatalogCiteInput]
  end
  subgraph T35_cli_driver_cli_driver_CatalogDriver["catalog_gen::CatalogDriver"]
    direction TB
    T35_cli_driver_cli_driver_CatalogDriver__self[CatalogDriver]
    T35_cli_driver_cli_driver_CatalogDriver_new([new])
    T35_cli_driver_cli_driver_CatalogDriver_handle([handle])
  end
  subgraph T39_cli_driver_cli_driver_CatalogGateSelect["catalog_gen::CatalogGateSelect"]
    direction TB
    T39_cli_driver_cli_driver_CatalogGateSelect__self[CatalogGateSelect]
    T39_cli_driver_cli_driver_CatalogGateSelect_Phase2[Phase2]
    T39_cli_driver_cli_driver_CatalogGateSelect_Commit[Commit]
    T39_cli_driver_cli_driver_CatalogGateSelect_Merge[Merge]
  end
  subgraph T40_cli_driver_cli_driver_CatalogImportInput["catalog_gen::CatalogImportInput"]
    direction TB
    T40_cli_driver_cli_driver_CatalogImportInput__self[CatalogImportInput]
  end
  subgraph T41_cli_driver_cli_driver_CatalogImportSelect["catalog_gen::CatalogImportSelect"]
    direction TB
    T41_cli_driver_cli_driver_CatalogImportSelect__self[CatalogImportSelect]
    T41_cli_driver_cli_driver_CatalogImportSelect_Reference[Reference]
    T41_cli_driver_cli_driver_CatalogImportSelect_Modify[Modify]
    T41_cli_driver_cli_driver_CatalogImportSelect_Delete[Delete]
  end
  subgraph T38_cli_driver_cli_driver_CatalogInitInput["catalog_gen::CatalogInitInput"]
    direction TB
    T38_cli_driver_cli_driver_CatalogInitInput__self[CatalogInitInput]
  end
  subgraph T34_cli_driver_cli_driver_CatalogInput["catalog_gen::CatalogInput"]
    direction TB
    T34_cli_driver_cli_driver_CatalogInput__self[CatalogInput]
    T34_cli_driver_cli_driver_CatalogInput_Init[Init]
    T34_cli_driver_cli_driver_CatalogInput_Add[Add]
    T34_cli_driver_cli_driver_CatalogInput_Import[Import]
    T34_cli_driver_cli_driver_CatalogInput_Cite[Cite]
    T34_cli_driver_cli_driver_CatalogInput_Check[Check]
  end
  subgraph T39_cli_driver_cli_driver_CatalogKindSelect["catalog_gen::CatalogKindSelect"]
    direction TB
    T39_cli_driver_cli_driver_CatalogKindSelect__self[CatalogKindSelect]
    T39_cli_driver_cli_driver_CatalogKindSelect_Struct[Struct]
    T39_cli_driver_cli_driver_CatalogKindSelect_Enum[Enum]
    T39_cli_driver_cli_driver_CatalogKindSelect_TypeAlias[TypeAlias]
    T39_cli_driver_cli_driver_CatalogKindSelect_Trait[Trait]
    T39_cli_driver_cli_driver_CatalogKindSelect_Function[Function]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_catalog["cli_composition::catalog"]
    direction TB
  subgraph T54_cli_composition_cli_composition_CatalogCompositionRoot["catalog::CatalogCompositionRoot"]
    direction TB
    T54_cli_composition_cli_composition_CatalogCompositionRoot__self[CatalogCompositionRoot]
    T54_cli_composition_cli_composition_CatalogCompositionRoot_new([new])
    T54_cli_composition_cli_composition_CatalogCompositionRoot_catalog_driver([catalog_driver])
    T54_cli_composition_cli_composition_CatalogCompositionRoot_handle([handle])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph T18_cli_cli_CliCommand["CliCommand"]
    direction TB
    T18_cli_cli_CliCommand__self[CliCommand]
    T18_cli_cli_CliCommand_Arch[Arch]
    T18_cli_cli_CliCommand_Conventions[Conventions]
    T18_cli_cli_CliCommand_Domain[Domain]
    T18_cli_cli_CliCommand_Guard[Guard]
    T18_cli_cli_CliCommand_Hook[Hook]
    T18_cli_cli_CliCommand_Track[Track]
    T18_cli_cli_CliCommand_Git[Git]
    T18_cli_cli_CliCommand_Pr[Pr]
    T18_cli_cli_CliCommand_Plan[Plan]
    T18_cli_cli_CliCommand_Review[Review]
    T18_cli_cli_CliCommand_File[File]
    T18_cli_cli_CliCommand_Verify[Verify]
    T18_cli_cli_CliCommand_FindSimilar[FindSimilar]
    T18_cli_cli_CliCommand_DupIndex[DupIndex]
    T18_cli_cli_CliCommand_DupCheck[DupCheck]
    T18_cli_cli_CliCommand_Telemetry[Telemetry]
    T18_cli_cli_CliCommand_Dry[Dry]
    T18_cli_cli_CliCommand_RefVerify[RefVerify]
    T18_cli_cli_CliCommand_Signal[Signal]
    T18_cli_cli_CliCommand_TaskContract[TaskContract]
    T18_cli_cli_CliCommand_Catalog[Catalog]
    T18_cli_cli_CliCommand_CatalogueLint[CatalogueLint]
    T18_cli_cli_CliCommand_Demo[Demo]
  end
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T24_cli_cli_CatalogActionArg["commands::catalog::CatalogActionArg"]
    direction TB
    T24_cli_cli_CatalogActionArg__self[CatalogActionArg]
    T24_cli_cli_CatalogActionArg_Reference[Reference]
    T24_cli_cli_CatalogActionArg_Modify[Modify]
    T24_cli_cli_CatalogActionArg_Delete[Delete]
  end
  subgraph T22_cli_cli_CatalogAddArgs["commands::catalog::CatalogAddArgs"]
    direction TB
    T22_cli_cli_CatalogAddArgs__self[CatalogAddArgs]
  end
  subgraph T24_cli_cli_CatalogCheckArgs["commands::catalog::CatalogCheckArgs"]
    direction TB
    T24_cli_cli_CatalogCheckArgs__self[CatalogCheckArgs]
  end
  subgraph T23_cli_cli_CatalogCiteArgs["commands::catalog::CatalogCiteArgs"]
    direction TB
    T23_cli_cli_CatalogCiteArgs__self[CatalogCiteArgs]
  end
  subgraph T22_cli_cli_CatalogCommand["commands::catalog::CatalogCommand"]
    direction TB
    T22_cli_cli_CatalogCommand__self[CatalogCommand]
    T22_cli_cli_CatalogCommand_Init[Init]
    T22_cli_cli_CatalogCommand_Add[Add]
    T22_cli_cli_CatalogCommand_Import[Import]
    T22_cli_cli_CatalogCommand_Cite[Cite]
    T22_cli_cli_CatalogCommand_Check[Check]
  end
  subgraph T22_cli_cli_CatalogGateArg["commands::catalog::CatalogGateArg"]
    direction TB
    T22_cli_cli_CatalogGateArg__self[CatalogGateArg]
    T22_cli_cli_CatalogGateArg_Phase2[Phase2]
    T22_cli_cli_CatalogGateArg_Commit[Commit]
    T22_cli_cli_CatalogGateArg_Merge[Merge]
  end
  subgraph T25_cli_cli_CatalogImportArgs["commands::catalog::CatalogImportArgs"]
    direction TB
    T25_cli_cli_CatalogImportArgs__self[CatalogImportArgs]
  end
  subgraph T23_cli_cli_CatalogInitArgs["commands::catalog::CatalogInitArgs"]
    direction TB
    T23_cli_cli_CatalogInitArgs__self[CatalogInitArgs]
  end
  subgraph T22_cli_cli_CatalogKindArg["commands::catalog::CatalogKindArg"]
    direction TB
    T22_cli_cli_CatalogKindArg__self[CatalogKindArg]
    T22_cli_cli_CatalogKindArg_Struct[Struct]
    T22_cli_cli_CatalogKindArg_Enum[Enum]
    T22_cli_cli_CatalogKindArg_TypeAlias[TypeAlias]
    T22_cli_cli_CatalogKindArg_Trait[Trait]
    T22_cli_cli_CatalogKindArg_Function[Function]
  end
  F48_cli_cli_cli__commands__catalog__action_to_select[[action_to_select]]
  F40_cli_cli_cli__commands__catalog__dispatch[[dispatch]]
  F39_cli_cli_cli__commands__catalog__execute[[execute]]
  F43_cli_cli_cli__commands__catalog__execute_add[[execute_add]]
  F45_cli_cli_cli__commands__catalog__execute_check[[execute_check]]
  F44_cli_cli_cli__commands__catalog__execute_cite[[execute_cite]]
  F46_cli_cli_cli__commands__catalog__execute_import[[execute_import]]
  F44_cli_cli_cli__commands__catalog__execute_init[[execute_init]]
  F46_cli_cli_cli__commands__catalog__gate_to_select[[gate_to_select]]
  F46_cli_cli_cli__commands__catalog__kind_to_select[[kind_to_select]]
  F48_cli_cli_cli__commands__catalog__resolve_for_read[[resolve_for_read]]
  F49_cli_cli_cli__commands__catalog__resolve_for_write[[resolve_for_write]]
  end
end
T26_domain_domain_FunctionInfo_new --> T26_domain_domain_FunctionInfo__self
T26_domain_domain_FunctionInfo_with_module_path --> T26_domain_domain_FunctionInfo__self
T22_domain_domain_ImplInfo_new --o T26_domain_domain_FunctionInfo__self
T22_domain_domain_ImplInfo_new --> T22_domain_domain_ImplInfo__self
T22_domain_domain_ImplInfo_with_trait_def_path --o T26_domain_domain_FunctionInfo__self
T22_domain_domain_ImplInfo_with_trait_def_path --> T22_domain_domain_ImplInfo__self
T22_domain_domain_ImplInfo_with_target_details --o T26_domain_domain_FunctionInfo__self
T22_domain_domain_ImplInfo_with_target_details --> T22_domain_domain_ImplInfo__self
T22_domain_domain_ImplInfo_methods --> T26_domain_domain_FunctionInfo__self
T22_domain_domain_TypeInfo_new --> T22_domain_domain_TypeInfo__self
T22_domain_domain_TypeInfo_with_module_path --> T22_domain_domain_TypeInfo__self
T22_domain_domain_TypeInfo_with_alias_target --> T22_domain_domain_TypeInfo__self
T22_domain_domain_TypeInfo_with_struct_shape --o T29_domain_domain_StructShapeKind__self
T22_domain_domain_TypeInfo_with_struct_shape --> T22_domain_domain_TypeInfo__self
T22_domain_domain_TypeInfo_struct_shape --> T29_domain_domain_StructShapeKind__self
T30_domain_domain_CatalogEntryName_try_new --> T30_domain_domain_CatalogEntryName__self
T31_domain_domain_CatalogueDocument_new --> T31_domain_domain_CatalogueDocument__self
T31_domain_domain_CatalogueDocument_types --> T23_domain_domain_TypeEntry__self
T31_domain_domain_CatalogueDocument_traits --> T24_domain_domain_TraitEntry__self
T31_domain_domain_CatalogueDocument_functions --> T27_domain_domain_FunctionEntry__self
T31_domain_domain_CatalogueDocument_inherent_impls --> T32_domain_domain_InherentImplDeclV2__self
T31_domain_domain_CatalogueDocument_trait_impls --> T29_domain_domain_TraitImplDeclV2__self
T31_domain_domain_CatalogueDocument_deletions --> T28_domain_domain_DeletionRecord__self
T31_domain_domain_CatalogueDocument_insert_type --o T23_domain_domain_TypeEntry__self
T31_domain_domain_CatalogueDocument_insert_trait --o T24_domain_domain_TraitEntry__self
T31_domain_domain_CatalogueDocument_insert_function --o T27_domain_domain_FunctionEntry__self
T31_domain_domain_CatalogueDocument_push_inherent_impl --o T32_domain_domain_InherentImplDeclV2__self
T31_domain_domain_CatalogueDocument_push_trait_impl --o T29_domain_domain_TraitImplDeclV2__self
T31_domain_domain_CatalogueDocument_push_deletion --o T28_domain_domain_DeletionRecord__self
T23_domain_domain_DocString_new --> T23_domain_domain_DocString__self
T23_domain_domain_DraftHole_new --o T27_domain_domain_DraftHolePath__self
T23_domain_domain_DraftHole_new --o T29_domain_domain_TodoInstruction__self
T23_domain_domain_DraftHole_new --> T23_domain_domain_DraftHole__self
T23_domain_domain_DraftHole_path --> T27_domain_domain_DraftHolePath__self
T23_domain_domain_DraftHole_instruction --> T29_domain_domain_TodoInstruction__self
T27_domain_domain_DraftHolePath_try_new --> T27_domain_domain_DraftHolePath__self
T27_domain_domain_FunctionEntry_new --o T23_domain_domain_DocString__self
T27_domain_domain_FunctionEntry_new --> T27_domain_domain_FunctionEntry__self
T27_domain_domain_FunctionEntry_docs --> T23_domain_domain_DocString__self
T29_domain_domain_TodoInstruction_try_new --> T29_domain_domain_TodoInstruction__self
T24_domain_domain_TraitEntry_new --o T23_domain_domain_DocString__self
T24_domain_domain_TraitEntry_new --> T24_domain_domain_TraitEntry__self
T24_domain_domain_TraitEntry_docs --> T23_domain_domain_DocString__self
T29_domain_domain_TraitImplDeclV2_new --> T29_domain_domain_TraitImplDeclV2__self
T23_domain_domain_TypeEntry_new --o T23_domain_domain_DocString__self
T23_domain_domain_TypeEntry_new --> T23_domain_domain_TypeEntry__self
T23_domain_domain_TypeEntry_docs --> T23_domain_domain_DocString__self
T33_usecase_usecase_CatalogAddCommand__self --o|kind| T30_domain_domain_CatalogEntryKind__self
T33_usecase_usecase_CatalogCheckQuery__self --o|gate| T34_usecase_usecase_CatalogGateContext__self
T34_usecase_usecase_CatalogCheckReport__self --o|verdict| T35_usecase_usecase_CatalogCheckVerdict__self
T34_usecase_usecase_CatalogCheckReport__self --o|remaining_holes| T23_domain_domain_DraftHole__self
T28_usecase_usecase_CatalogError_DuplicateEntry --o|entry_key| T30_domain_domain_CatalogEntryName__self
T28_usecase_usecase_CatalogError_DraftIncomplete --o|holes| T23_domain_domain_DraftHole__self
T36_usecase_usecase_CatalogImportCommand__self --o|action| T33_domain_domain_CatalogImportAction__self
T33_usecase_usecase_CatalogInteractor_new --> T33_usecase_usecase_CatalogInteractor__self
T34_usecase_usecase_CatalogWriteReport__self --o|holes| T23_domain_domain_DraftHole__self
R27_usecase_usecase_CatalogPort_init --> T28_usecase_usecase_CatalogError__self
R27_usecase_usecase_CatalogPort_init --> T33_usecase_usecase_CatalogInitReport__self
R27_usecase_usecase_CatalogPort_add --o T33_usecase_usecase_CatalogAddCommand__self
R27_usecase_usecase_CatalogPort_add --> T28_usecase_usecase_CatalogError__self
R27_usecase_usecase_CatalogPort_add --> T34_usecase_usecase_CatalogWriteReport__self
R27_usecase_usecase_CatalogPort_import --o T36_usecase_usecase_CatalogImportCommand__self
R27_usecase_usecase_CatalogPort_import --> T28_usecase_usecase_CatalogError__self
R27_usecase_usecase_CatalogPort_import --> T34_usecase_usecase_CatalogWriteReport__self
R27_usecase_usecase_CatalogPort_cite --o T34_usecase_usecase_CatalogCiteCommand__self
R27_usecase_usecase_CatalogPort_cite --> T28_usecase_usecase_CatalogError__self
R27_usecase_usecase_CatalogPort_cite --> T34_usecase_usecase_CatalogWriteReport__self
R27_usecase_usecase_CatalogPort_check --o T33_usecase_usecase_CatalogCheckQuery__self
R27_usecase_usecase_CatalogPort_check --> T34_usecase_usecase_CatalogCheckReport__self
R27_usecase_usecase_CatalogPort_check --> T28_usecase_usecase_CatalogError__self
R30_usecase_usecase_CatalogService_init --> T28_usecase_usecase_CatalogError__self
R30_usecase_usecase_CatalogService_init --> T33_usecase_usecase_CatalogInitReport__self
R30_usecase_usecase_CatalogService_add --o T33_usecase_usecase_CatalogAddCommand__self
R30_usecase_usecase_CatalogService_add --> T28_usecase_usecase_CatalogError__self
R30_usecase_usecase_CatalogService_add --> T34_usecase_usecase_CatalogWriteReport__self
R30_usecase_usecase_CatalogService_import --o T36_usecase_usecase_CatalogImportCommand__self
R30_usecase_usecase_CatalogService_import --> T28_usecase_usecase_CatalogError__self
R30_usecase_usecase_CatalogService_import --> T34_usecase_usecase_CatalogWriteReport__self
R30_usecase_usecase_CatalogService_cite --o T34_usecase_usecase_CatalogCiteCommand__self
R30_usecase_usecase_CatalogService_cite --> T28_usecase_usecase_CatalogError__self
R30_usecase_usecase_CatalogService_cite --> T34_usecase_usecase_CatalogWriteReport__self
R30_usecase_usecase_CatalogService_check --o T33_usecase_usecase_CatalogCheckQuery__self
R30_usecase_usecase_CatalogService_check --> T34_usecase_usecase_CatalogCheckReport__self
R30_usecase_usecase_CatalogService_check --> T28_usecase_usecase_CatalogError__self
T33_usecase_usecase_CatalogInteractor__self -.impl.-> R30_usecase_usecase_CatalogService__self
T47_infrastructure_infrastructure_CatalogDraftError_Incomplete --o|holes| T23_domain_domain_DraftHole__self
T46_infrastructure_infrastructure_FsCatalogAdapter_new --> T46_infrastructure_infrastructure_FsCatalogAdapter__self
F80_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__scan_todo_holes --> T23_domain_domain_DraftHole__self
F77_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__try_complete --> T47_infrastructure_infrastructure_CatalogDraftError__self
F77_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__try_complete --> T31_domain_domain_CatalogueDocument__self
T46_infrastructure_infrastructure_FsCatalogAdapter__self -.impl.-> R27_usecase_usecase_CatalogPort__self
T37_cli_driver_cli_driver_CatalogAddInput__self --o|kind| T39_cli_driver_cli_driver_CatalogKindSelect__self
T39_cli_driver_cli_driver_CatalogCheckInput__self --o|gate| T39_cli_driver_cli_driver_CatalogGateSelect__self
T35_cli_driver_cli_driver_CatalogDriver_new --> T35_cli_driver_cli_driver_CatalogDriver__self
T35_cli_driver_cli_driver_CatalogDriver_handle --o T34_cli_driver_cli_driver_CatalogInput__self
T40_cli_driver_cli_driver_CatalogImportInput__self --o|action| T41_cli_driver_cli_driver_CatalogImportSelect__self
T34_cli_driver_cli_driver_CatalogInput_Init --o T38_cli_driver_cli_driver_CatalogInitInput__self
T34_cli_driver_cli_driver_CatalogInput_Add --o T37_cli_driver_cli_driver_CatalogAddInput__self
T34_cli_driver_cli_driver_CatalogInput_Import --o T40_cli_driver_cli_driver_CatalogImportInput__self
T34_cli_driver_cli_driver_CatalogInput_Cite --o T38_cli_driver_cli_driver_CatalogCiteInput__self
T34_cli_driver_cli_driver_CatalogInput_Check --o T39_cli_driver_cli_driver_CatalogCheckInput__self
T54_cli_composition_cli_composition_CatalogCompositionRoot_new --> T54_cli_composition_cli_composition_CatalogCompositionRoot__self
T54_cli_composition_cli_composition_CatalogCompositionRoot_catalog_driver --> T35_cli_driver_cli_driver_CatalogDriver__self
T54_cli_composition_cli_composition_CatalogCompositionRoot_handle --o T34_cli_driver_cli_driver_CatalogInput__self
T18_cli_cli_CliCommand_Catalog --o|cmd| T22_cli_cli_CatalogCommand__self
T22_cli_cli_CatalogAddArgs__self --o|kind| T22_cli_cli_CatalogKindArg__self
T24_cli_cli_CatalogCheckArgs__self --o|gate| T22_cli_cli_CatalogGateArg__self
T22_cli_cli_CatalogCommand_Init --o T23_cli_cli_CatalogInitArgs__self
T22_cli_cli_CatalogCommand_Add --o T22_cli_cli_CatalogAddArgs__self
T22_cli_cli_CatalogCommand_Import --o T25_cli_cli_CatalogImportArgs__self
T22_cli_cli_CatalogCommand_Cite --o T23_cli_cli_CatalogCiteArgs__self
T22_cli_cli_CatalogCommand_Check --o T24_cli_cli_CatalogCheckArgs__self
T25_cli_cli_CatalogImportArgs__self --o|action| T24_cli_cli_CatalogActionArg__self
F48_cli_cli_cli__commands__catalog__action_to_select --o T24_cli_cli_CatalogActionArg__self
F48_cli_cli_cli__commands__catalog__action_to_select --> T41_cli_driver_cli_driver_CatalogImportSelect__self
F40_cli_cli_cli__commands__catalog__dispatch --o T34_cli_driver_cli_driver_CatalogInput__self
F39_cli_cli_cli__commands__catalog__execute --o T22_cli_cli_CatalogCommand__self
F43_cli_cli_cli__commands__catalog__execute_add --o T22_cli_cli_CatalogAddArgs__self
F45_cli_cli_cli__commands__catalog__execute_check --o T24_cli_cli_CatalogCheckArgs__self
F44_cli_cli_cli__commands__catalog__execute_cite --o T23_cli_cli_CatalogCiteArgs__self
F46_cli_cli_cli__commands__catalog__execute_import --o T25_cli_cli_CatalogImportArgs__self
F44_cli_cli_cli__commands__catalog__execute_init --o T23_cli_cli_CatalogInitArgs__self
F46_cli_cli_cli__commands__catalog__gate_to_select --o T22_cli_cli_CatalogGateArg__self
F46_cli_cli_cli__commands__catalog__gate_to_select --> T39_cli_driver_cli_driver_CatalogGateSelect__self
F46_cli_cli_cli__commands__catalog__kind_to_select --o T22_cli_cli_CatalogKindArg__self
F46_cli_cli_cli__commands__catalog__kind_to_select --> T39_cli_driver_cli_driver_CatalogKindSelect__self
class T26_domain_domain_FunctionInfo_new method_node
class T26_domain_domain_FunctionInfo_with_module_path method_node
class T26_domain_domain_FunctionInfo_name method_node
class T26_domain_domain_FunctionInfo_docs method_node
class T26_domain_domain_FunctionInfo_return_type_names method_node
class T26_domain_domain_FunctionInfo_has_self_receiver method_node
class T26_domain_domain_FunctionInfo_params method_node
class T26_domain_domain_FunctionInfo_returns method_node
class T26_domain_domain_FunctionInfo_receiver method_node
class T26_domain_domain_FunctionInfo_is_async method_node
class T26_domain_domain_FunctionInfo_module_path method_node
class T26_domain_domain_FunctionInfo__self value_object
class T22_domain_domain_ImplInfo_new method_node
class T22_domain_domain_ImplInfo_with_trait_def_path method_node
class T22_domain_domain_ImplInfo_with_target_details method_node
class T22_domain_domain_ImplInfo_target_type method_node
class T22_domain_domain_ImplInfo_trait_name method_node
class T22_domain_domain_ImplInfo_methods method_node
class T22_domain_domain_ImplInfo_trait_def_path method_node
class T22_domain_domain_ImplInfo_target_module_path method_node
class T22_domain_domain_ImplInfo__self value_object
class T29_domain_domain_StructShapeKind_Unit variant_node
class T29_domain_domain_StructShapeKind_Tuple variant_node
class T29_domain_domain_StructShapeKind_Plain variant_node
class T29_domain_domain_StructShapeKind__self value_object
class T22_domain_domain_TypeInfo_new method_node
class T22_domain_domain_TypeInfo_with_module_path method_node
class T22_domain_domain_TypeInfo_with_alias_target method_node
class T22_domain_domain_TypeInfo_with_struct_shape method_node
class T22_domain_domain_TypeInfo_name method_node
class T22_domain_domain_TypeInfo_kind method_node
class T22_domain_domain_TypeInfo_docs method_node
class T22_domain_domain_TypeInfo_members method_node
class T22_domain_domain_TypeInfo_module_path method_node
class T22_domain_domain_TypeInfo_alias_target method_node
class T22_domain_domain_TypeInfo_struct_shape method_node
class T22_domain_domain_TypeInfo_struct_shape_matches_kind method_node
class T22_domain_domain_TypeInfo__self value_object
class T30_domain_domain_CatalogEntryKind_Struct variant_node
class T30_domain_domain_CatalogEntryKind_Enum variant_node
class T30_domain_domain_CatalogEntryKind_TypeAlias variant_node
class T30_domain_domain_CatalogEntryKind_Trait variant_node
class T30_domain_domain_CatalogEntryKind_Function variant_node
class T30_domain_domain_CatalogEntryKind__self value_object
class T30_domain_domain_CatalogEntryName_try_new method_node
class T30_domain_domain_CatalogEntryName_as_str method_node
class T30_domain_domain_CatalogEntryName_is_non_empty method_node
class T30_domain_domain_CatalogEntryName__self value_object
class T33_domain_domain_CatalogImportAction_Reference variant_node
class T33_domain_domain_CatalogImportAction_Modify variant_node
class T33_domain_domain_CatalogImportAction_Delete variant_node
class T33_domain_domain_CatalogImportAction__self value_object
class T31_domain_domain_CatalogueDocument_new method_node
class T31_domain_domain_CatalogueDocument_validate_filename method_node
class T31_domain_domain_CatalogueDocument_schema_version method_node
class T31_domain_domain_CatalogueDocument_crate_name method_node
class T31_domain_domain_CatalogueDocument_layer method_node
class T31_domain_domain_CatalogueDocument_types method_node
class T31_domain_domain_CatalogueDocument_traits method_node
class T31_domain_domain_CatalogueDocument_functions method_node
class T31_domain_domain_CatalogueDocument_inherent_impls method_node
class T31_domain_domain_CatalogueDocument_trait_impls method_node
class T31_domain_domain_CatalogueDocument_deletions method_node
class T31_domain_domain_CatalogueDocument_insert_type method_node
class T31_domain_domain_CatalogueDocument_insert_trait method_node
class T31_domain_domain_CatalogueDocument_insert_function method_node
class T31_domain_domain_CatalogueDocument_push_inherent_impl method_node
class T31_domain_domain_CatalogueDocument_push_trait_impl method_node
class T31_domain_domain_CatalogueDocument_push_deletion method_node
class T31_domain_domain_CatalogueDocument__self domain_service
class T28_domain_domain_DeletionRecord_Type variant_node
class T28_domain_domain_DeletionRecord_Trait variant_node
class T28_domain_domain_DeletionRecord_Function variant_node
class T28_domain_domain_DeletionRecord__self value_object
class T23_domain_domain_DocString_new method_node
class T23_domain_domain_DocString_as_str method_node
class T23_domain_domain_DocString__self value_object
class T23_domain_domain_DraftHole_new method_node
class T23_domain_domain_DraftHole_path method_node
class T23_domain_domain_DraftHole_instruction method_node
class T23_domain_domain_DraftHole__self value_object
class T27_domain_domain_DraftHolePath_try_new method_node
class T27_domain_domain_DraftHolePath_as_str method_node
class T27_domain_domain_DraftHolePath_is_non_empty method_node
class T27_domain_domain_DraftHolePath__self value_object
class T27_domain_domain_FunctionEntry_new method_node
class T27_domain_domain_FunctionEntry_action method_node
class T27_domain_domain_FunctionEntry_role method_node
class T27_domain_domain_FunctionEntry_params method_node
class T27_domain_domain_FunctionEntry_returns method_node
class T27_domain_domain_FunctionEntry_is_async method_node
class T27_domain_domain_FunctionEntry_generics method_node
class T27_domain_domain_FunctionEntry_where_predicates method_node
class T27_domain_domain_FunctionEntry_docs method_node
class T27_domain_domain_FunctionEntry_spec_refs method_node
class T27_domain_domain_FunctionEntry_informal_grounds method_node
class T27_domain_domain_FunctionEntry__self value_object
class T32_domain_domain_InherentImplDeclV2__self value_object
class T29_domain_domain_TodoInstruction_try_new method_node
class T29_domain_domain_TodoInstruction_as_str method_node
class T29_domain_domain_TodoInstruction_is_non_empty method_node
class T29_domain_domain_TodoInstruction__self value_object
class T24_domain_domain_TraitEntry_new method_node
class T24_domain_domain_TraitEntry_action method_node
class T24_domain_domain_TraitEntry_role method_node
class T24_domain_domain_TraitEntry_methods method_node
class T24_domain_domain_TraitEntry_assoc_types method_node
class T24_domain_domain_TraitEntry_assoc_consts method_node
class T24_domain_domain_TraitEntry_supertrait_bounds method_node
class T24_domain_domain_TraitEntry_generics method_node
class T24_domain_domain_TraitEntry_where_predicates method_node
class T24_domain_domain_TraitEntry_module_path method_node
class T24_domain_domain_TraitEntry_docs method_node
class T24_domain_domain_TraitEntry_spec_refs method_node
class T24_domain_domain_TraitEntry_informal_grounds method_node
class T24_domain_domain_TraitEntry__self value_object
class T29_domain_domain_TraitImplDeclV2_new method_node
class T29_domain_domain_TraitImplDeclV2__self value_object
class T23_domain_domain_TypeEntry_new method_node
class T23_domain_domain_TypeEntry_action method_node
class T23_domain_domain_TypeEntry_role method_node
class T23_domain_domain_TypeEntry_kind method_node
class T23_domain_domain_TypeEntry_methods method_node
class T23_domain_domain_TypeEntry_generics method_node
class T23_domain_domain_TypeEntry_where_predicates method_node
class T23_domain_domain_TypeEntry_module_path method_node
class T23_domain_domain_TypeEntry_docs method_node
class T23_domain_domain_TypeEntry_spec_refs method_node
class T23_domain_domain_TypeEntry_informal_grounds method_node
class T23_domain_domain_TypeEntry__self value_object
class T33_usecase_usecase_CatalogAddCommand__self command
class T33_usecase_usecase_CatalogCheckQuery__self query
class T34_usecase_usecase_CatalogCheckReport__self dto
class T35_usecase_usecase_CatalogCheckVerdict_Pass variant_node
class T35_usecase_usecase_CatalogCheckVerdict_Interim variant_node
class T35_usecase_usecase_CatalogCheckVerdict_Blocked variant_node
class T35_usecase_usecase_CatalogCheckVerdict_Skipped variant_node
class T35_usecase_usecase_CatalogCheckVerdict__self value_object
class T34_usecase_usecase_CatalogCiteCommand__self command
class T28_usecase_usecase_CatalogError_FileExists variant_node
class T28_usecase_usecase_CatalogError_FileMissing variant_node
class T28_usecase_usecase_CatalogError_DuplicateEntry variant_node
class T28_usecase_usecase_CatalogError_AnchorNotFound variant_node
class T28_usecase_usecase_CatalogError_InvalidRole variant_node
class T28_usecase_usecase_CatalogError_ParseFragment variant_node
class T28_usecase_usecase_CatalogError_SchemaInvalid variant_node
class T28_usecase_usecase_CatalogError_DraftIncomplete variant_node
class T28_usecase_usecase_CatalogError_Port variant_node
class T28_usecase_usecase_CatalogError__self error_type
class T34_usecase_usecase_CatalogGateContext_Phase2 variant_node
class T34_usecase_usecase_CatalogGateContext_Commit variant_node
class T34_usecase_usecase_CatalogGateContext_Merge variant_node
class T34_usecase_usecase_CatalogGateContext__self value_object
class T36_usecase_usecase_CatalogImportCommand__self command
class T33_usecase_usecase_CatalogInitReport__self dto
class T33_usecase_usecase_CatalogInteractor_new method_node
class T33_usecase_usecase_CatalogInteractor__self interactor
class T34_usecase_usecase_CatalogWriteReport__self dto
class R27_usecase_usecase_CatalogPort_init method_node
class R27_usecase_usecase_CatalogPort_add method_node
class R27_usecase_usecase_CatalogPort_import method_node
class R27_usecase_usecase_CatalogPort_cite method_node
class R27_usecase_usecase_CatalogPort_check method_node
class R27_usecase_usecase_CatalogPort__self secondary_port
class R30_usecase_usecase_CatalogService_init method_node
class R30_usecase_usecase_CatalogService_add method_node
class R30_usecase_usecase_CatalogService_import method_node
class R30_usecase_usecase_CatalogService_cite method_node
class R30_usecase_usecase_CatalogService_check method_node
class R30_usecase_usecase_CatalogService__self app_service
class T47_infrastructure_infrastructure_CatalogDraftError_Incomplete variant_node
class T47_infrastructure_infrastructure_CatalogDraftError_Codec variant_node
class T47_infrastructure_infrastructure_CatalogDraftError__self error_type
class T46_infrastructure_infrastructure_FsCatalogAdapter_new method_node
class T46_infrastructure_infrastructure_FsCatalogAdapter__self secondary_adapter
class F80_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__scan_todo_holes free_function
class F80_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__scan_todo_holes function_node
class F77_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__try_complete free_function
class F77_infrastructure_infrastructure_infrastructure__tddd__catalog_gen__try_complete function_node
class T37_cli_driver_cli_driver_CatalogAddInput__self dto
class T39_cli_driver_cli_driver_CatalogCheckInput__self dto
class T38_cli_driver_cli_driver_CatalogCiteInput__self dto
class T35_cli_driver_cli_driver_CatalogDriver_new method_node
class T35_cli_driver_cli_driver_CatalogDriver_handle method_node
class T39_cli_driver_cli_driver_CatalogGateSelect_Phase2 variant_node
class T39_cli_driver_cli_driver_CatalogGateSelect_Commit variant_node
class T39_cli_driver_cli_driver_CatalogGateSelect_Merge variant_node
class T39_cli_driver_cli_driver_CatalogGateSelect__self dto
class T40_cli_driver_cli_driver_CatalogImportInput__self dto
class T41_cli_driver_cli_driver_CatalogImportSelect_Reference variant_node
class T41_cli_driver_cli_driver_CatalogImportSelect_Modify variant_node
class T41_cli_driver_cli_driver_CatalogImportSelect_Delete variant_node
class T41_cli_driver_cli_driver_CatalogImportSelect__self dto
class T38_cli_driver_cli_driver_CatalogInitInput__self dto
class T34_cli_driver_cli_driver_CatalogInput_Init variant_node
class T34_cli_driver_cli_driver_CatalogInput_Add variant_node
class T34_cli_driver_cli_driver_CatalogInput_Import variant_node
class T34_cli_driver_cli_driver_CatalogInput_Cite variant_node
class T34_cli_driver_cli_driver_CatalogInput_Check variant_node
class T34_cli_driver_cli_driver_CatalogInput__self dto
class T39_cli_driver_cli_driver_CatalogKindSelect_Struct variant_node
class T39_cli_driver_cli_driver_CatalogKindSelect_Enum variant_node
class T39_cli_driver_cli_driver_CatalogKindSelect_TypeAlias variant_node
class T39_cli_driver_cli_driver_CatalogKindSelect_Trait variant_node
class T39_cli_driver_cli_driver_CatalogKindSelect_Function variant_node
class T39_cli_driver_cli_driver_CatalogKindSelect__self dto
class T54_cli_composition_cli_composition_CatalogCompositionRoot_new method_node
class T54_cli_composition_cli_composition_CatalogCompositionRoot_catalog_driver method_node
class T54_cli_composition_cli_composition_CatalogCompositionRoot_handle method_node
class T18_cli_cli_CliCommand_Arch variant_node
class T18_cli_cli_CliCommand_Conventions variant_node
class T18_cli_cli_CliCommand_Domain variant_node
class T18_cli_cli_CliCommand_Guard variant_node
class T18_cli_cli_CliCommand_Hook variant_node
class T18_cli_cli_CliCommand_Track variant_node
class T18_cli_cli_CliCommand_Git variant_node
class T18_cli_cli_CliCommand_Pr variant_node
class T18_cli_cli_CliCommand_Plan variant_node
class T18_cli_cli_CliCommand_Review variant_node
class T18_cli_cli_CliCommand_File variant_node
class T18_cli_cli_CliCommand_Verify variant_node
class T18_cli_cli_CliCommand_FindSimilar variant_node
class T18_cli_cli_CliCommand_DupIndex variant_node
class T18_cli_cli_CliCommand_DupCheck variant_node
class T18_cli_cli_CliCommand_Telemetry variant_node
class T18_cli_cli_CliCommand_Dry variant_node
class T18_cli_cli_CliCommand_RefVerify variant_node
class T18_cli_cli_CliCommand_Signal variant_node
class T18_cli_cli_CliCommand_TaskContract variant_node
class T18_cli_cli_CliCommand_Catalog variant_node
class T18_cli_cli_CliCommand_CatalogueLint variant_node
class T18_cli_cli_CliCommand_Demo variant_node
class T18_cli_cli_CliCommand__self dto
class T24_cli_cli_CatalogActionArg_Reference variant_node
class T24_cli_cli_CatalogActionArg_Modify variant_node
class T24_cli_cli_CatalogActionArg_Delete variant_node
class T24_cli_cli_CatalogActionArg__self dto
class T22_cli_cli_CatalogAddArgs__self dto
class T24_cli_cli_CatalogCheckArgs__self dto
class T23_cli_cli_CatalogCiteArgs__self dto
class T22_cli_cli_CatalogCommand_Init variant_node
class T22_cli_cli_CatalogCommand_Add variant_node
class T22_cli_cli_CatalogCommand_Import variant_node
class T22_cli_cli_CatalogCommand_Cite variant_node
class T22_cli_cli_CatalogCommand_Check variant_node
class T22_cli_cli_CatalogCommand__self dto
class T22_cli_cli_CatalogGateArg_Phase2 variant_node
class T22_cli_cli_CatalogGateArg_Commit variant_node
class T22_cli_cli_CatalogGateArg_Merge variant_node
class T22_cli_cli_CatalogGateArg__self dto
class T25_cli_cli_CatalogImportArgs__self dto
class T23_cli_cli_CatalogInitArgs__self dto
class T22_cli_cli_CatalogKindArg_Struct variant_node
class T22_cli_cli_CatalogKindArg_Enum variant_node
class T22_cli_cli_CatalogKindArg_TypeAlias variant_node
class T22_cli_cli_CatalogKindArg_Trait variant_node
class T22_cli_cli_CatalogKindArg_Function variant_node
class T22_cli_cli_CatalogKindArg__self dto
class F48_cli_cli_cli__commands__catalog__action_to_select free_function
class F48_cli_cli_cli__commands__catalog__action_to_select function_node
class F40_cli_cli_cli__commands__catalog__dispatch free_function
class F40_cli_cli_cli__commands__catalog__dispatch function_node
class F39_cli_cli_cli__commands__catalog__execute free_function
class F39_cli_cli_cli__commands__catalog__execute function_node
class F43_cli_cli_cli__commands__catalog__execute_add free_function
class F43_cli_cli_cli__commands__catalog__execute_add function_node
class F45_cli_cli_cli__commands__catalog__execute_check free_function
class F45_cli_cli_cli__commands__catalog__execute_check function_node
class F44_cli_cli_cli__commands__catalog__execute_cite free_function
class F44_cli_cli_cli__commands__catalog__execute_cite function_node
class F46_cli_cli_cli__commands__catalog__execute_import free_function
class F46_cli_cli_cli__commands__catalog__execute_import function_node
class F44_cli_cli_cli__commands__catalog__execute_init free_function
class F44_cli_cli_cli__commands__catalog__execute_init function_node
class F46_cli_cli_cli__commands__catalog__gate_to_select free_function
class F46_cli_cli_cli__commands__catalog__gate_to_select function_node
class F46_cli_cli_cli__commands__catalog__kind_to_select free_function
class F46_cli_cli_cli__commands__catalog__kind_to_select function_node
class F48_cli_cli_cli__commands__catalog__resolve_for_read free_function
class F48_cli_cli_cli__commands__catalog__resolve_for_read function_node
class F49_cli_cli_cli__commands__catalog__resolve_for_write free_function
class F49_cli_cli_cli__commands__catalog__resolve_for_write function_node
```
