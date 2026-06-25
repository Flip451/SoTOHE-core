<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
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
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T22_domain_domain_DataRole["tddd::catalogue_v2::roles::DataRole"]
    direction TB
    T22_domain_domain_DataRole__self[DataRole]
    T22_domain_domain_DataRole_ValueObject[ValueObject]
    T22_domain_domain_DataRole_Entity[Entity]
    T22_domain_domain_DataRole_AggregateRoot[AggregateRoot]
    T22_domain_domain_DataRole_DomainService[DomainService]
    T22_domain_domain_DataRole_Specification[Specification]
    T22_domain_domain_DataRole_Factory[Factory]
    T22_domain_domain_DataRole_UseCase[UseCase]
    T22_domain_domain_DataRole_Interactor[Interactor]
    T22_domain_domain_DataRole_Command[Command]
    T22_domain_domain_DataRole_Query[Query]
    T22_domain_domain_DataRole_Dto[Dto]
    T22_domain_domain_DataRole_ErrorType[ErrorType]
    T22_domain_domain_DataRole_SecondaryAdapter[SecondaryAdapter]
    T22_domain_domain_DataRole_DomainEvent[DomainEvent]
    T22_domain_domain_DataRole_EventPolicy[EventPolicy]
    T22_domain_domain_DataRole_CompositionRoot[CompositionRoot]
    T22_domain_domain_DataRole_PrimaryAdapter[PrimaryAdapter]
    T22_domain_domain_DataRole_value_object([value_object])
    T22_domain_domain_DataRole_entity([entity])
    T22_domain_domain_DataRole_aggregate_root([aggregate_root])
    T22_domain_domain_DataRole_domain_service([domain_service])
    T22_domain_domain_DataRole_use_case([use_case])
    T22_domain_domain_DataRole_variant_name([variant_name])
  end
  subgraph T22_domain_domain_RoleKind["tddd::catalogue_linter::RoleKind"]
    direction TB
    T22_domain_domain_RoleKind__self[RoleKind]
    T22_domain_domain_RoleKind_ValueObject[ValueObject]
    T22_domain_domain_RoleKind_Entity[Entity]
    T22_domain_domain_RoleKind_AggregateRoot[AggregateRoot]
    T22_domain_domain_RoleKind_DomainService[DomainService]
    T22_domain_domain_RoleKind_Specification[Specification]
    T22_domain_domain_RoleKind_Factory[Factory]
    T22_domain_domain_RoleKind_UseCase[UseCase]
    T22_domain_domain_RoleKind_Interactor[Interactor]
    T22_domain_domain_RoleKind_Command[Command]
    T22_domain_domain_RoleKind_Query[Query]
    T22_domain_domain_RoleKind_Dto[Dto]
    T22_domain_domain_RoleKind_ErrorType[ErrorType]
    T22_domain_domain_RoleKind_SecondaryAdapter[SecondaryAdapter]
    T22_domain_domain_RoleKind_DomainEvent[DomainEvent]
    T22_domain_domain_RoleKind_EventPolicy[EventPolicy]
    T22_domain_domain_RoleKind_SpecificationPort[SpecificationPort]
    T22_domain_domain_RoleKind_ApplicationService[ApplicationService]
    T22_domain_domain_RoleKind_SecondaryPort[SecondaryPort]
    T22_domain_domain_RoleKind_Repository[Repository]
    T22_domain_domain_RoleKind_CompositionRoot[CompositionRoot]
    T22_domain_domain_RoleKind_PrimaryAdapter[PrimaryAdapter]
    T22_domain_domain_RoleKind_FreeFunction[FreeFunction]
    T22_domain_domain_RoleKind_UseCaseFunction[UseCaseFunction]
    T22_domain_domain_RoleKind_from_data_role([from_data_role])
    T22_domain_domain_RoleKind_from_contract_role([from_contract_role])
    T22_domain_domain_RoleKind_from_function_role([from_function_role])
    T22_domain_domain_RoleKind_variant_name([variant_name])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_arch["usecase::arch"]
    direction TB
  subgraph T30_usecase_usecase_ArchInteractor["arch::ArchInteractor"]
    direction TB
    T30_usecase_usecase_ArchInteractor__self[ArchInteractor]
    T30_usecase_usecase_ArchInteractor_new([new])
  end
  subgraph R27_usecase_usecase_ArchService["arch::ArchService"]
    direction TB
    R27_usecase_usecase_ArchService__self[ArchService]
    R27_usecase_usecase_ArchService_render_tree([render_tree])
    R27_usecase_usecase_ArchService_render_tree_full([render_tree_full])
    R27_usecase_usecase_ArchService_render_members([render_members])
    R27_usecase_usecase_ArchService_render_direct_checks([render_direct_checks])
  end
  end
  subgraph usecase_usecase_module_conventions["usecase::conventions"]
    direction TB
  subgraph T37_usecase_usecase_ConventionsInteractor["conventions::ConventionsInteractor"]
    direction TB
    T37_usecase_usecase_ConventionsInteractor__self[ConventionsInteractor]
    T37_usecase_usecase_ConventionsInteractor_new([new])
  end
  subgraph R34_usecase_usecase_ConventionsService["conventions::ConventionsService"]
    direction TB
    R34_usecase_usecase_ConventionsService__self[ConventionsService]
    R34_usecase_usecase_ConventionsService_add_convention([add_convention])
    R34_usecase_usecase_ConventionsService_update_index([update_index])
    R34_usecase_usecase_ConventionsService_verify_index([verify_index])
  end
  end
  subgraph usecase_usecase_module_file["usecase::file"]
    direction TB
  subgraph T30_usecase_usecase_FileInteractor["file::FileInteractor"]
    direction TB
    T30_usecase_usecase_FileInteractor__self[FileInteractor]
    T30_usecase_usecase_FileInteractor_new([new])
  end
  subgraph R27_usecase_usecase_FileService["file::FileService"]
    direction TB
    R27_usecase_usecase_FileService__self[FileService]
    R27_usecase_usecase_FileService_write_atomic([write_atomic])
  end
  end
  subgraph usecase_usecase_module_planner["usecase::planner"]
    direction TB
  subgraph T29_usecase_usecase_PlanRunOutput["planner::PlanRunOutput"]
    direction TB
    T29_usecase_usecase_PlanRunOutput__self[PlanRunOutput]
  end
  subgraph T33_usecase_usecase_PlannerInteractor["planner::PlannerInteractor"]
    direction TB
    T33_usecase_usecase_PlannerInteractor__self[PlannerInteractor]
    T33_usecase_usecase_PlannerInteractor_new([new])
  end
  subgraph T32_usecase_usecase_PlannerPortError["planner::PlannerPortError"]
    direction TB
    T32_usecase_usecase_PlannerPortError__self[PlannerPortError]
    T32_usecase_usecase_PlannerPortError_MissingPromptSource[MissingPromptSource]
    T32_usecase_usecase_PlannerPortError_PlannerUnavailable[PlannerUnavailable]
    T32_usecase_usecase_PlannerPortError_PlannerTimeout[PlannerTimeout]
    T32_usecase_usecase_PlannerPortError_PlannerFailed[PlannerFailed]
  end
  subgraph R27_usecase_usecase_PlannerPort["planner::PlannerPort"]
    direction TB
    R27_usecase_usecase_PlannerPort__self[PlannerPort]
    R27_usecase_usecase_PlannerPort_run([run])
  end
  subgraph R30_usecase_usecase_PlannerService["planner::PlannerService"]
    direction TB
    R30_usecase_usecase_PlannerService__self[PlannerService]
    R30_usecase_usecase_PlannerService_run_codex_local([run_codex_local])
  end
  end
  subgraph usecase_usecase_module_verify["usecase::verify"]
    direction TB
  subgraph T32_usecase_usecase_VerifyInteractor["verify::VerifyInteractor"]
    direction TB
    T32_usecase_usecase_VerifyInteractor__self[VerifyInteractor]
    T32_usecase_usecase_VerifyInteractor_new([new])
  end
  subgraph R29_usecase_usecase_VerifyService["verify::VerifyService"]
    direction TB
    R29_usecase_usecase_VerifyService__self[VerifyService]
    R29_usecase_usecase_VerifyService_verify_tech_stack([verify_tech_stack])
    R29_usecase_usecase_VerifyService_verify_latest_track([verify_latest_track])
    R29_usecase_usecase_VerifyService_verify_arch_docs([verify_arch_docs])
    R29_usecase_usecase_VerifyService_verify_layers([verify_layers])
    R29_usecase_usecase_VerifyService_verify_hooks_path([verify_hooks_path])
    R29_usecase_usecase_VerifyService_verify_spec_attribution([verify_spec_attribution])
    R29_usecase_usecase_VerifyService_verify_spec_frontmatter([verify_spec_frontmatter])
    R29_usecase_usecase_VerifyService_verify_canonical_modules([verify_canonical_modules])
    R29_usecase_usecase_VerifyService_verify_module_size([verify_module_size])
    R29_usecase_usecase_VerifyService_verify_domain_purity([verify_domain_purity])
    R29_usecase_usecase_VerifyService_verify_domain_strings([verify_domain_strings])
    R29_usecase_usecase_VerifyService_verify_usecase_purity([verify_usecase_purity])
    R29_usecase_usecase_VerifyService_verify_doc_links([verify_doc_links])
    R29_usecase_usecase_VerifyService_verify_view_freshness([verify_view_freshness])
    R29_usecase_usecase_VerifyService_verify_spec_signals([verify_spec_signals])
    R29_usecase_usecase_VerifyService_verify_plan_artifact_refs([verify_plan_artifact_refs])
    R29_usecase_usecase_VerifyService_verify_catalogue_spec_refs([verify_catalogue_spec_refs])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_codex_planner["infrastructure::codex_planner"]
    direction TB
  subgraph T49_infrastructure_infrastructure_CodexPlannerAdapter["codex_planner::CodexPlannerAdapter"]
    direction TB
    T49_infrastructure_infrastructure_CodexPlannerAdapter__self[CodexPlannerAdapter]
    T49_infrastructure_infrastructure_CodexPlannerAdapter_new([new])
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_arch["cli_driver::arch"]
    direction TB
  subgraph T32_cli_driver_cli_driver_ArchDriver["arch::ArchDriver"]
    direction TB
    T32_cli_driver_cli_driver_ArchDriver__self[ArchDriver]
    T32_cli_driver_cli_driver_ArchDriver_handle([handle])
    T32_cli_driver_cli_driver_ArchDriver_new([new])
  end
  subgraph T31_cli_driver_cli_driver_ArchInput["arch::ArchInput"]
    direction TB
    T31_cli_driver_cli_driver_ArchInput__self[ArchInput]
    T31_cli_driver_cli_driver_ArchInput_Tree[Tree]
    T31_cli_driver_cli_driver_ArchInput_TreeFull[TreeFull]
    T31_cli_driver_cli_driver_ArchInput_Members[Members]
    T31_cli_driver_cli_driver_ArchInput_DirectChecks[DirectChecks]
  end
  end
  subgraph cli_driver_cli_driver_module_conventions["cli_driver::conventions"]
    direction TB
  subgraph T39_cli_driver_cli_driver_ConventionsDriver["conventions::ConventionsDriver"]
    direction TB
    T39_cli_driver_cli_driver_ConventionsDriver__self[ConventionsDriver]
    T39_cli_driver_cli_driver_ConventionsDriver_handle([handle])
    T39_cli_driver_cli_driver_ConventionsDriver_new([new])
  end
  subgraph T38_cli_driver_cli_driver_ConventionsInput["conventions::ConventionsInput"]
    direction TB
    T38_cli_driver_cli_driver_ConventionsInput__self[ConventionsInput]
    T38_cli_driver_cli_driver_ConventionsInput_Add[Add]
    T38_cli_driver_cli_driver_ConventionsInput_UpdateIndex[UpdateIndex]
    T38_cli_driver_cli_driver_ConventionsInput_VerifyIndex[VerifyIndex]
  end
  end
  subgraph cli_driver_cli_driver_module_demo["cli_driver::demo"]
    direction TB
  subgraph T32_cli_driver_cli_driver_DemoDriver["demo::DemoDriver"]
    direction TB
    T32_cli_driver_cli_driver_DemoDriver__self[DemoDriver]
    T32_cli_driver_cli_driver_DemoDriver_handle([handle])
  end
  subgraph T31_cli_driver_cli_driver_DemoInput["demo::DemoInput"]
    direction TB
    T31_cli_driver_cli_driver_DemoInput__self[DemoInput]
    T31_cli_driver_cli_driver_DemoInput_Run[Run]
  end
  end
  subgraph cli_driver_cli_driver_module_domain["cli_driver::domain"]
    direction TB
  subgraph T34_cli_driver_cli_driver_DomainDriver["domain::DomainDriver"]
    direction TB
    T34_cli_driver_cli_driver_DomainDriver__self[DomainDriver]
    T34_cli_driver_cli_driver_DomainDriver_handle([handle])
  end
  subgraph T33_cli_driver_cli_driver_DomainInput["domain::DomainInput"]
    direction TB
    T33_cli_driver_cli_driver_DomainInput__self[DomainInput]
    T33_cli_driver_cli_driver_DomainInput_ExportSchema[ExportSchema]
  end
  subgraph T39_cli_driver_cli_driver_ExportSchemaInput["domain::ExportSchemaInput"]
    direction TB
    T39_cli_driver_cli_driver_ExportSchemaInput__self[ExportSchemaInput]
  end
  end
  subgraph cli_driver_cli_driver_module_dry["cli_driver::dry"]
    direction TB
  subgraph T31_cli_driver_cli_driver_DryDriver["dry::DryDriver"]
    direction TB
    T31_cli_driver_cli_driver_DryDriver__self[DryDriver]
    T31_cli_driver_cli_driver_DryDriver_handle([handle])
  end
  subgraph T30_cli_driver_cli_driver_DryInput["dry::DryInput"]
    direction TB
    T30_cli_driver_cli_driver_DryInput__self[DryInput]
    T30_cli_driver_cli_driver_DryInput_Write[Write]
    T30_cli_driver_cli_driver_DryInput_Results[Results]
    T30_cli_driver_cli_driver_DryInput_CheckApproved[CheckApproved]
    T30_cli_driver_cli_driver_DryInput_FixLocal[FixLocal]
  end
  end
  subgraph cli_driver_cli_driver_module_file["cli_driver::file"]
    direction TB
  subgraph T32_cli_driver_cli_driver_FileDriver["file::FileDriver"]
    direction TB
    T32_cli_driver_cli_driver_FileDriver__self[FileDriver]
    T32_cli_driver_cli_driver_FileDriver_handle([handle])
    T32_cli_driver_cli_driver_FileDriver_new([new])
  end
  subgraph T31_cli_driver_cli_driver_FileInput["file::FileInput"]
    direction TB
    T31_cli_driver_cli_driver_FileInput__self[FileInput]
    T31_cli_driver_cli_driver_FileInput_WriteAtomic[WriteAtomic]
  end
  end
  subgraph cli_driver_cli_driver_module_git["cli_driver::git"]
    direction TB
  subgraph T31_cli_driver_cli_driver_GitDriver["git::GitDriver"]
    direction TB
    T31_cli_driver_cli_driver_GitDriver__self[GitDriver]
    T31_cli_driver_cli_driver_GitDriver_handle([handle])
  end
  subgraph T30_cli_driver_cli_driver_GitInput["git::GitInput"]
    direction TB
    T30_cli_driver_cli_driver_GitInput__self[GitInput]
    T30_cli_driver_cli_driver_GitInput_AddAll[AddAll]
    T30_cli_driver_cli_driver_GitInput_AddFromFile[AddFromFile]
    T30_cli_driver_cli_driver_GitInput_CommitFromFile[CommitFromFile]
    T30_cli_driver_cli_driver_GitInput_NoteFromFile[NoteFromFile]
    T30_cli_driver_cli_driver_GitInput_SwitchAndPull[SwitchAndPull]
    T30_cli_driver_cli_driver_GitInput_Unstage[Unstage]
    T30_cli_driver_cli_driver_GitInput_CurrentBranchTrackIdStrict[CurrentBranchTrackIdStrict]
  end
  end
  subgraph cli_driver_cli_driver_module_guard["cli_driver::guard"]
    direction TB
  subgraph T33_cli_driver_cli_driver_GuardDriver["guard::GuardDriver"]
    direction TB
    T33_cli_driver_cli_driver_GuardDriver__self[GuardDriver]
    T33_cli_driver_cli_driver_GuardDriver_handle([handle])
  end
  subgraph T32_cli_driver_cli_driver_GuardInput["guard::GuardInput"]
    direction TB
    T32_cli_driver_cli_driver_GuardInput__self[GuardInput]
    T32_cli_driver_cli_driver_GuardInput_Check[Check]
  end
  end
  subgraph cli_driver_cli_driver_module_hook["cli_driver::hook"]
    direction TB
  subgraph T32_cli_driver_cli_driver_HookDriver["hook::HookDriver"]
    direction TB
    T32_cli_driver_cli_driver_HookDriver__self[HookDriver]
    T32_cli_driver_cli_driver_HookDriver_handle([handle])
  end
  subgraph T31_cli_driver_cli_driver_HookInput["hook::HookInput"]
    direction TB
    T31_cli_driver_cli_driver_HookInput__self[HookInput]
    T31_cli_driver_cli_driver_HookInput_Dispatch[Dispatch]
  end
  subgraph T30_cli_driver_cli_driver_HookName["hook::HookName"]
    direction TB
    T30_cli_driver_cli_driver_HookName__self[HookName]
    T30_cli_driver_cli_driver_HookName_HooksPathSetup[HooksPathSetup]
    T30_cli_driver_cli_driver_HookName_BlockDirectGitOps[BlockDirectGitOps]
    T30_cli_driver_cli_driver_HookName_BlockTestFileDeletion[BlockTestFileDeletion]
    T30_cli_driver_cli_driver_HookName_GitRefUpdate[GitRefUpdate]
    T30_cli_driver_cli_driver_HookName_GitPrePush[GitPrePush]
    T30_cli_driver_cli_driver_HookName_SkillCompliance[SkillCompliance]
  end
  end
  subgraph cli_driver_cli_driver_module_plan["cli_driver::plan"]
    direction TB
  subgraph T32_cli_driver_cli_driver_PlanDriver["plan::PlanDriver"]
    direction TB
    T32_cli_driver_cli_driver_PlanDriver__self[PlanDriver]
    T32_cli_driver_cli_driver_PlanDriver_handle([handle])
    T32_cli_driver_cli_driver_PlanDriver_new([new])
  end
  subgraph T31_cli_driver_cli_driver_PlanInput["plan::PlanInput"]
    direction TB
    T31_cli_driver_cli_driver_PlanInput__self[PlanInput]
    T31_cli_driver_cli_driver_PlanInput_RunCodexLocal[RunCodexLocal]
  end
  end
  subgraph cli_driver_cli_driver_module_pr["cli_driver::pr"]
    direction TB
  subgraph T30_cli_driver_cli_driver_PrDriver["pr::PrDriver"]
    direction TB
    T30_cli_driver_cli_driver_PrDriver__self[PrDriver]
    T30_cli_driver_cli_driver_PrDriver_handle([handle])
  end
  subgraph T29_cli_driver_cli_driver_PrInput["pr::PrInput"]
    direction TB
    T29_cli_driver_cli_driver_PrInput__self[PrInput]
    T29_cli_driver_cli_driver_PrInput_Push[Push]
    T29_cli_driver_cli_driver_PrInput_Ensure[Ensure]
    T29_cli_driver_cli_driver_PrInput_Status[Status]
    T29_cli_driver_cli_driver_PrInput_WaitAndMerge[WaitAndMerge]
    T29_cli_driver_cli_driver_PrInput_TriggerReview[TriggerReview]
    T29_cli_driver_cli_driver_PrInput_PollReview[PollReview]
    T29_cli_driver_cli_driver_PrInput_ReviewCycle[ReviewCycle]
  end
  end
  subgraph cli_driver_cli_driver_module_ref_verify["cli_driver::ref_verify"]
    direction TB
  subgraph T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput["ref_verify::RefVerifyCheckApprovedInput"]
    direction TB
    T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput__self[RefVerifyCheckApprovedInput]
  end
  subgraph T37_cli_driver_cli_driver_RefVerifyDriver["ref_verify::RefVerifyDriver"]
    direction TB
    T37_cli_driver_cli_driver_RefVerifyDriver__self[RefVerifyDriver]
    T37_cli_driver_cli_driver_RefVerifyDriver_handle([handle])
  end
  subgraph T36_cli_driver_cli_driver_RefVerifyInput["ref_verify::RefVerifyInput"]
    direction TB
    T36_cli_driver_cli_driver_RefVerifyInput__self[RefVerifyInput]
    T36_cli_driver_cli_driver_RefVerifyInput_Run[Run]
    T36_cli_driver_cli_driver_RefVerifyInput_CheckApproved[CheckApproved]
  end
  subgraph T39_cli_driver_cli_driver_RefVerifyRunInput["ref_verify::RefVerifyRunInput"]
    direction TB
    T39_cli_driver_cli_driver_RefVerifyRunInput__self[RefVerifyRunInput]
  end
  F64_cli_driver_cli_driver_cli_driver__ref_verify__format_pair_status[[format_pair_status]]
  end
  subgraph cli_driver_cli_driver_module_render["cli_driver::render"]
    direction TB
  subgraph T36_cli_driver_cli_driver_CommandOutcome["render::CommandOutcome"]
    direction TB
    T36_cli_driver_cli_driver_CommandOutcome__self[CommandOutcome]
  end
  end
  subgraph cli_driver_cli_driver_module_review["cli_driver::review"]
    direction TB
  subgraph T34_cli_driver_cli_driver_ReviewDriver["review::ReviewDriver"]
    direction TB
    T34_cli_driver_cli_driver_ReviewDriver__self[ReviewDriver]
    T34_cli_driver_cli_driver_ReviewDriver_handle([handle])
  end
  subgraph T33_cli_driver_cli_driver_ReviewInput["review::ReviewInput"]
    direction TB
    T33_cli_driver_cli_driver_ReviewInput__self[ReviewInput]
    T33_cli_driver_cli_driver_ReviewInput_RunCodex[RunCodex]
    T33_cli_driver_cli_driver_ReviewInput_RunClaude[RunClaude]
    T33_cli_driver_cli_driver_ReviewInput_RunLocal[RunLocal]
    T33_cli_driver_cli_driver_ReviewInput_RunFixLocal[RunFixLocal]
    T33_cli_driver_cli_driver_ReviewInput_CheckApproved[CheckApproved]
    T33_cli_driver_cli_driver_ReviewInput_Results[Results]
    T33_cli_driver_cli_driver_ReviewInput_Classify[Classify]
    T33_cli_driver_cli_driver_ReviewInput_Files[Files]
    T33_cli_driver_cli_driver_ReviewInput_ValidateScope[ValidateScope]
    T33_cli_driver_cli_driver_ReviewInput_GetBriefing[GetBriefing]
    T33_cli_driver_cli_driver_ReviewInput_PersistCommitHash[PersistCommitHash]
  end
  end
  subgraph cli_driver_cli_driver_module_semantic_dup["cli_driver::semantic_dup"]
    direction TB
  subgraph T39_cli_driver_cli_driver_SemanticDupDriver["semantic_dup::SemanticDupDriver"]
    direction TB
    T39_cli_driver_cli_driver_SemanticDupDriver__self[SemanticDupDriver]
    T39_cli_driver_cli_driver_SemanticDupDriver_handle([handle])
  end
  subgraph T38_cli_driver_cli_driver_SemanticDupInput["semantic_dup::SemanticDupInput"]
    direction TB
    T38_cli_driver_cli_driver_SemanticDupInput__self[SemanticDupInput]
    T38_cli_driver_cli_driver_SemanticDupInput_FindSimilar[FindSimilar]
    T38_cli_driver_cli_driver_SemanticDupInput_IndexBuild[IndexBuild]
    T38_cli_driver_cli_driver_SemanticDupInput_IndexMeasureQuality[IndexMeasureQuality]
    T38_cli_driver_cli_driver_SemanticDupInput_DupCheck[DupCheck]
  end
  end
  subgraph cli_driver_cli_driver_module_signal["cli_driver::signal"]
    direction TB
  subgraph T34_cli_driver_cli_driver_SignalDriver["signal::SignalDriver"]
    direction TB
    T34_cli_driver_cli_driver_SignalDriver__self[SignalDriver]
    T34_cli_driver_cli_driver_SignalDriver_handle([handle])
  end
  subgraph T36_cli_driver_cli_driver_SignalGateName["signal::SignalGateName"]
    direction TB
    T36_cli_driver_cli_driver_SignalGateName__self[SignalGateName]
    T36_cli_driver_cli_driver_SignalGateName_Commit[Commit]
    T36_cli_driver_cli_driver_SignalGateName_Merge[Merge]
  end
  subgraph T33_cli_driver_cli_driver_SignalInput["signal::SignalInput"]
    direction TB
    T33_cli_driver_cli_driver_SignalInput__self[SignalInput]
    T33_cli_driver_cli_driver_SignalInput_CalcAdrUser[CalcAdrUser]
    T33_cli_driver_cli_driver_SignalInput_CheckAdrUser[CheckAdrUser]
    T33_cli_driver_cli_driver_SignalInput_CalcSpecAdr[CalcSpecAdr]
    T33_cli_driver_cli_driver_SignalInput_CheckSpecAdr[CheckSpecAdr]
    T33_cli_driver_cli_driver_SignalInput_CalcCatalogSpec[CalcCatalogSpec]
    T33_cli_driver_cli_driver_SignalInput_CheckCatalogSpec[CheckCatalogSpec]
    T33_cli_driver_cli_driver_SignalInput_CalcImplCatalog[CalcImplCatalog]
    T33_cli_driver_cli_driver_SignalInput_CheckImplCatalog[CheckImplCatalog]
    T33_cli_driver_cli_driver_SignalInput_CheckGate[CheckGate]
  end
  end
  subgraph cli_driver_cli_driver_module_telemetry["cli_driver::telemetry"]
    direction TB
  subgraph T37_cli_driver_cli_driver_TelemetryDriver["telemetry::TelemetryDriver"]
    direction TB
    T37_cli_driver_cli_driver_TelemetryDriver__self[TelemetryDriver]
    T37_cli_driver_cli_driver_TelemetryDriver_handle([handle])
  end
  subgraph T36_cli_driver_cli_driver_TelemetryInput["telemetry::TelemetryInput"]
    direction TB
    T36_cli_driver_cli_driver_TelemetryInput__self[TelemetryInput]
    T36_cli_driver_cli_driver_TelemetryInput_Report[Report]
    T36_cli_driver_cli_driver_TelemetryInput_EmitArchivedTrackSubcommand[EmitArchivedTrackSubcommand]
  end
  subgraph T42_cli_driver_cli_driver_TelemetryReportInput["telemetry::TelemetryReportInput"]
    direction TB
    T42_cli_driver_cli_driver_TelemetryReportInput__self[TelemetryReportInput]
  end
  end
  subgraph cli_driver_cli_driver_module_track["cli_driver::track"]
    direction TB
  subgraph T33_cli_driver_cli_driver_TrackDriver["track::TrackDriver"]
    direction TB
    T33_cli_driver_cli_driver_TrackDriver__self[TrackDriver]
    T33_cli_driver_cli_driver_TrackDriver_handle([handle])
  end
  subgraph T32_cli_driver_cli_driver_TrackInput["track::TrackInput"]
    direction TB
    T32_cli_driver_cli_driver_TrackInput__self[TrackInput]
    T32_cli_driver_cli_driver_TrackInput_Init[Init]
    T32_cli_driver_cli_driver_TrackInput_Transition[Transition]
    T32_cli_driver_cli_driver_TrackInput_Resolve[Resolve]
    T32_cli_driver_cli_driver_TrackInput_BranchCreate[BranchCreate]
    T32_cli_driver_cli_driver_TrackInput_BranchSwitch[BranchSwitch]
    T32_cli_driver_cli_driver_TrackInput_ViewsValidate[ViewsValidate]
    T32_cli_driver_cli_driver_TrackInput_ViewsSync[ViewsSync]
    T32_cli_driver_cli_driver_TrackInput_AddTask[AddTask]
    T32_cli_driver_cli_driver_TrackInput_SetOverride[SetOverride]
    T32_cli_driver_cli_driver_TrackInput_ClearOverride[ClearOverride]
    T32_cli_driver_cli_driver_TrackInput_NextTask[NextTask]
    T32_cli_driver_cli_driver_TrackInput_TaskCounts[TaskCounts]
    T32_cli_driver_cli_driver_TrackInput_Archive[Archive]
    T32_cli_driver_cli_driver_TrackInput_DetectActive[DetectActive]
  end
  end
  subgraph cli_driver_cli_driver_module_verify["cli_driver::verify"]
    direction TB
  subgraph T34_cli_driver_cli_driver_VerifyDriver["verify::VerifyDriver"]
    direction TB
    T34_cli_driver_cli_driver_VerifyDriver__self[VerifyDriver]
    T34_cli_driver_cli_driver_VerifyDriver_handle([handle])
    T34_cli_driver_cli_driver_VerifyDriver_new([new])
  end
  subgraph T33_cli_driver_cli_driver_VerifyInput["verify::VerifyInput"]
    direction TB
    T33_cli_driver_cli_driver_VerifyInput__self[VerifyInput]
    T33_cli_driver_cli_driver_VerifyInput_TechStack[TechStack]
    T33_cli_driver_cli_driver_VerifyInput_LatestTrack[LatestTrack]
    T33_cli_driver_cli_driver_VerifyInput_ArchDocs[ArchDocs]
    T33_cli_driver_cli_driver_VerifyInput_Layers[Layers]
    T33_cli_driver_cli_driver_VerifyInput_HooksPath[HooksPath]
    T33_cli_driver_cli_driver_VerifyInput_SpecAttribution[SpecAttribution]
    T33_cli_driver_cli_driver_VerifyInput_SpecFrontmatter[SpecFrontmatter]
    T33_cli_driver_cli_driver_VerifyInput_CanonicalModules[CanonicalModules]
    T33_cli_driver_cli_driver_VerifyInput_ModuleSize[ModuleSize]
    T33_cli_driver_cli_driver_VerifyInput_DomainPurity[DomainPurity]
    T33_cli_driver_cli_driver_VerifyInput_DomainStrings[DomainStrings]
    T33_cli_driver_cli_driver_VerifyInput_UsecasePurity[UsecasePurity]
    T33_cli_driver_cli_driver_VerifyInput_DocLinks[DocLinks]
    T33_cli_driver_cli_driver_VerifyInput_ViewFreshness[ViewFreshness]
    T33_cli_driver_cli_driver_VerifyInput_SpecSignals[SpecSignals]
    T33_cli_driver_cli_driver_VerifyInput_PlanArtifactRefs[PlanArtifactRefs]
    T33_cli_driver_cli_driver_VerifyInput_CatalogueSpecRefs[CatalogueSpecRefs]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_arch["cli_composition::arch"]
    direction TB
  subgraph T51_cli_composition_cli_composition_ArchCompositionRoot["arch::ArchCompositionRoot"]
    direction TB
    T51_cli_composition_cli_composition_ArchCompositionRoot__self[ArchCompositionRoot]
    T51_cli_composition_cli_composition_ArchCompositionRoot_new([new])
    T51_cli_composition_cli_composition_ArchCompositionRoot_arch_driver([arch_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_conventions["cli_composition::conventions"]
    direction TB
  subgraph T58_cli_composition_cli_composition_ConventionsCompositionRoot["conventions::ConventionsCompositionRoot"]
    direction TB
    T58_cli_composition_cli_composition_ConventionsCompositionRoot__self[ConventionsCompositionRoot]
    T58_cli_composition_cli_composition_ConventionsCompositionRoot_new([new])
    T58_cli_composition_cli_composition_ConventionsCompositionRoot_conventions_driver([conventions_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_demo["cli_composition::demo"]
    direction TB
  subgraph T51_cli_composition_cli_composition_DemoCompositionRoot["demo::DemoCompositionRoot"]
    direction TB
    T51_cli_composition_cli_composition_DemoCompositionRoot__self[DemoCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_domain["cli_composition::domain"]
    direction TB
  subgraph T53_cli_composition_cli_composition_DomainCompositionRoot["domain::DomainCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_DomainCompositionRoot__self[DomainCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_dry["cli_composition::dry"]
    direction TB
  subgraph T50_cli_composition_cli_composition_DryCompositionRoot["dry::DryCompositionRoot"]
    direction TB
    T50_cli_composition_cli_composition_DryCompositionRoot__self[DryCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_dry_fix_runner["cli_composition::dry_fix_runner"]
    direction TB
  subgraph T59_cli_composition_cli_composition_DryFixRunnerCompositionRoot["dry_fix_runner::DryFixRunnerCompositionRoot"]
    direction TB
    T59_cli_composition_cli_composition_DryFixRunnerCompositionRoot__self[DryFixRunnerCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_error["cli_composition::error"]
    direction TB
  subgraph T48_cli_composition_cli_composition_CompositionError["error::CompositionError"]
    direction TB
    T48_cli_composition_cli_composition_CompositionError__self[CompositionError]
    T48_cli_composition_cli_composition_CompositionError_ConfigLoad[ConfigLoad]
    T48_cli_composition_cli_composition_CompositionError_AdapterInit[AdapterInit]
    T48_cli_composition_cli_composition_CompositionError_WiringFailed[WiringFailed]
    T48_cli_composition_cli_composition_CompositionError_Usecase[Usecase]
    T48_cli_composition_cli_composition_CompositionError_Infrastructure[Infrastructure]
  end
  end
  subgraph cli_composition_cli_composition_module_file["cli_composition::file"]
    direction TB
  subgraph T51_cli_composition_cli_composition_FileCompositionRoot["file::FileCompositionRoot"]
    direction TB
    T51_cli_composition_cli_composition_FileCompositionRoot__self[FileCompositionRoot]
    T51_cli_composition_cli_composition_FileCompositionRoot_new([new])
    T51_cli_composition_cli_composition_FileCompositionRoot_file_driver([file_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_git["cli_composition::git"]
    direction TB
  subgraph T50_cli_composition_cli_composition_GitCompositionRoot["git::GitCompositionRoot"]
    direction TB
    T50_cli_composition_cli_composition_GitCompositionRoot__self[GitCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_guard["cli_composition::guard"]
    direction TB
  subgraph T52_cli_composition_cli_composition_GuardCompositionRoot["guard::GuardCompositionRoot"]
    direction TB
    T52_cli_composition_cli_composition_GuardCompositionRoot__self[GuardCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_hook["cli_composition::hook"]
    direction TB
  subgraph T51_cli_composition_cli_composition_HookCompositionRoot["hook::HookCompositionRoot"]
    direction TB
    T51_cli_composition_cli_composition_HookCompositionRoot__self[HookCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_plan["cli_composition::plan"]
    direction TB
  subgraph T51_cli_composition_cli_composition_PlanCompositionRoot["plan::PlanCompositionRoot"]
    direction TB
    T51_cli_composition_cli_composition_PlanCompositionRoot__self[PlanCompositionRoot]
    T51_cli_composition_cli_composition_PlanCompositionRoot_new([new])
    T51_cli_composition_cli_composition_PlanCompositionRoot_plan_driver([plan_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_pr["cli_composition::pr"]
    direction TB
  subgraph T49_cli_composition_cli_composition_PrCompositionRoot["pr::PrCompositionRoot"]
    direction TB
    T49_cli_composition_cli_composition_PrCompositionRoot__self[PrCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_ref_verify["cli_composition::ref_verify"]
    direction TB
  subgraph T56_cli_composition_cli_composition_RefVerifyCompositionRoot["ref_verify::RefVerifyCompositionRoot"]
    direction TB
    T56_cli_composition_cli_composition_RefVerifyCompositionRoot__self[RefVerifyCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_review_v2["cli_composition::review_v2"]
    direction TB
  subgraph T53_cli_composition_cli_composition_ReviewCompositionRoot["review_v2::ReviewCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_ReviewCompositionRoot__self[ReviewCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_semantic_dup["cli_composition::semantic_dup"]
    direction TB
  subgraph T58_cli_composition_cli_composition_SemanticDupCompositionRoot["semantic_dup::SemanticDupCompositionRoot"]
    direction TB
    T58_cli_composition_cli_composition_SemanticDupCompositionRoot__self[SemanticDupCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_signal["cli_composition::signal"]
    direction TB
  subgraph T53_cli_composition_cli_composition_SignalCompositionRoot["signal::SignalCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_SignalCompositionRoot__self[SignalCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_telemetry["cli_composition::telemetry"]
    direction TB
  subgraph T56_cli_composition_cli_composition_TelemetryCompositionRoot["telemetry::TelemetryCompositionRoot"]
    direction TB
    T56_cli_composition_cli_composition_TelemetryCompositionRoot__self[TelemetryCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_track["cli_composition::track"]
    direction TB
  subgraph T52_cli_composition_cli_composition_TrackCompositionRoot["track::composition_root::TrackCompositionRoot"]
    direction TB
    T52_cli_composition_cli_composition_TrackCompositionRoot__self[TrackCompositionRoot]
  end
  end
  subgraph cli_composition_cli_composition_module_verify["cli_composition::verify"]
    direction TB
  subgraph T53_cli_composition_cli_composition_VerifyCompositionRoot["verify::VerifyCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_VerifyCompositionRoot__self[VerifyCompositionRoot]
    T53_cli_composition_cli_composition_VerifyCompositionRoot_new([new])
    T53_cli_composition_cli_composition_VerifyCompositionRoot_verify_driver([verify_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T19_cli_cli_ArchCommand["commands::arch::ArchCommand"]
    direction TB
    T19_cli_cli_ArchCommand__self[ArchCommand]
    T19_cli_cli_ArchCommand_Tree[Tree]
    T19_cli_cli_ArchCommand_TreeFull[TreeFull]
    T19_cli_cli_ArchCommand_Members[Members]
    T19_cli_cli_ArchCommand_DirectChecks[DirectChecks]
  end
  subgraph T20_cli_cli_BranchAction["commands::track::BranchAction"]
    direction TB
    T20_cli_cli_BranchAction__self[BranchAction]
    T20_cli_cli_BranchAction_Create[Create]
    T20_cli_cli_BranchAction_Switch[Switch]
  end
  subgraph T18_cli_cli_BranchArgs["commands::track::BranchArgs"]
    direction TB
    T18_cli_cli_BranchArgs__self[BranchArgs]
  end
  subgraph T23_cli_cli_CalcAdrUserArgs["commands::signal::CalcAdrUserArgs"]
    direction TB
    T23_cli_cli_CalcAdrUserArgs__self[CalcAdrUserArgs]
  end
  subgraph T27_cli_cli_CalcCatalogSpecArgs["commands::signal::CalcCatalogSpecArgs"]
    direction TB
    T27_cli_cli_CalcCatalogSpecArgs__self[CalcCatalogSpecArgs]
  end
  subgraph T27_cli_cli_CalcImplCatalogArgs["commands::signal::CalcImplCatalogArgs"]
    direction TB
    T27_cli_cli_CalcImplCatalogArgs__self[CalcImplCatalogArgs]
  end
  subgraph T23_cli_cli_CalcSpecAdrArgs["commands::signal::CalcSpecAdrArgs"]
    direction TB
    T23_cli_cli_CalcSpecAdrArgs__self[CalcSpecAdrArgs]
  end
  subgraph T29_cli_cli_CatalogueSpecRefsArgs["commands::verify::CatalogueSpecRefsArgs"]
    direction TB
    T29_cli_cli_CatalogueSpecRefsArgs__self[CatalogueSpecRefsArgs]
  end
  subgraph T24_cli_cli_CheckAdrUserArgs["commands::signal::CheckAdrUserArgs"]
    direction TB
    T24_cli_cli_CheckAdrUserArgs__self[CheckAdrUserArgs]
  end
  subgraph T25_cli_cli_CheckApprovedArgs["commands::ref_verify::CheckApprovedArgs"]
    direction TB
    T25_cli_cli_CheckApprovedArgs__self[CheckApprovedArgs]
  end
  subgraph T28_cli_cli_CheckCatalogSpecArgs["commands::signal::CheckCatalogSpecArgs"]
    direction TB
    T28_cli_cli_CheckCatalogSpecArgs__self[CheckCatalogSpecArgs]
  end
  subgraph T18_cli_cli_CheckFlags["commands::signal::CheckFlags"]
    direction TB
    T18_cli_cli_CheckFlags__self[CheckFlags]
  end
  subgraph T28_cli_cli_CheckImplCatalogArgs["commands::signal::CheckImplCatalogArgs"]
    direction TB
    T28_cli_cli_CheckImplCatalogArgs__self[CheckImplCatalogArgs]
  end
  subgraph T24_cli_cli_CheckSpecAdrArgs["commands::signal::CheckSpecAdrArgs"]
    direction TB
    T24_cli_cli_CheckSpecAdrArgs__self[CheckSpecAdrArgs]
  end
  subgraph T23_cli_cli_ClaudeLocalArgs["commands::review::ClaudeLocalArgs"]
    direction TB
    T23_cli_cli_ClaudeLocalArgs__self[ClaudeLocalArgs]
  end
  subgraph T19_cli_cli_CliHookName["commands::hook::CliHookName"]
    direction TB
    T19_cli_cli_CliHookName__self[CliHookName]
    T19_cli_cli_CliHookName_HooksPathSetup[HooksPathSetup]
    T19_cli_cli_CliHookName_BlockDirectGitOps[BlockDirectGitOps]
    T19_cli_cli_CliHookName_BlockTestFileDeletion[BlockTestFileDeletion]
    T19_cli_cli_CliHookName_GitRefUpdate[GitRefUpdate]
    T19_cli_cli_CliHookName_GitPrePush[GitPrePush]
    T19_cli_cli_CliHookName_SkillCompliance[SkillCompliance]
  end
  subgraph T22_cli_cli_CodexLocalArgs["commands::review::CodexLocalArgs"]
    direction TB
    T22_cli_cli_CodexLocalArgs__self[CodexLocalArgs]
  end
  subgraph T25_cli_cli_CodexRoundTypeArg["commands::review::CodexRoundTypeArg"]
    direction TB
    T25_cli_cli_CodexRoundTypeArg__self[CodexRoundTypeArg]
    T25_cli_cli_CodexRoundTypeArg_Fast[Fast]
    T25_cli_cli_CodexRoundTypeArg_Final[Final]
  end
  subgraph T26_cli_cli_CommitFromFileArgs["commands::git::CommitFromFileArgs"]
    direction TB
    T26_cli_cli_CommitFromFileArgs__self[CommitFromFileArgs]
  end
  subgraph T26_cli_cli_ConventionsCommand["commands::conventions::ConventionsCommand"]
    direction TB
    T26_cli_cli_ConventionsCommand__self[ConventionsCommand]
    T26_cli_cli_ConventionsCommand_Add[Add]
    T26_cli_cli_ConventionsCommand_UpdateIndex[UpdateIndex]
    T26_cli_cli_ConventionsCommand_VerifyIndex[VerifyIndex]
  end
  subgraph T21_cli_cli_DomainCommand["commands::domain::DomainCommand"]
    direction TB
    T21_cli_cli_DomainCommand__self[DomainCommand]
    T21_cli_cli_DomainCommand_ExportSchema[ExportSchema]
  end
  subgraph T28_cli_cli_DryCheckApprovedArgs["commands::dry::DryCheckApprovedArgs"]
    direction TB
    T28_cli_cli_DryCheckApprovedArgs__self[DryCheckApprovedArgs]
  end
  subgraph T18_cli_cli_DryCommand["commands::dry::DryCommand"]
    direction TB
    T18_cli_cli_DryCommand__self[DryCommand]
    T18_cli_cli_DryCommand_Write[Write]
    T18_cli_cli_DryCommand_Results[Results]
    T18_cli_cli_DryCommand_CheckApproved[CheckApproved]
    T18_cli_cli_DryCommand_FixLocal[FixLocal]
  end
  subgraph T23_cli_cli_DryFixLocalArgs["commands::dry::DryFixLocalArgs"]
    direction TB
    T23_cli_cli_DryFixLocalArgs__self[DryFixLocalArgs]
  end
  subgraph T22_cli_cli_DryResultsArgs["commands::dry::DryResultsArgs"]
    direction TB
    T22_cli_cli_DryResultsArgs__self[DryResultsArgs]
  end
  subgraph T20_cli_cli_DryWriteArgs["commands::dry::DryWriteArgs"]
    direction TB
    T20_cli_cli_DryWriteArgs__self[DryWriteArgs]
  end
  subgraph T20_cli_cli_DupCheckArgs["commands::semantic_dup::DupCheckArgs"]
    direction TB
    T20_cli_cli_DupCheckArgs__self[DupCheckArgs]
  end
  subgraph T25_cli_cli_DupIndexBuildArgs["commands::semantic_dup::DupIndexBuildArgs"]
    direction TB
    T25_cli_cli_DupIndexBuildArgs__self[DupIndexBuildArgs]
  end
  subgraph T23_cli_cli_DupIndexCommand["commands::semantic_dup::DupIndexCommand"]
    direction TB
    T23_cli_cli_DupIndexCommand__self[DupIndexCommand]
    T23_cli_cli_DupIndexCommand_Build[Build]
    T23_cli_cli_DupIndexCommand_MeasureQuality[MeasureQuality]
  end
  subgraph T34_cli_cli_DupIndexMeasureQualityArgs["commands::semantic_dup::DupIndexMeasureQualityArgs"]
    direction TB
    T34_cli_cli_DupIndexMeasureQualityArgs__self[DupIndexMeasureQualityArgs]
  end
  subgraph T20_cli_cli_EnsurePrArgs["commands::pr::EnsurePrArgs"]
    direction TB
    T20_cli_cli_EnsurePrArgs__self[EnsurePrArgs]
  end
  subgraph T24_cli_cli_ExportSchemaArgs["commands::domain::ExportSchemaArgs"]
    direction TB
    T24_cli_cli_ExportSchemaArgs__self[ExportSchemaArgs]
  end
  subgraph T16_cli_cli_FileArgs["commands::git::FileArgs"]
    direction TB
    T16_cli_cli_FileArgs__self[FileArgs]
  end
  subgraph T19_cli_cli_FileCommand["commands::file::FileCommand"]
    direction TB
    T19_cli_cli_FileCommand__self[FileCommand]
    T19_cli_cli_FileCommand_WriteAtomic[WriteAtomic]
  end
  subgraph T23_cli_cli_FindSimilarArgs["commands::semantic_dup::FindSimilarArgs"]
    direction TB
    T23_cli_cli_FindSimilarArgs__self[FindSimilarArgs]
  end
  subgraph T27_cli_cli_FixpointResolveArgs["commands::track::fixpoint_resolve::FixpointResolveArgs"]
    direction TB
    T27_cli_cli_FixpointResolveArgs__self[FixpointResolveArgs]
  end
  subgraph T15_cli_cli_GateArg["commands::signal::GateArg"]
    direction TB
    T15_cli_cli_GateArg__self[GateArg]
    T15_cli_cli_GateArg_Commit[Commit]
    T15_cli_cli_GateArg_Merge[Merge]
  end
  subgraph T18_cli_cli_GitCommand["commands::git::GitCommand"]
    direction TB
    T18_cli_cli_GitCommand__self[GitCommand]
    T18_cli_cli_GitCommand_AddAll[AddAll]
    T18_cli_cli_GitCommand_AddPaths[AddPaths]
    T18_cli_cli_GitCommand_CommitFromFile[CommitFromFile]
    T18_cli_cli_GitCommand_SwitchAndPull[SwitchAndPull]
    T18_cli_cli_GitCommand_Unstage[Unstage]
  end
  subgraph T20_cli_cli_GuardCommand["commands::guard::GuardCommand"]
    direction TB
    T20_cli_cli_GuardCommand__self[GuardCommand]
    T20_cli_cli_GuardCommand_Check[Check]
  end
  subgraph T19_cli_cli_HookCommand["commands::hook::HookCommand"]
    direction TB
    T19_cli_cli_HookCommand__self[HookCommand]
    T19_cli_cli_HookCommand_Dispatch[Dispatch]
  end
  subgraph T28_cli_cli_PlanArtifactRefsArgs["commands::verify::PlanArtifactRefsArgs"]
    direction TB
    T28_cli_cli_PlanArtifactRefsArgs__self[PlanArtifactRefsArgs]
  end
  subgraph T26_cli_cli_PlanCodexLocalArgs["commands::plan::PlanCodexLocalArgs"]
    direction TB
    T26_cli_cli_PlanCodexLocalArgs__self[PlanCodexLocalArgs]
  end
  subgraph T19_cli_cli_PlanCommand["commands::plan::PlanCommand"]
    direction TB
    T19_cli_cli_PlanCommand__self[PlanCommand]
    T19_cli_cli_PlanCommand_CodexLocal[CodexLocal]
  end
  subgraph T22_cli_cli_PollReviewArgs["commands::pr::PollReviewArgs"]
    direction TB
    T22_cli_cli_PollReviewArgs__self[PollReviewArgs]
  end
  subgraph T17_cli_cli_PrCommand["commands::pr::PrCommand"]
    direction TB
    T17_cli_cli_PrCommand__self[PrCommand]
    T17_cli_cli_PrCommand_Push[Push]
    T17_cli_cli_PrCommand_EnsurePr[EnsurePr]
    T17_cli_cli_PrCommand_Status[Status]
    T17_cli_cli_PrCommand_WaitAndMerge[WaitAndMerge]
    T17_cli_cli_PrCommand_TriggerReview[TriggerReview]
    T17_cli_cli_PrCommand_PollReview[PollReview]
    T17_cli_cli_PrCommand_ReviewCycle[ReviewCycle]
  end
  subgraph T16_cli_cli_PushArgs["commands::pr::PushArgs"]
    direction TB
    T16_cli_cli_PushArgs__self[PushArgs]
  end
  subgraph T24_cli_cli_RefVerifyCommand["commands::ref_verify::RefVerifyCommand"]
    direction TB
    T24_cli_cli_RefVerifyCommand__self[RefVerifyCommand]
    T24_cli_cli_RefVerifyCommand_Run[Run]
    T24_cli_cli_RefVerifyCommand_CheckApproved[CheckApproved]
  end
  subgraph T18_cli_cli_ReportArgs["commands::telemetry::ReportArgs"]
    direction TB
    T18_cli_cli_ReportArgs__self[ReportArgs]
  end
  subgraph T19_cli_cli_ResolveArgs["commands::track::ResolveArgs"]
    direction TB
    T19_cli_cli_ResolveArgs__self[ResolveArgs]
  end
  subgraph T19_cli_cli_ResultsArgs["commands::review::ResultsArgs"]
    direction TB
    T19_cli_cli_ResultsArgs__self[ResultsArgs]
  end
  subgraph T20_cli_cli_ResultsLimit["commands::review::ResultsLimit"]
    direction TB
    T20_cli_cli_ResultsLimit__self[ResultsLimit]
    T20_cli_cli_ResultsLimit_Zero[Zero]
    T20_cli_cli_ResultsLimit_Count[Count]
    T20_cli_cli_ResultsLimit_All[All]
  end
  subgraph T21_cli_cli_ReviewCommand["commands::review::ReviewCommand"]
    direction TB
    T21_cli_cli_ReviewCommand__self[ReviewCommand]
    T21_cli_cli_ReviewCommand_CodexLocal[CodexLocal]
    T21_cli_cli_ReviewCommand_ClaudeLocal[ClaudeLocal]
    T21_cli_cli_ReviewCommand_Local[Local]
    T21_cli_cli_ReviewCommand_FixLocal[FixLocal]
    T21_cli_cli_ReviewCommand_CheckApproved[CheckApproved]
    T21_cli_cli_ReviewCommand_Results[Results]
    T21_cli_cli_ReviewCommand_Classify[Classify]
    T21_cli_cli_ReviewCommand_Files[Files]
  end
  subgraph T23_cli_cli_ReviewCycleArgs["commands::pr::ReviewCycleArgs"]
    direction TB
    T23_cli_cli_ReviewCycleArgs__self[ReviewCycleArgs]
  end
  subgraph T23_cli_cli_RoundTypeFilter["commands::review::RoundTypeFilter"]
    direction TB
    T23_cli_cli_RoundTypeFilter__self[RoundTypeFilter]
    T23_cli_cli_RoundTypeFilter_Fast[Fast]
    T23_cli_cli_RoundTypeFilter_Final[Final]
    T23_cli_cli_RoundTypeFilter_Any[Any]
  end
  subgraph T15_cli_cli_RunArgs["commands::ref_verify::RunArgs"]
    direction TB
    T15_cli_cli_RunArgs__self[RunArgs]
  end
  subgraph T25_cli_cli_SetCommitHashArgs["commands::track::SetCommitHashArgs"]
    direction TB
    T25_cli_cli_SetCommitHashArgs__self[SetCommitHashArgs]
  end
  subgraph T23_cli_cli_SignalCheckArgs["commands::signal::SignalCheckArgs"]
    direction TB
    T23_cli_cli_SignalCheckArgs__self[SignalCheckArgs]
  end
  subgraph T21_cli_cli_SignalCommand["commands::signal::SignalCommand"]
    direction TB
    T21_cli_cli_SignalCommand__self[SignalCommand]
    T21_cli_cli_SignalCommand_CalcAdrUser[CalcAdrUser]
    T21_cli_cli_SignalCommand_CheckAdrUser[CheckAdrUser]
    T21_cli_cli_SignalCommand_CalcSpecAdr[CalcSpecAdr]
    T21_cli_cli_SignalCommand_CheckSpecAdr[CheckSpecAdr]
    T21_cli_cli_SignalCommand_CalcCatalogSpec[CalcCatalogSpec]
    T21_cli_cli_SignalCommand_CheckCatalogSpec[CheckCatalogSpec]
    T21_cli_cli_SignalCommand_CalcImplCatalog[CalcImplCatalog]
    T21_cli_cli_SignalCommand_CheckImplCatalog[CheckImplCatalog]
    T21_cli_cli_SignalCommand_Check[Check]
  end
  subgraph T22_cli_cli_SpecVerifyArgs["commands::verify::SpecVerifyArgs"]
    direction TB
    T22_cli_cli_SpecVerifyArgs__self[SpecVerifyArgs]
  end
  subgraph T18_cli_cli_StatusArgs["commands::pr::StatusArgs"]
    direction TB
    T18_cli_cli_StatusArgs__self[StatusArgs]
  end
  subgraph T25_cli_cli_SwitchAndPullArgs["commands::git::SwitchAndPullArgs"]
    direction TB
    T25_cli_cli_SwitchAndPullArgs__self[SwitchAndPullArgs]
  end
  subgraph T24_cli_cli_TelemetryCommand["commands::telemetry::TelemetryCommand"]
    direction TB
    T24_cli_cli_TelemetryCommand__self[TelemetryCommand]
    T24_cli_cli_TelemetryCommand_Report[Report]
  end
  subgraph T20_cli_cli_TrackCommand["commands::track::TrackCommand"]
    direction TB
    T20_cli_cli_TrackCommand__self[TrackCommand]
    T20_cli_cli_TrackCommand_Archive[Archive]
    T20_cli_cli_TrackCommand_Transition[Transition]
    T20_cli_cli_TrackCommand_Branch[Branch]
    T20_cli_cli_TrackCommand_Resolve[Resolve]
    T20_cli_cli_TrackCommand_Views[Views]
    T20_cli_cli_TrackCommand_AddTask[AddTask]
    T20_cli_cli_TrackCommand_SetOverride[SetOverride]
    T20_cli_cli_TrackCommand_ClearOverride[ClearOverride]
    T20_cli_cli_TrackCommand_NextTask[NextTask]
    T20_cli_cli_TrackCommand_TaskCounts[TaskCounts]
    T20_cli_cli_TrackCommand_TypeGraph[TypeGraph]
    T20_cli_cli_TrackCommand_BaselineGraph[BaselineGraph]
    T20_cli_cli_TrackCommand_ContractMap[ContractMap]
    T20_cli_cli_TrackCommand_SpecElementHash[SpecElementHash]
    T20_cli_cli_TrackCommand_BaselineCapture[BaselineCapture]
    T20_cli_cli_TrackCommand_FixpointResolve[FixpointResolve]
    T20_cli_cli_TrackCommand_SetCommitHash[SetCommitHash]
    T20_cli_cli_TrackCommand_Lint[Lint]
    T20_cli_cli_TrackCommand_CatalogueImplSignals[CatalogueImplSignals]
  end
  subgraph T25_cli_cli_TriggerReviewArgs["commands::pr::TriggerReviewArgs"]
    direction TB
    T25_cli_cli_TriggerReviewArgs__self[TriggerReviewArgs]
  end
  subgraph T19_cli_cli_UnstageArgs["commands::git::UnstageArgs"]
    direction TB
    T19_cli_cli_UnstageArgs__self[UnstageArgs]
  end
  subgraph T24_cli_cli_VerdictFilterArg["commands::dry::VerdictFilterArg"]
    direction TB
    T24_cli_cli_VerdictFilterArg__self[VerdictFilterArg]
    T24_cli_cli_VerdictFilterArg_All[All]
    T24_cli_cli_VerdictFilterArg_NotAViolation[NotAViolation]
    T24_cli_cli_VerdictFilterArg_Accepted[Accepted]
    T24_cli_cli_VerdictFilterArg_Violation[Violation]
  end
  subgraph T18_cli_cli_VerifyArgs["commands::verify::VerifyArgs"]
    direction TB
    T18_cli_cli_VerifyArgs__self[VerifyArgs]
  end
  subgraph T21_cli_cli_VerifyCommand["commands::verify::VerifyCommand"]
    direction TB
    T21_cli_cli_VerifyCommand__self[VerifyCommand]
    T21_cli_cli_VerifyCommand_TechStack[TechStack]
    T21_cli_cli_VerifyCommand_LatestTrack[LatestTrack]
    T21_cli_cli_VerifyCommand_ArchDocs[ArchDocs]
    T21_cli_cli_VerifyCommand_Layers[Layers]
    T21_cli_cli_VerifyCommand_HooksPath[HooksPath]
    T21_cli_cli_VerifyCommand_SpecAttribution[SpecAttribution]
    T21_cli_cli_VerifyCommand_SpecFrontmatter[SpecFrontmatter]
    T21_cli_cli_VerifyCommand_CanonicalModules[CanonicalModules]
    T21_cli_cli_VerifyCommand_ModuleSize[ModuleSize]
    T21_cli_cli_VerifyCommand_DomainPurity[DomainPurity]
    T21_cli_cli_VerifyCommand_DomainStrings[DomainStrings]
    T21_cli_cli_VerifyCommand_UsecasePurity[UsecasePurity]
    T21_cli_cli_VerifyCommand_DocLinks[DocLinks]
    T21_cli_cli_VerifyCommand_ViewFreshness[ViewFreshness]
    T21_cli_cli_VerifyCommand_SpecSignals[SpecSignals]
    T21_cli_cli_VerifyCommand_PlanArtifactRefs[PlanArtifactRefs]
    T21_cli_cli_VerifyCommand_CatalogueSpecRefs[CatalogueSpecRefs]
  end
  subgraph T18_cli_cli_ViewAction["commands::track::ViewAction"]
    direction TB
    T18_cli_cli_ViewAction__self[ViewAction]
    T18_cli_cli_ViewAction_Validate[Validate]
    T18_cli_cli_ViewAction_Sync[Sync]
  end
  subgraph T24_cli_cli_WaitAndMergeArgs["commands::pr::WaitAndMergeArgs"]
    direction TB
    T24_cli_cli_WaitAndMergeArgs__self[WaitAndMergeArgs]
  end
  F62_cli_cli_cli__commands__plan__codex_local__plan_input_from_args[[plan_input_from_args]]
  F65_cli_cli_cli__commands__plan__codex_local__run_execute_codex_local[[run_execute_codex_local]]
  F66_cli_cli_cli__commands__review__codex_local__emit_outcome_output_to[[emit_outcome_output_to]]
  F66_cli_cli_cli__commands__review__codex_local__review_input_from_args[[review_input_from_args]]
  F67_cli_cli_cli__commands__review__codex_local__run_execute_codex_local[[run_execute_codex_local]]
  end
  subgraph cli_cli_module_error["cli::error"]
    direction TB
  subgraph T16_cli_cli_CliError["error::CliError"]
    direction TB
    T16_cli_cli_CliError__self[CliError]
    T16_cli_cli_CliError_Message[Message]
    T16_cli_cli_CliError_Io[Io]
  end
  end
end
T22_domain_domain_DataRole_value_object --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_entity --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_aggregate_root --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_domain_service --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_use_case --> T22_domain_domain_DataRole__self
T22_domain_domain_RoleKind_from_data_role --o T22_domain_domain_DataRole__self
T22_domain_domain_RoleKind_from_data_role --> T22_domain_domain_RoleKind__self
T22_domain_domain_RoleKind_from_contract_role --> T22_domain_domain_RoleKind__self
T22_domain_domain_RoleKind_from_function_role --> T22_domain_domain_RoleKind__self
T30_usecase_usecase_ArchInteractor_new --> T30_usecase_usecase_ArchInteractor__self
T37_usecase_usecase_ConventionsInteractor_new --> T37_usecase_usecase_ConventionsInteractor__self
T30_usecase_usecase_FileInteractor_new --> T30_usecase_usecase_FileInteractor__self
T33_usecase_usecase_PlannerInteractor_new --> T33_usecase_usecase_PlannerInteractor__self
R27_usecase_usecase_PlannerPort_run --> T29_usecase_usecase_PlanRunOutput__self
R27_usecase_usecase_PlannerPort_run --> T32_usecase_usecase_PlannerPortError__self
R30_usecase_usecase_PlannerService_run_codex_local --> T29_usecase_usecase_PlanRunOutput__self
R30_usecase_usecase_PlannerService_run_codex_local --> T32_usecase_usecase_PlannerPortError__self
T32_usecase_usecase_VerifyInteractor_new --> T32_usecase_usecase_VerifyInteractor__self
T30_usecase_usecase_ArchInteractor__self -.impl.-> R27_usecase_usecase_ArchService__self
T37_usecase_usecase_ConventionsInteractor__self -.impl.-> R34_usecase_usecase_ConventionsService__self
T30_usecase_usecase_FileInteractor__self -.impl.-> R27_usecase_usecase_FileService__self
T33_usecase_usecase_PlannerInteractor__self -.impl.-> R30_usecase_usecase_PlannerService__self
T32_usecase_usecase_VerifyInteractor__self -.impl.-> R29_usecase_usecase_VerifyService__self
T49_infrastructure_infrastructure_CodexPlannerAdapter_new --> T49_infrastructure_infrastructure_CodexPlannerAdapter__self
T49_infrastructure_infrastructure_CodexPlannerAdapter__self -.impl.-> R27_usecase_usecase_PlannerPort__self
T32_cli_driver_cli_driver_ArchDriver_handle --o T31_cli_driver_cli_driver_ArchInput__self
T32_cli_driver_cli_driver_ArchDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T32_cli_driver_cli_driver_ArchDriver_new --> T32_cli_driver_cli_driver_ArchDriver__self
T39_cli_driver_cli_driver_ConventionsDriver_handle --o T38_cli_driver_cli_driver_ConventionsInput__self
T39_cli_driver_cli_driver_ConventionsDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T39_cli_driver_cli_driver_ConventionsDriver_new --> T39_cli_driver_cli_driver_ConventionsDriver__self
T32_cli_driver_cli_driver_DemoDriver_handle --o T31_cli_driver_cli_driver_DemoInput__self
T32_cli_driver_cli_driver_DemoDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T34_cli_driver_cli_driver_DomainDriver_handle --o T33_cli_driver_cli_driver_DomainInput__self
T34_cli_driver_cli_driver_DomainDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T33_cli_driver_cli_driver_DomainInput_ExportSchema --o T39_cli_driver_cli_driver_ExportSchemaInput__self
T31_cli_driver_cli_driver_DryDriver_handle --o T30_cli_driver_cli_driver_DryInput__self
T31_cli_driver_cli_driver_DryDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T32_cli_driver_cli_driver_FileDriver_handle --o T31_cli_driver_cli_driver_FileInput__self
T32_cli_driver_cli_driver_FileDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T32_cli_driver_cli_driver_FileDriver_new --> T32_cli_driver_cli_driver_FileDriver__self
T31_cli_driver_cli_driver_GitDriver_handle --o T30_cli_driver_cli_driver_GitInput__self
T31_cli_driver_cli_driver_GitDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T33_cli_driver_cli_driver_GuardDriver_handle --o T32_cli_driver_cli_driver_GuardInput__self
T33_cli_driver_cli_driver_GuardDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T32_cli_driver_cli_driver_HookDriver_handle --o T31_cli_driver_cli_driver_HookInput__self
T32_cli_driver_cli_driver_HookDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T31_cli_driver_cli_driver_HookInput_Dispatch --o|hook| T30_cli_driver_cli_driver_HookName__self
T32_cli_driver_cli_driver_PlanDriver_handle --o T31_cli_driver_cli_driver_PlanInput__self
T32_cli_driver_cli_driver_PlanDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T32_cli_driver_cli_driver_PlanDriver_new --> T32_cli_driver_cli_driver_PlanDriver__self
T30_cli_driver_cli_driver_PrDriver_handle --o T29_cli_driver_cli_driver_PrInput__self
T30_cli_driver_cli_driver_PrDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T37_cli_driver_cli_driver_RefVerifyDriver_handle --o T36_cli_driver_cli_driver_RefVerifyInput__self
T37_cli_driver_cli_driver_RefVerifyDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_RefVerifyInput_Run --o T39_cli_driver_cli_driver_RefVerifyRunInput__self
T36_cli_driver_cli_driver_RefVerifyInput_CheckApproved --o T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput__self
T34_cli_driver_cli_driver_ReviewDriver_handle --o T33_cli_driver_cli_driver_ReviewInput__self
T34_cli_driver_cli_driver_ReviewDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T39_cli_driver_cli_driver_SemanticDupDriver_handle --o T38_cli_driver_cli_driver_SemanticDupInput__self
T39_cli_driver_cli_driver_SemanticDupDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T34_cli_driver_cli_driver_SignalDriver_handle --o T33_cli_driver_cli_driver_SignalInput__self
T34_cli_driver_cli_driver_SignalDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T33_cli_driver_cli_driver_SignalInput_CheckAdrUser --o|gate| T36_cli_driver_cli_driver_SignalGateName__self
T33_cli_driver_cli_driver_SignalInput_CheckSpecAdr --o|gate| T36_cli_driver_cli_driver_SignalGateName__self
T33_cli_driver_cli_driver_SignalInput_CheckCatalogSpec --o|gate| T36_cli_driver_cli_driver_SignalGateName__self
T33_cli_driver_cli_driver_SignalInput_CheckImplCatalog --o|gate| T36_cli_driver_cli_driver_SignalGateName__self
T33_cli_driver_cli_driver_SignalInput_CheckGate --o|gate| T36_cli_driver_cli_driver_SignalGateName__self
T37_cli_driver_cli_driver_TelemetryDriver_handle --o T36_cli_driver_cli_driver_TelemetryInput__self
T37_cli_driver_cli_driver_TelemetryDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_TelemetryInput_Report --o T42_cli_driver_cli_driver_TelemetryReportInput__self
T33_cli_driver_cli_driver_TrackDriver_handle --o T32_cli_driver_cli_driver_TrackInput__self
T33_cli_driver_cli_driver_TrackDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T34_cli_driver_cli_driver_VerifyDriver_handle --o T33_cli_driver_cli_driver_VerifyInput__self
T34_cli_driver_cli_driver_VerifyDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T34_cli_driver_cli_driver_VerifyDriver_new --> T34_cli_driver_cli_driver_VerifyDriver__self
T51_cli_composition_cli_composition_ArchCompositionRoot_new --> T51_cli_composition_cli_composition_ArchCompositionRoot__self
T51_cli_composition_cli_composition_ArchCompositionRoot_arch_driver --> T32_cli_driver_cli_driver_ArchDriver__self
T58_cli_composition_cli_composition_ConventionsCompositionRoot_new --> T58_cli_composition_cli_composition_ConventionsCompositionRoot__self
T58_cli_composition_cli_composition_ConventionsCompositionRoot_conventions_driver --> T39_cli_driver_cli_driver_ConventionsDriver__self
T51_cli_composition_cli_composition_FileCompositionRoot_new --> T51_cli_composition_cli_composition_FileCompositionRoot__self
T51_cli_composition_cli_composition_FileCompositionRoot_file_driver --> T32_cli_driver_cli_driver_FileDriver__self
T51_cli_composition_cli_composition_PlanCompositionRoot_new --> T51_cli_composition_cli_composition_PlanCompositionRoot__self
T51_cli_composition_cli_composition_PlanCompositionRoot_plan_driver --> T32_cli_driver_cli_driver_PlanDriver__self
T53_cli_composition_cli_composition_VerifyCompositionRoot_new --> T53_cli_composition_cli_composition_VerifyCompositionRoot__self
T53_cli_composition_cli_composition_VerifyCompositionRoot_verify_driver --> T34_cli_driver_cli_driver_VerifyDriver__self
T18_cli_cli_CheckFlags__self --o|gate| T15_cli_cli_GateArg__self
T22_cli_cli_DryResultsArgs__self --o|filter| T24_cli_cli_VerdictFilterArg__self
T23_cli_cli_SignalCheckArgs__self --o|gate| T15_cli_cli_GateArg__self
F62_cli_cli_cli__commands__plan__codex_local__plan_input_from_args --o T26_cli_cli_PlanCodexLocalArgs__self
F62_cli_cli_cli__commands__plan__codex_local__plan_input_from_args --> T31_cli_driver_cli_driver_PlanInput__self
F62_cli_cli_cli__commands__plan__codex_local__plan_input_from_args --> T16_cli_cli_CliError__self
F65_cli_cli_cli__commands__plan__codex_local__run_execute_codex_local --o T26_cli_cli_PlanCodexLocalArgs__self
F66_cli_cli_cli__commands__review__codex_local__emit_outcome_output_to --> T16_cli_cli_CliError__self
F66_cli_cli_cli__commands__review__codex_local__review_input_from_args --o T22_cli_cli_CodexLocalArgs__self
F66_cli_cli_cli__commands__review__codex_local__review_input_from_args --> T33_cli_driver_cli_driver_ReviewInput__self
F66_cli_cli_cli__commands__review__codex_local__review_input_from_args --> T16_cli_cli_CliError__self
F67_cli_cli_cli__commands__review__codex_local__run_execute_codex_local --o T22_cli_cli_CodexLocalArgs__self
class T22_domain_domain_DataRole_ValueObject variant_node
class T22_domain_domain_DataRole_Entity variant_node
class T22_domain_domain_DataRole_AggregateRoot variant_node
class T22_domain_domain_DataRole_DomainService variant_node
class T22_domain_domain_DataRole_Specification variant_node
class T22_domain_domain_DataRole_Factory variant_node
class T22_domain_domain_DataRole_UseCase variant_node
class T22_domain_domain_DataRole_Interactor variant_node
class T22_domain_domain_DataRole_Command variant_node
class T22_domain_domain_DataRole_Query variant_node
class T22_domain_domain_DataRole_Dto variant_node
class T22_domain_domain_DataRole_ErrorType variant_node
class T22_domain_domain_DataRole_SecondaryAdapter variant_node
class T22_domain_domain_DataRole_DomainEvent variant_node
class T22_domain_domain_DataRole_EventPolicy variant_node
class T22_domain_domain_DataRole_CompositionRoot variant_node
class T22_domain_domain_DataRole_PrimaryAdapter variant_node
class T22_domain_domain_DataRole_value_object method_node
class T22_domain_domain_DataRole_entity method_node
class T22_domain_domain_DataRole_aggregate_root method_node
class T22_domain_domain_DataRole_domain_service method_node
class T22_domain_domain_DataRole_use_case method_node
class T22_domain_domain_DataRole_variant_name method_node
class T22_domain_domain_DataRole__self value_object
class T22_domain_domain_RoleKind_ValueObject variant_node
class T22_domain_domain_RoleKind_Entity variant_node
class T22_domain_domain_RoleKind_AggregateRoot variant_node
class T22_domain_domain_RoleKind_DomainService variant_node
class T22_domain_domain_RoleKind_Specification variant_node
class T22_domain_domain_RoleKind_Factory variant_node
class T22_domain_domain_RoleKind_UseCase variant_node
class T22_domain_domain_RoleKind_Interactor variant_node
class T22_domain_domain_RoleKind_Command variant_node
class T22_domain_domain_RoleKind_Query variant_node
class T22_domain_domain_RoleKind_Dto variant_node
class T22_domain_domain_RoleKind_ErrorType variant_node
class T22_domain_domain_RoleKind_SecondaryAdapter variant_node
class T22_domain_domain_RoleKind_DomainEvent variant_node
class T22_domain_domain_RoleKind_EventPolicy variant_node
class T22_domain_domain_RoleKind_SpecificationPort variant_node
class T22_domain_domain_RoleKind_ApplicationService variant_node
class T22_domain_domain_RoleKind_SecondaryPort variant_node
class T22_domain_domain_RoleKind_Repository variant_node
class T22_domain_domain_RoleKind_CompositionRoot variant_node
class T22_domain_domain_RoleKind_PrimaryAdapter variant_node
class T22_domain_domain_RoleKind_FreeFunction variant_node
class T22_domain_domain_RoleKind_UseCaseFunction variant_node
class T22_domain_domain_RoleKind_from_data_role method_node
class T22_domain_domain_RoleKind_from_contract_role method_node
class T22_domain_domain_RoleKind_from_function_role method_node
class T22_domain_domain_RoleKind_variant_name method_node
class T22_domain_domain_RoleKind__self value_object
class T30_usecase_usecase_ArchInteractor_new method_node
class T30_usecase_usecase_ArchInteractor__self interactor
class R27_usecase_usecase_ArchService_render_tree method_node
class R27_usecase_usecase_ArchService_render_tree_full method_node
class R27_usecase_usecase_ArchService_render_members method_node
class R27_usecase_usecase_ArchService_render_direct_checks method_node
class R27_usecase_usecase_ArchService__self app_service
class T37_usecase_usecase_ConventionsInteractor_new method_node
class T37_usecase_usecase_ConventionsInteractor__self interactor
class R34_usecase_usecase_ConventionsService_add_convention method_node
class R34_usecase_usecase_ConventionsService_update_index method_node
class R34_usecase_usecase_ConventionsService_verify_index method_node
class R34_usecase_usecase_ConventionsService__self app_service
class T30_usecase_usecase_FileInteractor_new method_node
class T30_usecase_usecase_FileInteractor__self interactor
class R27_usecase_usecase_FileService_write_atomic method_node
class R27_usecase_usecase_FileService__self app_service
class T29_usecase_usecase_PlanRunOutput__self dto
class T33_usecase_usecase_PlannerInteractor_new method_node
class T33_usecase_usecase_PlannerInteractor__self interactor
class T32_usecase_usecase_PlannerPortError_MissingPromptSource variant_node
class T32_usecase_usecase_PlannerPortError_PlannerUnavailable variant_node
class T32_usecase_usecase_PlannerPortError_PlannerTimeout variant_node
class T32_usecase_usecase_PlannerPortError_PlannerFailed variant_node
class T32_usecase_usecase_PlannerPortError__self error_type
class R27_usecase_usecase_PlannerPort_run method_node
class R27_usecase_usecase_PlannerPort__self secondary_port
class R30_usecase_usecase_PlannerService_run_codex_local method_node
class R30_usecase_usecase_PlannerService__self app_service
class T32_usecase_usecase_VerifyInteractor_new method_node
class T32_usecase_usecase_VerifyInteractor__self interactor
class R29_usecase_usecase_VerifyService_verify_tech_stack method_node
class R29_usecase_usecase_VerifyService_verify_latest_track method_node
class R29_usecase_usecase_VerifyService_verify_arch_docs method_node
class R29_usecase_usecase_VerifyService_verify_layers method_node
class R29_usecase_usecase_VerifyService_verify_hooks_path method_node
class R29_usecase_usecase_VerifyService_verify_spec_attribution method_node
class R29_usecase_usecase_VerifyService_verify_spec_frontmatter method_node
class R29_usecase_usecase_VerifyService_verify_canonical_modules method_node
class R29_usecase_usecase_VerifyService_verify_module_size method_node
class R29_usecase_usecase_VerifyService_verify_domain_purity method_node
class R29_usecase_usecase_VerifyService_verify_domain_strings method_node
class R29_usecase_usecase_VerifyService_verify_usecase_purity method_node
class R29_usecase_usecase_VerifyService_verify_doc_links method_node
class R29_usecase_usecase_VerifyService_verify_view_freshness method_node
class R29_usecase_usecase_VerifyService_verify_spec_signals method_node
class R29_usecase_usecase_VerifyService_verify_plan_artifact_refs method_node
class R29_usecase_usecase_VerifyService_verify_catalogue_spec_refs method_node
class R29_usecase_usecase_VerifyService__self app_service
class T49_infrastructure_infrastructure_CodexPlannerAdapter_new method_node
class T49_infrastructure_infrastructure_CodexPlannerAdapter__self secondary_adapter
class T32_cli_driver_cli_driver_ArchDriver_handle method_node
class T32_cli_driver_cli_driver_ArchDriver_new method_node
class T31_cli_driver_cli_driver_ArchInput_Tree variant_node
class T31_cli_driver_cli_driver_ArchInput_TreeFull variant_node
class T31_cli_driver_cli_driver_ArchInput_Members variant_node
class T31_cli_driver_cli_driver_ArchInput_DirectChecks variant_node
class T31_cli_driver_cli_driver_ArchInput__self dto
class T39_cli_driver_cli_driver_ConventionsDriver_handle method_node
class T39_cli_driver_cli_driver_ConventionsDriver_new method_node
class T38_cli_driver_cli_driver_ConventionsInput_Add variant_node
class T38_cli_driver_cli_driver_ConventionsInput_UpdateIndex variant_node
class T38_cli_driver_cli_driver_ConventionsInput_VerifyIndex variant_node
class T38_cli_driver_cli_driver_ConventionsInput__self dto
class T32_cli_driver_cli_driver_DemoDriver_handle method_node
class T31_cli_driver_cli_driver_DemoInput_Run variant_node
class T31_cli_driver_cli_driver_DemoInput__self dto
class T34_cli_driver_cli_driver_DomainDriver_handle method_node
class T33_cli_driver_cli_driver_DomainInput_ExportSchema variant_node
class T33_cli_driver_cli_driver_DomainInput__self dto
class T39_cli_driver_cli_driver_ExportSchemaInput__self dto
class T31_cli_driver_cli_driver_DryDriver_handle method_node
class T30_cli_driver_cli_driver_DryInput_Write variant_node
class T30_cli_driver_cli_driver_DryInput_Results variant_node
class T30_cli_driver_cli_driver_DryInput_CheckApproved variant_node
class T30_cli_driver_cli_driver_DryInput_FixLocal variant_node
class T30_cli_driver_cli_driver_DryInput__self dto
class T32_cli_driver_cli_driver_FileDriver_handle method_node
class T32_cli_driver_cli_driver_FileDriver_new method_node
class T31_cli_driver_cli_driver_FileInput_WriteAtomic variant_node
class T31_cli_driver_cli_driver_FileInput__self dto
class T31_cli_driver_cli_driver_GitDriver_handle method_node
class T30_cli_driver_cli_driver_GitInput_AddAll variant_node
class T30_cli_driver_cli_driver_GitInput_AddFromFile variant_node
class T30_cli_driver_cli_driver_GitInput_CommitFromFile variant_node
class T30_cli_driver_cli_driver_GitInput_NoteFromFile variant_node
class T30_cli_driver_cli_driver_GitInput_SwitchAndPull variant_node
class T30_cli_driver_cli_driver_GitInput_Unstage variant_node
class T30_cli_driver_cli_driver_GitInput_CurrentBranchTrackIdStrict variant_node
class T30_cli_driver_cli_driver_GitInput__self dto
class T33_cli_driver_cli_driver_GuardDriver_handle method_node
class T32_cli_driver_cli_driver_GuardInput_Check variant_node
class T32_cli_driver_cli_driver_GuardInput__self dto
class T32_cli_driver_cli_driver_HookDriver_handle method_node
class T31_cli_driver_cli_driver_HookInput_Dispatch variant_node
class T31_cli_driver_cli_driver_HookInput__self dto
class T30_cli_driver_cli_driver_HookName_HooksPathSetup variant_node
class T30_cli_driver_cli_driver_HookName_BlockDirectGitOps variant_node
class T30_cli_driver_cli_driver_HookName_BlockTestFileDeletion variant_node
class T30_cli_driver_cli_driver_HookName_GitRefUpdate variant_node
class T30_cli_driver_cli_driver_HookName_GitPrePush variant_node
class T30_cli_driver_cli_driver_HookName_SkillCompliance variant_node
class T30_cli_driver_cli_driver_HookName__self dto
class T32_cli_driver_cli_driver_PlanDriver_handle method_node
class T32_cli_driver_cli_driver_PlanDriver_new method_node
class T31_cli_driver_cli_driver_PlanInput_RunCodexLocal variant_node
class T31_cli_driver_cli_driver_PlanInput__self dto
class T30_cli_driver_cli_driver_PrDriver_handle method_node
class T29_cli_driver_cli_driver_PrInput_Push variant_node
class T29_cli_driver_cli_driver_PrInput_Ensure variant_node
class T29_cli_driver_cli_driver_PrInput_Status variant_node
class T29_cli_driver_cli_driver_PrInput_WaitAndMerge variant_node
class T29_cli_driver_cli_driver_PrInput_TriggerReview variant_node
class T29_cli_driver_cli_driver_PrInput_PollReview variant_node
class T29_cli_driver_cli_driver_PrInput_ReviewCycle variant_node
class T29_cli_driver_cli_driver_PrInput__self dto
class T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput__self dto
class T37_cli_driver_cli_driver_RefVerifyDriver_handle method_node
class T36_cli_driver_cli_driver_RefVerifyInput_Run variant_node
class T36_cli_driver_cli_driver_RefVerifyInput_CheckApproved variant_node
class T36_cli_driver_cli_driver_RefVerifyInput__self dto
class T39_cli_driver_cli_driver_RefVerifyRunInput__self dto
class F64_cli_driver_cli_driver_cli_driver__ref_verify__format_pair_status free_function
class F64_cli_driver_cli_driver_cli_driver__ref_verify__format_pair_status function_node
class T36_cli_driver_cli_driver_CommandOutcome__self dto
class T34_cli_driver_cli_driver_ReviewDriver_handle method_node
class T33_cli_driver_cli_driver_ReviewInput_RunCodex variant_node
class T33_cli_driver_cli_driver_ReviewInput_RunClaude variant_node
class T33_cli_driver_cli_driver_ReviewInput_RunLocal variant_node
class T33_cli_driver_cli_driver_ReviewInput_RunFixLocal variant_node
class T33_cli_driver_cli_driver_ReviewInput_CheckApproved variant_node
class T33_cli_driver_cli_driver_ReviewInput_Results variant_node
class T33_cli_driver_cli_driver_ReviewInput_Classify variant_node
class T33_cli_driver_cli_driver_ReviewInput_Files variant_node
class T33_cli_driver_cli_driver_ReviewInput_ValidateScope variant_node
class T33_cli_driver_cli_driver_ReviewInput_GetBriefing variant_node
class T33_cli_driver_cli_driver_ReviewInput_PersistCommitHash variant_node
class T33_cli_driver_cli_driver_ReviewInput__self dto
class T39_cli_driver_cli_driver_SemanticDupDriver_handle method_node
class T38_cli_driver_cli_driver_SemanticDupInput_FindSimilar variant_node
class T38_cli_driver_cli_driver_SemanticDupInput_IndexBuild variant_node
class T38_cli_driver_cli_driver_SemanticDupInput_IndexMeasureQuality variant_node
class T38_cli_driver_cli_driver_SemanticDupInput_DupCheck variant_node
class T38_cli_driver_cli_driver_SemanticDupInput__self dto
class T34_cli_driver_cli_driver_SignalDriver_handle method_node
class T36_cli_driver_cli_driver_SignalGateName_Commit variant_node
class T36_cli_driver_cli_driver_SignalGateName_Merge variant_node
class T36_cli_driver_cli_driver_SignalGateName__self dto
class T33_cli_driver_cli_driver_SignalInput_CalcAdrUser variant_node
class T33_cli_driver_cli_driver_SignalInput_CheckAdrUser variant_node
class T33_cli_driver_cli_driver_SignalInput_CalcSpecAdr variant_node
class T33_cli_driver_cli_driver_SignalInput_CheckSpecAdr variant_node
class T33_cli_driver_cli_driver_SignalInput_CalcCatalogSpec variant_node
class T33_cli_driver_cli_driver_SignalInput_CheckCatalogSpec variant_node
class T33_cli_driver_cli_driver_SignalInput_CalcImplCatalog variant_node
class T33_cli_driver_cli_driver_SignalInput_CheckImplCatalog variant_node
class T33_cli_driver_cli_driver_SignalInput_CheckGate variant_node
class T33_cli_driver_cli_driver_SignalInput__self dto
class T37_cli_driver_cli_driver_TelemetryDriver_handle method_node
class T36_cli_driver_cli_driver_TelemetryInput_Report variant_node
class T36_cli_driver_cli_driver_TelemetryInput_EmitArchivedTrackSubcommand variant_node
class T36_cli_driver_cli_driver_TelemetryInput__self dto
class T42_cli_driver_cli_driver_TelemetryReportInput__self dto
class T33_cli_driver_cli_driver_TrackDriver_handle method_node
class T32_cli_driver_cli_driver_TrackInput_Init variant_node
class T32_cli_driver_cli_driver_TrackInput_Transition variant_node
class T32_cli_driver_cli_driver_TrackInput_Resolve variant_node
class T32_cli_driver_cli_driver_TrackInput_BranchCreate variant_node
class T32_cli_driver_cli_driver_TrackInput_BranchSwitch variant_node
class T32_cli_driver_cli_driver_TrackInput_ViewsValidate variant_node
class T32_cli_driver_cli_driver_TrackInput_ViewsSync variant_node
class T32_cli_driver_cli_driver_TrackInput_AddTask variant_node
class T32_cli_driver_cli_driver_TrackInput_SetOverride variant_node
class T32_cli_driver_cli_driver_TrackInput_ClearOverride variant_node
class T32_cli_driver_cli_driver_TrackInput_NextTask variant_node
class T32_cli_driver_cli_driver_TrackInput_TaskCounts variant_node
class T32_cli_driver_cli_driver_TrackInput_Archive variant_node
class T32_cli_driver_cli_driver_TrackInput_DetectActive variant_node
class T32_cli_driver_cli_driver_TrackInput__self dto
class T34_cli_driver_cli_driver_VerifyDriver_handle method_node
class T34_cli_driver_cli_driver_VerifyDriver_new method_node
class T33_cli_driver_cli_driver_VerifyInput_TechStack variant_node
class T33_cli_driver_cli_driver_VerifyInput_LatestTrack variant_node
class T33_cli_driver_cli_driver_VerifyInput_ArchDocs variant_node
class T33_cli_driver_cli_driver_VerifyInput_Layers variant_node
class T33_cli_driver_cli_driver_VerifyInput_HooksPath variant_node
class T33_cli_driver_cli_driver_VerifyInput_SpecAttribution variant_node
class T33_cli_driver_cli_driver_VerifyInput_SpecFrontmatter variant_node
class T33_cli_driver_cli_driver_VerifyInput_CanonicalModules variant_node
class T33_cli_driver_cli_driver_VerifyInput_ModuleSize variant_node
class T33_cli_driver_cli_driver_VerifyInput_DomainPurity variant_node
class T33_cli_driver_cli_driver_VerifyInput_DomainStrings variant_node
class T33_cli_driver_cli_driver_VerifyInput_UsecasePurity variant_node
class T33_cli_driver_cli_driver_VerifyInput_DocLinks variant_node
class T33_cli_driver_cli_driver_VerifyInput_ViewFreshness variant_node
class T33_cli_driver_cli_driver_VerifyInput_SpecSignals variant_node
class T33_cli_driver_cli_driver_VerifyInput_PlanArtifactRefs variant_node
class T33_cli_driver_cli_driver_VerifyInput_CatalogueSpecRefs variant_node
class T33_cli_driver_cli_driver_VerifyInput__self dto
class T51_cli_composition_cli_composition_ArchCompositionRoot_new method_node
class T51_cli_composition_cli_composition_ArchCompositionRoot_arch_driver method_node
class T58_cli_composition_cli_composition_ConventionsCompositionRoot_new method_node
class T58_cli_composition_cli_composition_ConventionsCompositionRoot_conventions_driver method_node
class T48_cli_composition_cli_composition_CompositionError_ConfigLoad variant_node
class T48_cli_composition_cli_composition_CompositionError_AdapterInit variant_node
class T48_cli_composition_cli_composition_CompositionError_WiringFailed variant_node
class T48_cli_composition_cli_composition_CompositionError_Usecase variant_node
class T48_cli_composition_cli_composition_CompositionError_Infrastructure variant_node
class T48_cli_composition_cli_composition_CompositionError__self error_type
class T51_cli_composition_cli_composition_FileCompositionRoot_new method_node
class T51_cli_composition_cli_composition_FileCompositionRoot_file_driver method_node
class T51_cli_composition_cli_composition_PlanCompositionRoot_new method_node
class T51_cli_composition_cli_composition_PlanCompositionRoot_plan_driver method_node
class T53_cli_composition_cli_composition_VerifyCompositionRoot_new method_node
class T53_cli_composition_cli_composition_VerifyCompositionRoot_verify_driver method_node
class T19_cli_cli_ArchCommand_Tree variant_node
class T19_cli_cli_ArchCommand_TreeFull variant_node
class T19_cli_cli_ArchCommand_Members variant_node
class T19_cli_cli_ArchCommand_DirectChecks variant_node
class T19_cli_cli_ArchCommand__self dto
class T20_cli_cli_BranchAction_Create variant_node
class T20_cli_cli_BranchAction_Switch variant_node
class T20_cli_cli_BranchAction__self dto
class T18_cli_cli_BranchArgs__self dto
class T23_cli_cli_CalcAdrUserArgs__self dto
class T27_cli_cli_CalcCatalogSpecArgs__self dto
class T27_cli_cli_CalcImplCatalogArgs__self dto
class T23_cli_cli_CalcSpecAdrArgs__self dto
class T29_cli_cli_CatalogueSpecRefsArgs__self dto
class T24_cli_cli_CheckAdrUserArgs__self dto
class T25_cli_cli_CheckApprovedArgs__self dto
class T28_cli_cli_CheckCatalogSpecArgs__self dto
class T18_cli_cli_CheckFlags__self dto
class T28_cli_cli_CheckImplCatalogArgs__self dto
class T24_cli_cli_CheckSpecAdrArgs__self dto
class T23_cli_cli_ClaudeLocalArgs__self dto
class T19_cli_cli_CliHookName_HooksPathSetup variant_node
class T19_cli_cli_CliHookName_BlockDirectGitOps variant_node
class T19_cli_cli_CliHookName_BlockTestFileDeletion variant_node
class T19_cli_cli_CliHookName_GitRefUpdate variant_node
class T19_cli_cli_CliHookName_GitPrePush variant_node
class T19_cli_cli_CliHookName_SkillCompliance variant_node
class T19_cli_cli_CliHookName__self dto
class T22_cli_cli_CodexLocalArgs__self dto
class T25_cli_cli_CodexRoundTypeArg_Fast variant_node
class T25_cli_cli_CodexRoundTypeArg_Final variant_node
class T25_cli_cli_CodexRoundTypeArg__self dto
class T26_cli_cli_CommitFromFileArgs__self dto
class T26_cli_cli_ConventionsCommand_Add variant_node
class T26_cli_cli_ConventionsCommand_UpdateIndex variant_node
class T26_cli_cli_ConventionsCommand_VerifyIndex variant_node
class T26_cli_cli_ConventionsCommand__self dto
class T21_cli_cli_DomainCommand_ExportSchema variant_node
class T21_cli_cli_DomainCommand__self dto
class T28_cli_cli_DryCheckApprovedArgs__self dto
class T18_cli_cli_DryCommand_Write variant_node
class T18_cli_cli_DryCommand_Results variant_node
class T18_cli_cli_DryCommand_CheckApproved variant_node
class T18_cli_cli_DryCommand_FixLocal variant_node
class T18_cli_cli_DryCommand__self dto
class T23_cli_cli_DryFixLocalArgs__self dto
class T22_cli_cli_DryResultsArgs__self dto
class T20_cli_cli_DryWriteArgs__self dto
class T20_cli_cli_DupCheckArgs__self dto
class T25_cli_cli_DupIndexBuildArgs__self dto
class T23_cli_cli_DupIndexCommand_Build variant_node
class T23_cli_cli_DupIndexCommand_MeasureQuality variant_node
class T23_cli_cli_DupIndexCommand__self dto
class T34_cli_cli_DupIndexMeasureQualityArgs__self dto
class T20_cli_cli_EnsurePrArgs__self dto
class T24_cli_cli_ExportSchemaArgs__self dto
class T16_cli_cli_FileArgs__self dto
class T19_cli_cli_FileCommand_WriteAtomic variant_node
class T19_cli_cli_FileCommand__self dto
class T23_cli_cli_FindSimilarArgs__self dto
class T27_cli_cli_FixpointResolveArgs__self dto
class T15_cli_cli_GateArg_Commit variant_node
class T15_cli_cli_GateArg_Merge variant_node
class T15_cli_cli_GateArg__self dto
class T18_cli_cli_GitCommand_AddAll variant_node
class T18_cli_cli_GitCommand_AddPaths variant_node
class T18_cli_cli_GitCommand_CommitFromFile variant_node
class T18_cli_cli_GitCommand_SwitchAndPull variant_node
class T18_cli_cli_GitCommand_Unstage variant_node
class T18_cli_cli_GitCommand__self dto
class T20_cli_cli_GuardCommand_Check variant_node
class T20_cli_cli_GuardCommand__self dto
class T19_cli_cli_HookCommand_Dispatch variant_node
class T19_cli_cli_HookCommand__self dto
class T28_cli_cli_PlanArtifactRefsArgs__self dto
class T26_cli_cli_PlanCodexLocalArgs__self dto
class T19_cli_cli_PlanCommand_CodexLocal variant_node
class T19_cli_cli_PlanCommand__self dto
class T22_cli_cli_PollReviewArgs__self dto
class T17_cli_cli_PrCommand_Push variant_node
class T17_cli_cli_PrCommand_EnsurePr variant_node
class T17_cli_cli_PrCommand_Status variant_node
class T17_cli_cli_PrCommand_WaitAndMerge variant_node
class T17_cli_cli_PrCommand_TriggerReview variant_node
class T17_cli_cli_PrCommand_PollReview variant_node
class T17_cli_cli_PrCommand_ReviewCycle variant_node
class T17_cli_cli_PrCommand__self dto
class T16_cli_cli_PushArgs__self dto
class T24_cli_cli_RefVerifyCommand_Run variant_node
class T24_cli_cli_RefVerifyCommand_CheckApproved variant_node
class T24_cli_cli_RefVerifyCommand__self dto
class T18_cli_cli_ReportArgs__self dto
class T19_cli_cli_ResolveArgs__self dto
class T19_cli_cli_ResultsArgs__self dto
class T20_cli_cli_ResultsLimit_Zero variant_node
class T20_cli_cli_ResultsLimit_Count variant_node
class T20_cli_cli_ResultsLimit_All variant_node
class T20_cli_cli_ResultsLimit__self dto
class T21_cli_cli_ReviewCommand_CodexLocal variant_node
class T21_cli_cli_ReviewCommand_ClaudeLocal variant_node
class T21_cli_cli_ReviewCommand_Local variant_node
class T21_cli_cli_ReviewCommand_FixLocal variant_node
class T21_cli_cli_ReviewCommand_CheckApproved variant_node
class T21_cli_cli_ReviewCommand_Results variant_node
class T21_cli_cli_ReviewCommand_Classify variant_node
class T21_cli_cli_ReviewCommand_Files variant_node
class T21_cli_cli_ReviewCommand__self dto
class T23_cli_cli_ReviewCycleArgs__self dto
class T23_cli_cli_RoundTypeFilter_Fast variant_node
class T23_cli_cli_RoundTypeFilter_Final variant_node
class T23_cli_cli_RoundTypeFilter_Any variant_node
class T23_cli_cli_RoundTypeFilter__self dto
class T15_cli_cli_RunArgs__self dto
class T25_cli_cli_SetCommitHashArgs__self dto
class T23_cli_cli_SignalCheckArgs__self dto
class T21_cli_cli_SignalCommand_CalcAdrUser variant_node
class T21_cli_cli_SignalCommand_CheckAdrUser variant_node
class T21_cli_cli_SignalCommand_CalcSpecAdr variant_node
class T21_cli_cli_SignalCommand_CheckSpecAdr variant_node
class T21_cli_cli_SignalCommand_CalcCatalogSpec variant_node
class T21_cli_cli_SignalCommand_CheckCatalogSpec variant_node
class T21_cli_cli_SignalCommand_CalcImplCatalog variant_node
class T21_cli_cli_SignalCommand_CheckImplCatalog variant_node
class T21_cli_cli_SignalCommand_Check variant_node
class T21_cli_cli_SignalCommand__self dto
class T22_cli_cli_SpecVerifyArgs__self dto
class T18_cli_cli_StatusArgs__self dto
class T25_cli_cli_SwitchAndPullArgs__self dto
class T24_cli_cli_TelemetryCommand_Report variant_node
class T24_cli_cli_TelemetryCommand__self dto
class T20_cli_cli_TrackCommand_Archive variant_node
class T20_cli_cli_TrackCommand_Transition variant_node
class T20_cli_cli_TrackCommand_Branch variant_node
class T20_cli_cli_TrackCommand_Resolve variant_node
class T20_cli_cli_TrackCommand_Views variant_node
class T20_cli_cli_TrackCommand_AddTask variant_node
class T20_cli_cli_TrackCommand_SetOverride variant_node
class T20_cli_cli_TrackCommand_ClearOverride variant_node
class T20_cli_cli_TrackCommand_NextTask variant_node
class T20_cli_cli_TrackCommand_TaskCounts variant_node
class T20_cli_cli_TrackCommand_TypeGraph variant_node
class T20_cli_cli_TrackCommand_BaselineGraph variant_node
class T20_cli_cli_TrackCommand_ContractMap variant_node
class T20_cli_cli_TrackCommand_SpecElementHash variant_node
class T20_cli_cli_TrackCommand_BaselineCapture variant_node
class T20_cli_cli_TrackCommand_FixpointResolve variant_node
class T20_cli_cli_TrackCommand_SetCommitHash variant_node
class T20_cli_cli_TrackCommand_Lint variant_node
class T20_cli_cli_TrackCommand_CatalogueImplSignals variant_node
class T20_cli_cli_TrackCommand__self dto
class T25_cli_cli_TriggerReviewArgs__self dto
class T19_cli_cli_UnstageArgs__self dto
class T24_cli_cli_VerdictFilterArg_All variant_node
class T24_cli_cli_VerdictFilterArg_NotAViolation variant_node
class T24_cli_cli_VerdictFilterArg_Accepted variant_node
class T24_cli_cli_VerdictFilterArg_Violation variant_node
class T24_cli_cli_VerdictFilterArg__self dto
class T18_cli_cli_VerifyArgs__self dto
class T21_cli_cli_VerifyCommand_TechStack variant_node
class T21_cli_cli_VerifyCommand_LatestTrack variant_node
class T21_cli_cli_VerifyCommand_ArchDocs variant_node
class T21_cli_cli_VerifyCommand_Layers variant_node
class T21_cli_cli_VerifyCommand_HooksPath variant_node
class T21_cli_cli_VerifyCommand_SpecAttribution variant_node
class T21_cli_cli_VerifyCommand_SpecFrontmatter variant_node
class T21_cli_cli_VerifyCommand_CanonicalModules variant_node
class T21_cli_cli_VerifyCommand_ModuleSize variant_node
class T21_cli_cli_VerifyCommand_DomainPurity variant_node
class T21_cli_cli_VerifyCommand_DomainStrings variant_node
class T21_cli_cli_VerifyCommand_UsecasePurity variant_node
class T21_cli_cli_VerifyCommand_DocLinks variant_node
class T21_cli_cli_VerifyCommand_ViewFreshness variant_node
class T21_cli_cli_VerifyCommand_SpecSignals variant_node
class T21_cli_cli_VerifyCommand_PlanArtifactRefs variant_node
class T21_cli_cli_VerifyCommand_CatalogueSpecRefs variant_node
class T21_cli_cli_VerifyCommand__self dto
class T18_cli_cli_ViewAction_Validate variant_node
class T18_cli_cli_ViewAction_Sync variant_node
class T18_cli_cli_ViewAction__self dto
class T24_cli_cli_WaitAndMergeArgs__self dto
class F62_cli_cli_cli__commands__plan__codex_local__plan_input_from_args free_function
class F62_cli_cli_cli__commands__plan__codex_local__plan_input_from_args function_node
class F65_cli_cli_cli__commands__plan__codex_local__run_execute_codex_local free_function
class F65_cli_cli_cli__commands__plan__codex_local__run_execute_codex_local function_node
class F66_cli_cli_cli__commands__review__codex_local__emit_outcome_output_to free_function
class F66_cli_cli_cli__commands__review__codex_local__emit_outcome_output_to function_node
class F66_cli_cli_cli__commands__review__codex_local__review_input_from_args free_function
class F66_cli_cli_cli__commands__review__codex_local__review_input_from_args function_node
class F67_cli_cli_cli__commands__review__codex_local__run_execute_codex_local free_function
class F67_cli_cli_cli__commands__review__codex_local__run_execute_codex_local function_node
class T16_cli_cli_CliError_Message variant_node
class T16_cli_cli_CliError_Io variant_node
class T16_cli_cli_CliError__self error_type
```
