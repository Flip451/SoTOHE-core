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
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_arch["usecase::arch"]
    direction TB
  subgraph T29_usecase_usecase_ArchPortError["arch::ArchPortError"]
    direction TB
    T29_usecase_usecase_ArchPortError__self[ArchPortError]
    T29_usecase_usecase_ArchPortError_Unavailable[Unavailable]
  end
  subgraph R24_usecase_usecase_ArchPort["arch::ArchPort"]
    direction TB
    R24_usecase_usecase_ArchPort__self[ArchPort]
    R24_usecase_usecase_ArchPort_render_tree([render_tree])
    R24_usecase_usecase_ArchPort_render_tree_full([render_tree_full])
    R24_usecase_usecase_ArchPort_render_members([render_members])
    R24_usecase_usecase_ArchPort_render_direct_checks([render_direct_checks])
  end
  end
  subgraph usecase_usecase_module_conventions["usecase::conventions"]
    direction TB
  subgraph T36_usecase_usecase_ConventionsPortError["conventions::ConventionsPortError"]
    direction TB
    T36_usecase_usecase_ConventionsPortError__self[ConventionsPortError]
    T36_usecase_usecase_ConventionsPortError_Unavailable[Unavailable]
  end
  subgraph T33_usecase_usecase_VerifyIndexResult["conventions::VerifyIndexResult"]
    direction TB
    T33_usecase_usecase_VerifyIndexResult__self[VerifyIndexResult]
  end
  subgraph R31_usecase_usecase_ConventionsPort["conventions::ConventionsPort"]
    direction TB
    R31_usecase_usecase_ConventionsPort__self[ConventionsPort]
    R31_usecase_usecase_ConventionsPort_add_convention([add_convention])
    R31_usecase_usecase_ConventionsPort_update_index([update_index])
    R31_usecase_usecase_ConventionsPort_verify_index([verify_index])
  end
  end
  subgraph usecase_usecase_module_d4_orchestration["usecase::d4_orchestration"]
    direction TB
  subgraph T36_usecase_usecase_D4OrchestrationError["d4_orchestration::D4OrchestrationError"]
    direction TB
    T36_usecase_usecase_D4OrchestrationError__self[D4OrchestrationError]
    T36_usecase_usecase_D4OrchestrationError_DiffFragment[DiffFragment]
    T36_usecase_usecase_D4OrchestrationError_DryGate[DryGate]
    T36_usecase_usecase_D4OrchestrationError_PrPolling[PrPolling]
  end
  end
  subgraph usecase_usecase_module_demo["usecase::demo"]
    direction TB
  subgraph T30_usecase_usecase_DemoInteractor["demo::DemoInteractor"]
    direction TB
    T30_usecase_usecase_DemoInteractor__self[DemoInteractor]
    T30_usecase_usecase_DemoInteractor_new([new])
  end
  subgraph T29_usecase_usecase_DemoPortError["demo::DemoPortError"]
    direction TB
    T29_usecase_usecase_DemoPortError__self[DemoPortError]
    T29_usecase_usecase_DemoPortError_Unavailable[Unavailable]
  end
  subgraph R24_usecase_usecase_DemoPort["demo::DemoPort"]
    direction TB
    R24_usecase_usecase_DemoPort__self[DemoPort]
    R24_usecase_usecase_DemoPort_run([run])
  end
  subgraph R27_usecase_usecase_DemoService["demo::DemoService"]
    direction TB
    R27_usecase_usecase_DemoService__self[DemoService]
    R27_usecase_usecase_DemoService_run([run])
  end
  end
  subgraph usecase_usecase_module_dry_check["usecase::dry_check"]
    direction TB
  subgraph T42_usecase_usecase_CodeFragmentExtractorError["dry_check::fragment_pipeline::CodeFragmentExtractorError"]
    direction TB
    T42_usecase_usecase_CodeFragmentExtractorError__self[CodeFragmentExtractorError]
    T42_usecase_usecase_CodeFragmentExtractorError_ExtractionFailed[ExtractionFailed]
  end
  subgraph T35_usecase_usecase_DryCheckSharedError["dry_check::shared::DryCheckSharedError"]
    direction TB
    T35_usecase_usecase_DryCheckSharedError__self[DryCheckSharedError]
    T35_usecase_usecase_DryCheckSharedError_InvalidContentHash[InvalidContentHash]
    T35_usecase_usecase_DryCheckSharedError_InvalidSourcePath[InvalidSourcePath]
  end
  subgraph T42_usecase_usecase_DryFragmentPipelineCommand["dry_check::fragment_pipeline::DryFragmentPipelineCommand"]
    direction TB
    T42_usecase_usecase_DryFragmentPipelineCommand__self[DryFragmentPipelineCommand]
  end
  subgraph T45_usecase_usecase_DryFragmentPipelineInteractor["dry_check::fragment_pipeline::DryFragmentPipelineInteractor"]
    direction TB
    T45_usecase_usecase_DryFragmentPipelineInteractor__self[DryFragmentPipelineInteractor]
    T45_usecase_usecase_DryFragmentPipelineInteractor_new([new])
  end
  subgraph T41_usecase_usecase_DryFragmentPipelineOutput["dry_check::fragment_pipeline::DryFragmentPipelineOutput"]
    direction TB
    T41_usecase_usecase_DryFragmentPipelineOutput__self[DryFragmentPipelineOutput]
  end
  subgraph R41_usecase_usecase_CodeFragmentExtractorPort["dry_check::fragment_pipeline::CodeFragmentExtractorPort"]
    direction TB
    R41_usecase_usecase_CodeFragmentExtractorPort__self[CodeFragmentExtractorPort]
    R41_usecase_usecase_CodeFragmentExtractorPort_extract([extract])
  end
  subgraph R33_usecase_usecase_DryCheckAgentPort["dry_check::ports::DryCheckAgentPort"]
    direction TB
    R33_usecase_usecase_DryCheckAgentPort__self[DryCheckAgentPort]
    R33_usecase_usecase_DryCheckAgentPort_judge([judge])
  end
  subgraph R39_usecase_usecase_DryCheckApprovalService["dry_check::services::DryCheckApprovalService"]
    direction TB
    R39_usecase_usecase_DryCheckApprovalService__self[DryCheckApprovalService]
    R39_usecase_usecase_DryCheckApprovalService_check_approved([check_approved])
  end
  subgraph R34_usecase_usecase_DryCheckDiffSource["dry_check::ports::DryCheckDiffSource"]
    direction TB
    R34_usecase_usecase_DryCheckDiffSource__self[DryCheckDiffSource]
    R34_usecase_usecase_DryCheckDiffSource_list_changed_hunks([list_changed_hunks])
  end
  subgraph R42_usecase_usecase_DryFragmentPipelineService["dry_check::fragment_pipeline::DryFragmentPipelineService"]
    direction TB
    R42_usecase_usecase_DryFragmentPipelineService__self[DryFragmentPipelineService]
    R42_usecase_usecase_DryFragmentPipelineService_derive_current_refs([derive_current_refs])
  end
  F59_usecase_usecase_usecase__dry_check__shared__fragment_ref_of[[fragment_ref_of]]
  end
  subgraph usecase_usecase_module_dry_driver["usecase::dry_driver"]
    direction TB
  subgraph T43_usecase_usecase_DryCheckApprovedDriverInput["dry_driver::DryCheckApprovedDriverInput"]
    direction TB
    T43_usecase_usecase_DryCheckApprovedDriverInput__self[DryCheckApprovedDriverInput]
  end
  subgraph T35_usecase_usecase_DryDriverInteractor["dry_driver::DryDriverInteractor"]
    direction TB
    T35_usecase_usecase_DryDriverInteractor__self[DryDriverInteractor]
    T35_usecase_usecase_DryDriverInteractor_new([new])
  end
  subgraph T32_usecase_usecase_DryDriverOutcome["dry_driver::DryDriverOutcome"]
    direction TB
    T32_usecase_usecase_DryDriverOutcome__self[DryDriverOutcome]
    T32_usecase_usecase_DryDriverOutcome_success([success])
    T32_usecase_usecase_DryDriverOutcome_failure([failure])
  end
  subgraph T38_usecase_usecase_DryFixLocalDriverInput["dry_driver::DryFixLocalDriverInput"]
    direction TB
    T38_usecase_usecase_DryFixLocalDriverInput__self[DryFixLocalDriverInput]
  end
  subgraph T37_usecase_usecase_DryResultsDriverInput["dry_driver::DryResultsDriverInput"]
    direction TB
    T37_usecase_usecase_DryResultsDriverInput__self[DryResultsDriverInput]
  end
  subgraph T35_usecase_usecase_DryWriteDriverInput["dry_driver::DryWriteDriverInput"]
    direction TB
    T35_usecase_usecase_DryWriteDriverInput__self[DryWriteDriverInput]
  end
  subgraph R29_usecase_usecase_DryDriverPort["dry_driver::DryDriverPort"]
    direction TB
    R29_usecase_usecase_DryDriverPort__self[DryDriverPort]
    R29_usecase_usecase_DryDriverPort_dry_write([dry_write])
    R29_usecase_usecase_DryDriverPort_dry_results([dry_results])
    R29_usecase_usecase_DryDriverPort_dry_check_approved([dry_check_approved])
    R29_usecase_usecase_DryDriverPort_dry_fix_local([dry_fix_local])
  end
  subgraph R32_usecase_usecase_DryDriverService["dry_driver::DryDriverService"]
    direction TB
    R32_usecase_usecase_DryDriverService__self[DryDriverService]
    R32_usecase_usecase_DryDriverService_dry_write([dry_write])
    R32_usecase_usecase_DryDriverService_dry_results([dry_results])
    R32_usecase_usecase_DryDriverService_dry_check_approved([dry_check_approved])
    R32_usecase_usecase_DryDriverService_dry_fix_local([dry_fix_local])
  end
  end
  subgraph usecase_usecase_module_export_schema["usecase::export_schema"]
    direction TB
  subgraph T35_usecase_usecase_ExportSchemaCommand["export_schema::ExportSchemaCommand"]
    direction TB
    T35_usecase_usecase_ExportSchemaCommand__self[ExportSchemaCommand]
  end
  subgraph T33_usecase_usecase_ExportSchemaError["export_schema::ExportSchemaError"]
    direction TB
    T33_usecase_usecase_ExportSchemaError__self[ExportSchemaError]
    T33_usecase_usecase_ExportSchemaError_ExportFailed[ExportFailed]
    T33_usecase_usecase_ExportSchemaError_SerializationFailed[SerializationFailed]
    T33_usecase_usecase_ExportSchemaError_FileWriteFailed[FileWriteFailed]
  end
  subgraph T38_usecase_usecase_ExportSchemaInteractor["export_schema::ExportSchemaInteractor"]
    direction TB
    T38_usecase_usecase_ExportSchemaInteractor__self[ExportSchemaInteractor]
    T38_usecase_usecase_ExportSchemaInteractor_new([new])
  end
  subgraph T35_usecase_usecase_SchemaExporterError["export_schema::SchemaExporterError"]
    direction TB
    T35_usecase_usecase_SchemaExporterError__self[SchemaExporterError]
    T35_usecase_usecase_SchemaExporterError_ExportFailed[ExportFailed]
  end
  subgraph R34_usecase_usecase_SchemaExporterPort["export_schema::SchemaExporterPort"]
    direction TB
    R34_usecase_usecase_SchemaExporterPort__self[SchemaExporterPort]
    R34_usecase_usecase_SchemaExporterPort_export_as_json([export_as_json])
  end
  end
  subgraph usecase_usecase_module_file["usecase::file"]
    direction TB
  subgraph T29_usecase_usecase_FilePortError["file::FilePortError"]
    direction TB
    T29_usecase_usecase_FilePortError__self[FilePortError]
    T29_usecase_usecase_FilePortError_Unavailable[Unavailable]
  end
  subgraph R29_usecase_usecase_FileWritePort["file::FileWritePort"]
    direction TB
    R29_usecase_usecase_FileWritePort__self[FileWritePort]
    R29_usecase_usecase_FileWritePort_write_atomic([write_atomic])
  end
  end
  subgraph usecase_usecase_module_fixpoint_resolve["usecase::fixpoint_resolve"]
    direction TB
  subgraph T37_usecase_usecase_DiffBaseResolverError["fixpoint_resolve::DiffBaseResolverError"]
    direction TB
    T37_usecase_usecase_DiffBaseResolverError__self[DiffBaseResolverError]
    T37_usecase_usecase_DiffBaseResolverError_Unavailable[Unavailable]
  end
  subgraph T34_usecase_usecase_DryCorpusMetaError["fixpoint_resolve::DryCorpusMetaError"]
    direction TB
    T34_usecase_usecase_DryCorpusMetaError__self[DryCorpusMetaError]
    T34_usecase_usecase_DryCorpusMetaError_Unavailable[Unavailable]
  end
  subgraph T38_usecase_usecase_FixpointDryGateCommand["fixpoint_resolve::FixpointDryGateCommand"]
    direction TB
    T38_usecase_usecase_FixpointDryGateCommand__self[FixpointDryGateCommand]
  end
  subgraph T41_usecase_usecase_FixpointDryGateInteractor["fixpoint_resolve::FixpointDryGateInteractor"]
    direction TB
    T41_usecase_usecase_FixpointDryGateInteractor__self[FixpointDryGateInteractor]
    T41_usecase_usecase_FixpointDryGateInteractor_new([new])
  end
  subgraph T37_usecase_usecase_FixpointDryGateOutput["fixpoint_resolve::FixpointDryGateOutput"]
    direction TB
    T37_usecase_usecase_FixpointDryGateOutput__self[FixpointDryGateOutput]
  end
  subgraph R36_usecase_usecase_DiffBaseResolverPort["fixpoint_resolve::DiffBaseResolverPort"]
    direction TB
    R36_usecase_usecase_DiffBaseResolverPort__self[DiffBaseResolverPort]
    R36_usecase_usecase_DiffBaseResolverPort_resolve_diff_base([resolve_diff_base])
  end
  subgraph R38_usecase_usecase_DryApprovalFactoryPort["fixpoint_resolve::DryApprovalFactoryPort"]
    direction TB
    R38_usecase_usecase_DryApprovalFactoryPort__self[DryApprovalFactoryPort]
    R38_usecase_usecase_DryApprovalFactoryPort_build_approval([build_approval])
  end
  subgraph R33_usecase_usecase_DryCorpusMetaPort["fixpoint_resolve::DryCorpusMetaPort"]
    direction TB
    R33_usecase_usecase_DryCorpusMetaPort__self[DryCorpusMetaPort]
    R33_usecase_usecase_DryCorpusMetaPort_resolve_corpus_meta([resolve_corpus_meta])
  end
  subgraph R38_usecase_usecase_FixpointDryGateService["fixpoint_resolve::FixpointDryGateService"]
    direction TB
    R38_usecase_usecase_FixpointDryGateService__self[FixpointDryGateService]
    R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate([resolve_dry_gate])
  end
  subgraph R38_usecase_usecase_RefVerifyGateStatePort["fixpoint_resolve::RefVerifyGateStatePort"]
    direction TB
    R38_usecase_usecase_RefVerifyGateStatePort__self[RefVerifyGateStatePort]
    R38_usecase_usecase_RefVerifyGateStatePort_ref_verify_status([ref_verify_status])
  end
  subgraph R35_usecase_usecase_ReviewGateStatePort["fixpoint_resolve::ReviewGateStatePort"]
    direction TB
    R35_usecase_usecase_ReviewGateStatePort__self[ReviewGateStatePort]
    R35_usecase_usecase_ReviewGateStatePort_review_status([review_status])
  end
  end
  subgraph usecase_usecase_module_git_workflow["usecase::git_workflow"]
    direction TB
  subgraph T32_usecase_usecase_GitWorkflowError["git_workflow::GitWorkflowError"]
    direction TB
    T32_usecase_usecase_GitWorkflowError__self[GitWorkflowError]
    T32_usecase_usecase_GitWorkflowError_Validation[Validation]
    T32_usecase_usecase_GitWorkflowError_NoBranch[NoBranch]
    T32_usecase_usecase_GitWorkflowError_DetachedHead[DetachedHead]
    T32_usecase_usecase_GitWorkflowError_BranchMismatch[BranchMismatch]
    T32_usecase_usecase_GitWorkflowError_Message[Message]
    T32_usecase_usecase_GitWorkflowError_Unavailable[Unavailable]
  end
  subgraph T37_usecase_usecase_GitWorkflowInteractor["git_workflow::GitWorkflowInteractor"]
    direction TB
    T37_usecase_usecase_GitWorkflowInteractor__self[GitWorkflowInteractor]
    T37_usecase_usecase_GitWorkflowInteractor_new([new])
  end
  subgraph R34_usecase_usecase_GitWorkflowService["git_workflow::GitWorkflowService"]
    direction TB
    R34_usecase_usecase_GitWorkflowService__self[GitWorkflowService]
    R34_usecase_usecase_GitWorkflowService_stage_all([stage_all])
    R34_usecase_usecase_GitWorkflowService_stage_from_file([stage_from_file])
    R34_usecase_usecase_GitWorkflowService_commit_from_file([commit_from_file])
    R34_usecase_usecase_GitWorkflowService_note_from_file([note_from_file])
    R34_usecase_usecase_GitWorkflowService_switch_and_pull([switch_and_pull])
    R34_usecase_usecase_GitWorkflowService_unstage([unstage])
    R34_usecase_usecase_GitWorkflowService_current_branch_track_id([current_branch_track_id])
  end
  end
  subgraph usecase_usecase_module_guard["usecase::guard"]
    direction TB
  subgraph T32_usecase_usecase_ShellParserError["guard::ShellParserError"]
    direction TB
    T32_usecase_usecase_ShellParserError__self[ShellParserError]
    T32_usecase_usecase_ShellParserError_ParseFailed[ParseFailed]
  end
  subgraph R31_usecase_usecase_ShellParserPort["guard::ShellParserPort"]
    direction TB
    R31_usecase_usecase_ShellParserPort__self[ShellParserPort]
    R31_usecase_usecase_ShellParserPort_split_shell([split_shell])
  end
  end
  subgraph usecase_usecase_module_hook_dispatch["usecase::hook_dispatch"]
    direction TB
  subgraph T35_usecase_usecase_HookDispatchCommand["hook_dispatch::HookDispatchCommand"]
    direction TB
    T35_usecase_usecase_HookDispatchCommand__self[HookDispatchCommand]
  end
  subgraph T33_usecase_usecase_HookDispatchError["hook_dispatch::HookDispatchError"]
    direction TB
    T33_usecase_usecase_HookDispatchError__self[HookDispatchError]
    T33_usecase_usecase_HookDispatchError_UnknownHookName[UnknownHookName]
    T33_usecase_usecase_HookDispatchError_HandlerFailed[HandlerFailed]
  end
  subgraph T38_usecase_usecase_HookDispatchInteractor["hook_dispatch::HookDispatchInteractor"]
    direction TB
    T38_usecase_usecase_HookDispatchInteractor__self[HookDispatchInteractor]
    T38_usecase_usecase_HookDispatchInteractor_new([new])
  end
  subgraph T35_usecase_usecase_HookVerdictDecision["hook_dispatch::HookVerdictDecision"]
    direction TB
    T35_usecase_usecase_HookVerdictDecision__self[HookVerdictDecision]
    T35_usecase_usecase_HookVerdictDecision_Allow[Allow]
    T35_usecase_usecase_HookVerdictDecision_Block[Block]
  end
  subgraph T33_usecase_usecase_HookVerdictOutput["hook_dispatch::HookVerdictOutput"]
    direction TB
    T33_usecase_usecase_HookVerdictOutput__self[HookVerdictOutput]
  end
  subgraph R35_usecase_usecase_HookDispatchService["hook_dispatch::HookDispatchService"]
    direction TB
    R35_usecase_usecase_HookDispatchService__self[HookDispatchService]
    R35_usecase_usecase_HookDispatchService_dispatch([dispatch])
    R35_usecase_usecase_HookDispatchService_check_skill_compliance([check_skill_compliance])
  end
  end
  subgraph usecase_usecase_module_pr["usecase::pr"]
    direction TB
  subgraph T35_usecase_usecase_PrCommandInteractor["pr::PrCommandInteractor"]
    direction TB
    T35_usecase_usecase_PrCommandInteractor__self[PrCommandInteractor]
    T35_usecase_usecase_PrCommandInteractor_new([new])
  end
  subgraph T31_usecase_usecase_PrCommandOutput["pr::PrCommandOutput"]
    direction TB
    T31_usecase_usecase_PrCommandOutput__self[PrCommandOutput]
    T31_usecase_usecase_PrCommandOutput_success([success])
    T31_usecase_usecase_PrCommandOutput_failure([failure])
    T31_usecase_usecase_PrCommandOutput_with_exit_code([with_exit_code])
  end
  subgraph R32_usecase_usecase_PrCommandService["pr::PrCommandService"]
    direction TB
    R32_usecase_usecase_PrCommandService__self[PrCommandService]
    R32_usecase_usecase_PrCommandService_push([push])
    R32_usecase_usecase_PrCommandService_ensure([ensure])
    R32_usecase_usecase_PrCommandService_status([status])
    R32_usecase_usecase_PrCommandService_wait_and_merge([wait_and_merge])
    R32_usecase_usecase_PrCommandService_trigger_review([trigger_review])
    R32_usecase_usecase_PrCommandService_poll_review([poll_review])
    R32_usecase_usecase_PrCommandService_review_cycle([review_cycle])
  end
  end
  subgraph usecase_usecase_module_pr_review_polling["usecase::pr_review_polling"]
    direction TB
  subgraph T28_usecase_usecase_PrGhApiError["pr_review_polling::PrGhApiError"]
    direction TB
    T28_usecase_usecase_PrGhApiError__self[PrGhApiError]
    T28_usecase_usecase_PrGhApiError_ApiFailure[ApiFailure]
  end
  subgraph T30_usecase_usecase_PrRepoNwoError["pr_review_polling::PrRepoNwoError"]
    direction TB
    T30_usecase_usecase_PrRepoNwoError__self[PrRepoNwoError]
    T30_usecase_usecase_PrRepoNwoError_Unavailable[Unavailable]
  end
  subgraph T38_usecase_usecase_PrReviewPollingCommand["pr_review_polling::PrReviewPollingCommand"]
    direction TB
    T38_usecase_usecase_PrReviewPollingCommand__self[PrReviewPollingCommand]
  end
  subgraph T41_usecase_usecase_PrReviewPollingInteractor["pr_review_polling::PrReviewPollingInteractor"]
    direction TB
    T41_usecase_usecase_PrReviewPollingInteractor__self[PrReviewPollingInteractor]
    T41_usecase_usecase_PrReviewPollingInteractor_new([new])
  end
  subgraph T37_usecase_usecase_PrReviewPollingOutput["pr_review_polling::PrReviewPollingOutput"]
    direction TB
    T37_usecase_usecase_PrReviewPollingOutput__self[PrReviewPollingOutput]
    T37_usecase_usecase_PrReviewPollingOutput_ReviewFound[ReviewFound]
    T37_usecase_usecase_PrReviewPollingOutput_ZeroFindings[ZeroFindings]
    T37_usecase_usecase_PrReviewPollingOutput_Timeout[Timeout]
  end
  subgraph R39_usecase_usecase_PrListIssueCommentsPort["pr_review_polling::PrListIssueCommentsPort"]
    direction TB
    R39_usecase_usecase_PrListIssueCommentsPort__self[PrListIssueCommentsPort]
    R39_usecase_usecase_PrListIssueCommentsPort_list_issue_comments([list_issue_comments])
  end
  subgraph R35_usecase_usecase_PrListReactionsPort["pr_review_polling::PrListReactionsPort"]
    direction TB
    R35_usecase_usecase_PrListReactionsPort__self[PrListReactionsPort]
    R35_usecase_usecase_PrListReactionsPort_list_reactions([list_reactions])
  end
  subgraph R33_usecase_usecase_PrListReviewsPort["pr_review_polling::PrListReviewsPort"]
    direction TB
    R33_usecase_usecase_PrListReviewsPort__self[PrListReviewsPort]
    R33_usecase_usecase_PrListReviewsPort_list_reviews([list_reviews])
  end
  subgraph R29_usecase_usecase_PrRepoNwoPort["pr_review_polling::PrRepoNwoPort"]
    direction TB
    R29_usecase_usecase_PrRepoNwoPort__self[PrRepoNwoPort]
    R29_usecase_usecase_PrRepoNwoPort_repo_nwo([repo_nwo])
  end
  subgraph R38_usecase_usecase_PrReviewPollingService["pr_review_polling::PrReviewPollingService"]
    direction TB
    R38_usecase_usecase_PrReviewPollingService__self[PrReviewPollingService]
    R38_usecase_usecase_PrReviewPollingService_poll([poll])
  end
  subgraph R25_usecase_usecase_SleepPort["pr_review_polling::SleepPort"]
    direction TB
    R25_usecase_usecase_SleepPort__self[SleepPort]
    R25_usecase_usecase_SleepPort_sleep([sleep])
  end
  end
  subgraph usecase_usecase_module_ref_verify["usecase::ref_verify"]
    direction TB
  subgraph T36_usecase_usecase_CheckApprovedOutcome["ref_verify::check_approved::CheckApprovedOutcome"]
    direction TB
    T36_usecase_usecase_CheckApprovedOutcome__self[CheckApprovedOutcome]
    T36_usecase_usecase_CheckApprovedOutcome_NoPairs[NoPairs]
    T36_usecase_usecase_CheckApprovedOutcome_AllApproved[AllApproved]
    T36_usecase_usecase_CheckApprovedOutcome_NotApproved[NotApproved]
  end
  subgraph T48_usecase_usecase_RefVerifyCheckApprovedInteractor["ref_verify::check_approved::RefVerifyCheckApprovedInteractor"]
    direction TB
    T48_usecase_usecase_RefVerifyCheckApprovedInteractor__self[RefVerifyCheckApprovedInteractor]
    T48_usecase_usecase_RefVerifyCheckApprovedInteractor_new([new])
  end
  subgraph T45_usecase_usecase_RefVerifyCheckApprovedOutcome["ref_verify::driver_service::RefVerifyCheckApprovedOutcome"]
    direction TB
    T45_usecase_usecase_RefVerifyCheckApprovedOutcome__self[RefVerifyCheckApprovedOutcome]
    T45_usecase_usecase_RefVerifyCheckApprovedOutcome_NoPairs[NoPairs]
    T45_usecase_usecase_RefVerifyCheckApprovedOutcome_AllApproved[AllApproved]
    T45_usecase_usecase_RefVerifyCheckApprovedOutcome_NotApproved[NotApproved]
  end
  subgraph T36_usecase_usecase_RefVerifyDriverError["ref_verify::driver_service::RefVerifyDriverError"]
    direction TB
    T36_usecase_usecase_RefVerifyDriverError__self[RefVerifyDriverError]
    T36_usecase_usecase_RefVerifyDriverError_Unavailable[Unavailable]
    T36_usecase_usecase_RefVerifyDriverError_Wiring[Wiring]
    T36_usecase_usecase_RefVerifyDriverError_Usecase[Usecase]
  end
  subgraph T35_usecase_usecase_RefVerifyRunOutcome["ref_verify::driver_service::RefVerifyRunOutcome"]
    direction TB
    T35_usecase_usecase_RefVerifyRunOutcome__self[RefVerifyRunOutcome]
    T35_usecase_usecase_RefVerifyRunOutcome_Passed[Passed]
    T35_usecase_usecase_RefVerifyRunOutcome_SemanticFailuresConfirmed[SemanticFailuresConfirmed]
    T35_usecase_usecase_RefVerifyRunOutcome_HumanEscalationRequired[HumanEscalationRequired]
  end
  subgraph R41_usecase_usecase_RefVerifyAggregateService["ref_verify::driver_service::RefVerifyAggregateService"]
    direction TB
    R41_usecase_usecase_RefVerifyAggregateService__self[RefVerifyAggregateService]
    R41_usecase_usecase_RefVerifyAggregateService_run([run])
    R41_usecase_usecase_RefVerifyAggregateService_check_approved([check_approved])
  end
  subgraph R51_usecase_usecase_RefVerifyCheckApprovedDriverService["ref_verify::driver_service::RefVerifyCheckApprovedDriverService"]
    direction TB
    R51_usecase_usecase_RefVerifyCheckApprovedDriverService__self[RefVerifyCheckApprovedDriverService]
    R51_usecase_usecase_RefVerifyCheckApprovedDriverService_check_approved([check_approved])
  end
  subgraph R45_usecase_usecase_RefVerifyCheckApprovedService["ref_verify::check_approved::RefVerifyCheckApprovedService"]
    direction TB
    R45_usecase_usecase_RefVerifyCheckApprovedService__self[RefVerifyCheckApprovedService]
    R45_usecase_usecase_RefVerifyCheckApprovedService_check_approved([check_approved])
  end
  subgraph R35_usecase_usecase_RefVerifyRunService["ref_verify::driver_service::RefVerifyRunService"]
    direction TB
    R35_usecase_usecase_RefVerifyRunService__self[RefVerifyRunService]
    R35_usecase_usecase_RefVerifyRunService_run([run])
  end
  end
  subgraph usecase_usecase_module_review_v2["usecase::review_v2"]
    direction TB
  subgraph T30_usecase_usecase_ReviewAuxError["review_v2::review_aux::ReviewAuxError"]
    direction TB
    T30_usecase_usecase_ReviewAuxError__self[ReviewAuxError]
    T30_usecase_usecase_ReviewAuxError_Failed[Failed]
  end
  subgraph T40_usecase_usecase_ReviewClassifyInteractor["review_v2::review_aux::ReviewClassifyInteractor"]
    direction TB
    T40_usecase_usecase_ReviewClassifyInteractor__self[ReviewClassifyInteractor]
    T40_usecase_usecase_ReviewClassifyInteractor_new([new])
  end
  subgraph T37_usecase_usecase_ReviewFilesInteractor["review_v2::review_aux::ReviewFilesInteractor"]
    direction TB
    T37_usecase_usecase_ReviewFilesInteractor__self[ReviewFilesInteractor]
    T37_usecase_usecase_ReviewFilesInteractor_new([new])
  end
  subgraph T43_usecase_usecase_ReviewGetBriefingInteractor["review_v2::review_aux::ReviewGetBriefingInteractor"]
    direction TB
    T43_usecase_usecase_ReviewGetBriefingInteractor__self[ReviewGetBriefingInteractor]
    T43_usecase_usecase_ReviewGetBriefingInteractor_new([new])
  end
  subgraph T39_usecase_usecase_ReviewResultsInteractor["review_v2::review_aux::ReviewResultsInteractor"]
    direction TB
    T39_usecase_usecase_ReviewResultsInteractor__self[ReviewResultsInteractor]
    T39_usecase_usecase_ReviewResultsInteractor_new([new])
  end
  subgraph T31_usecase_usecase_ReviewRoundType["review_v2::run_review::ReviewRoundType"]
    direction TB
    T31_usecase_usecase_ReviewRoundType__self[ReviewRoundType]
    T31_usecase_usecase_ReviewRoundType_Fast[Fast]
    T31_usecase_usecase_ReviewRoundType_Final[Final]
    T31_usecase_usecase_ReviewRoundType_parse([parse])
  end
  subgraph T36_usecase_usecase_ReviewRoundTypeError["review_v2::run_review::ReviewRoundTypeError"]
    direction TB
    T36_usecase_usecase_ReviewRoundTypeError__self[ReviewRoundTypeError]
    T36_usecase_usecase_ReviewRoundTypeError_InvalidValue[InvalidValue]
  end
  subgraph T33_usecase_usecase_ReviewRunFixInput["review_v2::aggregate_service::ReviewRunFixInput"]
    direction TB
    T33_usecase_usecase_ReviewRunFixInput__self[ReviewRunFixInput]
  end
  subgraph T30_usecase_usecase_ReviewRunInput["review_v2::aggregate_service::ReviewRunInput"]
    direction TB
    T30_usecase_usecase_ReviewRunInput__self[ReviewRunInput]
  end
  subgraph T40_usecase_usecase_ReviewRunLocalInteractor["review_v2::review_aux::ReviewRunLocalInteractor"]
    direction TB
    T40_usecase_usecase_ReviewRunLocalInteractor__self[ReviewRunLocalInteractor]
    T40_usecase_usecase_ReviewRunLocalInteractor_new([new])
  end
  subgraph T36_usecase_usecase_ReviewRunLocalOutput["review_v2::review_aux::ReviewRunLocalOutput"]
    direction TB
    T36_usecase_usecase_ReviewRunLocalOutput__self[ReviewRunLocalOutput]
  end
  subgraph T45_usecase_usecase_ReviewValidateScopeInteractor["review_v2::review_aux::ReviewValidateScopeInteractor"]
    direction TB
    T45_usecase_usecase_ReviewValidateScopeInteractor__self[ReviewValidateScopeInteractor]
    T45_usecase_usecase_ReviewValidateScopeInteractor_new([new])
  end
  subgraph T30_usecase_usecase_RunReviewError["review_v2::run_review::RunReviewError"]
    direction TB
    T30_usecase_usecase_RunReviewError__self[RunReviewError]
    T30_usecase_usecase_RunReviewError_InvalidTrackId[InvalidTrackId]
    T30_usecase_usecase_RunReviewError_InvalidGroupName[InvalidGroupName]
    T30_usecase_usecase_RunReviewError_CompositionFailed[CompositionFailed]
    T30_usecase_usecase_RunReviewError_ReviewerFailed[ReviewerFailed]
  end
  subgraph T33_usecase_usecase_RunReviewFixError["review_v2::run_review_fix::RunReviewFixError"]
    direction TB
    T33_usecase_usecase_RunReviewFixError__self[RunReviewFixError]
    T33_usecase_usecase_RunReviewFixError_InvalidScope[InvalidScope]
    T33_usecase_usecase_RunReviewFixError_InvalidTrackId[InvalidTrackId]
    T33_usecase_usecase_RunReviewFixError_InvalidRoundType[InvalidRoundType]
    T33_usecase_usecase_RunReviewFixError_SmokeTestFailed[SmokeTestFailed]
    T33_usecase_usecase_RunReviewFixError_FixRunnerFailed[FixRunnerFailed]
  end
  subgraph T34_usecase_usecase_RunReviewFixOutput["review_v2::run_review_fix::RunReviewFixOutput"]
    direction TB
    T34_usecase_usecase_RunReviewFixOutput__self[RunReviewFixOutput]
  end
  subgraph T31_usecase_usecase_RunReviewOutput["review_v2::run_review::RunReviewOutput"]
    direction TB
    T31_usecase_usecase_RunReviewOutput__self[RunReviewOutput]
  end
  subgraph R37_usecase_usecase_ReviewClassifyService["review_v2::review_aux::ReviewClassifyService"]
    direction TB
    R37_usecase_usecase_ReviewClassifyService__self[ReviewClassifyService]
    R37_usecase_usecase_ReviewClassifyService_classify([classify])
  end
  subgraph R34_usecase_usecase_ReviewFilesService["review_v2::review_aux::ReviewFilesService"]
    direction TB
    R34_usecase_usecase_ReviewFilesService__self[ReviewFilesService]
    R34_usecase_usecase_ReviewFilesService_files([files])
  end
  subgraph R40_usecase_usecase_ReviewGetBriefingService["review_v2::review_aux::ReviewGetBriefingService"]
    direction TB
    R40_usecase_usecase_ReviewGetBriefingService__self[ReviewGetBriefingService]
    R40_usecase_usecase_ReviewGetBriefingService_get_briefing([get_briefing])
  end
  subgraph R36_usecase_usecase_ReviewResultsService["review_v2::review_aux::ReviewResultsService"]
    direction TB
    R36_usecase_usecase_ReviewResultsService__self[ReviewResultsService]
    R36_usecase_usecase_ReviewResultsService_results([results])
  end
  subgraph R37_usecase_usecase_ReviewRunLocalService["review_v2::review_aux::ReviewRunLocalService"]
    direction TB
    R37_usecase_usecase_ReviewRunLocalService__self[ReviewRunLocalService]
    R37_usecase_usecase_ReviewRunLocalService_run_local([run_local])
  end
  subgraph R29_usecase_usecase_ReviewService["review_v2::aggregate_service::ReviewService"]
    direction TB
    R29_usecase_usecase_ReviewService__self[ReviewService]
    R29_usecase_usecase_ReviewService_run_codex([run_codex])
    R29_usecase_usecase_ReviewService_run_claude([run_claude])
    R29_usecase_usecase_ReviewService_run_local([run_local])
    R29_usecase_usecase_ReviewService_run_fix_local([run_fix_local])
    R29_usecase_usecase_ReviewService_check_approved([check_approved])
    R29_usecase_usecase_ReviewService_results([results])
    R29_usecase_usecase_ReviewService_classify([classify])
    R29_usecase_usecase_ReviewService_files([files])
    R29_usecase_usecase_ReviewService_validate_scope([validate_scope])
    R29_usecase_usecase_ReviewService_get_briefing([get_briefing])
    R29_usecase_usecase_ReviewService_persist_commit_hash([persist_commit_hash])
  end
  subgraph R42_usecase_usecase_ReviewValidateScopeService["review_v2::review_aux::ReviewValidateScopeService"]
    direction TB
    R42_usecase_usecase_ReviewValidateScopeService__self[ReviewValidateScopeService]
    R42_usecase_usecase_ReviewValidateScopeService_validate_scope([validate_scope])
  end
  end
  subgraph usecase_usecase_module_review_workflow["usecase::review_workflow"]
    direction TB
  subgraph T35_usecase_usecase_ReviewWorkflowError["review_workflow::ReviewWorkflowError"]
    direction TB
    T35_usecase_usecase_ReviewWorkflowError__self[ReviewWorkflowError]
    T35_usecase_usecase_ReviewWorkflowError_Serialize[Serialize]
    T35_usecase_usecase_ReviewWorkflowError_Validation[Validation]
  end
  end
  subgraph usecase_usecase_module_semantic_dup["usecase::semantic_dup"]
    direction TB
  subgraph R33_usecase_usecase_SemanticIndexPort["semantic_dup::ports::SemanticIndexPort"]
    direction TB
    R33_usecase_usecase_SemanticIndexPort__self[SemanticIndexPort]
    R33_usecase_usecase_SemanticIndexPort_insert([insert])
    R33_usecase_usecase_SemanticIndexPort_insert_batch([insert_batch])
    R33_usecase_usecase_SemanticIndexPort_delete_by_source_path([delete_by_source_path])
    R33_usecase_usecase_SemanticIndexPort_search([search])
  end
  end
  subgraph usecase_usecase_module_semantic_dup_driver["usecase::semantic_dup_driver"]
    direction TB
  subgraph T35_usecase_usecase_DupCheckDriverInput["semantic_dup_driver::DupCheckDriverInput"]
    direction TB
    T35_usecase_usecase_DupCheckDriverInput__self[DupCheckDriverInput]
  end
  subgraph T38_usecase_usecase_FindSimilarDriverInput["semantic_dup_driver::FindSimilarDriverInput"]
    direction TB
    T38_usecase_usecase_FindSimilarDriverInput__self[FindSimilarDriverInput]
  end
  subgraph T37_usecase_usecase_IndexBuildDriverInput["semantic_dup_driver::IndexBuildDriverInput"]
    direction TB
    T37_usecase_usecase_IndexBuildDriverInput__self[IndexBuildDriverInput]
  end
  subgraph T46_usecase_usecase_IndexMeasureQualityDriverInput["semantic_dup_driver::IndexMeasureQualityDriverInput"]
    direction TB
    T46_usecase_usecase_IndexMeasureQualityDriverInput__self[IndexMeasureQualityDriverInput]
  end
  subgraph T43_usecase_usecase_SemanticDupDriverInteractor["semantic_dup_driver::SemanticDupDriverInteractor"]
    direction TB
    T43_usecase_usecase_SemanticDupDriverInteractor__self[SemanticDupDriverInteractor]
    T43_usecase_usecase_SemanticDupDriverInteractor_new([new])
  end
  subgraph T40_usecase_usecase_SemanticDupDriverOutcome["semantic_dup_driver::SemanticDupDriverOutcome"]
    direction TB
    T40_usecase_usecase_SemanticDupDriverOutcome__self[SemanticDupDriverOutcome]
    T40_usecase_usecase_SemanticDupDriverOutcome_success([success])
    T40_usecase_usecase_SemanticDupDriverOutcome_failure([failure])
  end
  subgraph R37_usecase_usecase_SemanticDupDriverPort["semantic_dup_driver::SemanticDupDriverPort"]
    direction TB
    R37_usecase_usecase_SemanticDupDriverPort__self[SemanticDupDriverPort]
    R37_usecase_usecase_SemanticDupDriverPort_find_similar([find_similar])
    R37_usecase_usecase_SemanticDupDriverPort_index_build([index_build])
    R37_usecase_usecase_SemanticDupDriverPort_index_measure_quality([index_measure_quality])
    R37_usecase_usecase_SemanticDupDriverPort_dup_check([dup_check])
  end
  subgraph R40_usecase_usecase_SemanticDupDriverService["semantic_dup_driver::SemanticDupDriverService"]
    direction TB
    R40_usecase_usecase_SemanticDupDriverService__self[SemanticDupDriverService]
    R40_usecase_usecase_SemanticDupDriverService_find_similar([find_similar])
    R40_usecase_usecase_SemanticDupDriverService_index_build([index_build])
    R40_usecase_usecase_SemanticDupDriverService_index_measure_quality([index_measure_quality])
    R40_usecase_usecase_SemanticDupDriverService_dup_check([dup_check])
  end
  end
  subgraph usecase_usecase_module_signal_gate["usecase::signal_gate"]
    direction TB
  subgraph T32_usecase_usecase_ChainRunnerError["signal_gate::ChainRunnerError"]
    direction TB
    T32_usecase_usecase_ChainRunnerError__self[ChainRunnerError]
    T32_usecase_usecase_ChainRunnerError_ExecutionFailed[ExecutionFailed]
  end
  subgraph T33_usecase_usecase_SignalChainOutput["signal_gate::SignalChainOutput"]
    direction TB
    T33_usecase_usecase_SignalChainOutput__self[SignalChainOutput]
  end
  subgraph T33_usecase_usecase_SignalGateCommand["signal_gate::SignalGateCommand"]
    direction TB
    T33_usecase_usecase_SignalGateCommand__self[SignalGateCommand]
  end
  subgraph T31_usecase_usecase_SignalGateError["signal_gate::SignalGateError"]
    direction TB
    T31_usecase_usecase_SignalGateError__self[SignalGateError]
    T31_usecase_usecase_SignalGateError_ChainExecutionFailed[ChainExecutionFailed]
    T31_usecase_usecase_SignalGateError_InvalidTrackId[InvalidTrackId]
    T31_usecase_usecase_SignalGateError_StrictnessConfigLoad[StrictnessConfigLoad]
  end
  subgraph T36_usecase_usecase_SignalGateInteractor["signal_gate::SignalGateInteractor"]
    direction TB
    T36_usecase_usecase_SignalGateInteractor__self[SignalGateInteractor]
    T36_usecase_usecase_SignalGateInteractor_new([new])
  end
  subgraph T32_usecase_usecase_SignalGateOutput["signal_gate::SignalGateOutput"]
    direction TB
    T32_usecase_usecase_SignalGateOutput__self[SignalGateOutput]
  end
  subgraph R34_usecase_usecase_AdrChainRunnerPort["signal_gate::AdrChainRunnerPort"]
    direction TB
    R34_usecase_usecase_AdrChainRunnerPort__self[AdrChainRunnerPort]
    R34_usecase_usecase_AdrChainRunnerPort_run_adr_chain([run_adr_chain])
  end
  subgraph R36_usecase_usecase_LayerChainRunnerPort["signal_gate::LayerChainRunnerPort"]
    direction TB
    R36_usecase_usecase_LayerChainRunnerPort__self[LayerChainRunnerPort]
    R36_usecase_usecase_LayerChainRunnerPort_run_catalog_spec_chain([run_catalog_spec_chain])
    R36_usecase_usecase_LayerChainRunnerPort_run_impl_catalog_chain([run_impl_catalog_chain])
  end
  subgraph R33_usecase_usecase_SignalGateService["signal_gate::SignalGateService"]
    direction TB
    R33_usecase_usecase_SignalGateService__self[SignalGateService]
    R33_usecase_usecase_SignalGateService_run_gate([run_gate])
  end
  subgraph R38_usecase_usecase_SpecAdrChainRunnerPort["signal_gate::SpecAdrChainRunnerPort"]
    direction TB
    R38_usecase_usecase_SpecAdrChainRunnerPort__self[SpecAdrChainRunnerPort]
    R38_usecase_usecase_SpecAdrChainRunnerPort_run_spec_adr_chain([run_spec_adr_chain])
  end
  end
  subgraph usecase_usecase_module_signal_service["usecase::signal_service"]
    direction TB
  subgraph T35_usecase_usecase_SignalCommandOutput["signal_service::SignalCommandOutput"]
    direction TB
    T35_usecase_usecase_SignalCommandOutput__self[SignalCommandOutput]
    T35_usecase_usecase_SignalCommandOutput_success([success])
    T35_usecase_usecase_SignalCommandOutput_failure([failure])
  end
  subgraph T30_usecase_usecase_SignalGateName["signal_service::SignalGateName"]
    direction TB
    T30_usecase_usecase_SignalGateName__self[SignalGateName]
    T30_usecase_usecase_SignalGateName_Commit[Commit]
    T30_usecase_usecase_SignalGateName_Merge[Merge]
  end
  subgraph R29_usecase_usecase_SignalService["signal_service::SignalService"]
    direction TB
    R29_usecase_usecase_SignalService__self[SignalService]
    R29_usecase_usecase_SignalService_calc_adr_user([calc_adr_user])
    R29_usecase_usecase_SignalService_check_adr_user([check_adr_user])
    R29_usecase_usecase_SignalService_calc_spec_adr([calc_spec_adr])
    R29_usecase_usecase_SignalService_check_spec_adr([check_spec_adr])
    R29_usecase_usecase_SignalService_calc_catalog_spec([calc_catalog_spec])
    R29_usecase_usecase_SignalService_check_catalog_spec([check_catalog_spec])
    R29_usecase_usecase_SignalService_calc_impl_catalog([calc_impl_catalog])
    R29_usecase_usecase_SignalService_check_impl_catalog([check_impl_catalog])
    R29_usecase_usecase_SignalService_check_gate([check_gate])
  end
  end
  subgraph usecase_usecase_module_spec_adr_signal["usecase::spec_adr_signal"]
    direction TB
  subgraph T36_usecase_usecase_SpecAdrSignalCommand["spec_adr_signal::SpecAdrSignalCommand"]
    direction TB
    T36_usecase_usecase_SpecAdrSignalCommand__self[SpecAdrSignalCommand]
  end
  subgraph T34_usecase_usecase_SpecAdrSignalError["spec_adr_signal::SpecAdrSignalError"]
    direction TB
    T34_usecase_usecase_SpecAdrSignalError__self[SpecAdrSignalError]
    T34_usecase_usecase_SpecAdrSignalError_Read[Read]
    T34_usecase_usecase_SpecAdrSignalError_Decode[Decode]
    T34_usecase_usecase_SpecAdrSignalError_Encode[Encode]
    T34_usecase_usecase_SpecAdrSignalError_Write[Write]
  end
  subgraph T39_usecase_usecase_SpecAdrSignalInteractor["spec_adr_signal::SpecAdrSignalInteractor"]
    direction TB
    T39_usecase_usecase_SpecAdrSignalInteractor__self[SpecAdrSignalInteractor]
    T39_usecase_usecase_SpecAdrSignalInteractor_new([new])
  end
  subgraph T35_usecase_usecase_SpecAdrSignalOutput["spec_adr_signal::SpecAdrSignalOutput"]
    direction TB
    T35_usecase_usecase_SpecAdrSignalOutput__self[SpecAdrSignalOutput]
  end
  subgraph R36_usecase_usecase_SpecAdrSignalService["spec_adr_signal::SpecAdrSignalService"]
    direction TB
    R36_usecase_usecase_SpecAdrSignalService__self[SpecAdrSignalService]
    R36_usecase_usecase_SpecAdrSignalService_calc_and_persist([calc_and_persist])
  end
  subgraph R34_usecase_usecase_SpecFileWriterPort["spec_adr_signal::SpecFileWriterPort"]
    direction TB
    R34_usecase_usecase_SpecFileWriterPort__self[SpecFileWriterPort]
    R34_usecase_usecase_SpecFileWriterPort_read_spec_json([read_spec_json])
    R34_usecase_usecase_SpecFileWriterPort_write_spec_json([write_spec_json])
  end
  end
  subgraph usecase_usecase_module_telemetry["usecase::telemetry"]
    direction TB
  subgraph T45_usecase_usecase_ArchivedTrackTelemetryCommand["telemetry::ArchivedTrackTelemetryCommand"]
    direction TB
    T45_usecase_usecase_ArchivedTrackTelemetryCommand__self[ArchivedTrackTelemetryCommand]
  end
  subgraph T43_usecase_usecase_ArchivedTrackTelemetryError["telemetry::ArchivedTrackTelemetryError"]
    direction TB
    T43_usecase_usecase_ArchivedTrackTelemetryError__self[ArchivedTrackTelemetryError]
    T43_usecase_usecase_ArchivedTrackTelemetryError_EmitUnavailable[EmitUnavailable]
  end
  subgraph T48_usecase_usecase_ArchivedTrackTelemetryInteractor["telemetry::ArchivedTrackTelemetryInteractor"]
    direction TB
    T48_usecase_usecase_ArchivedTrackTelemetryInteractor__self[ArchivedTrackTelemetryInteractor]
    T48_usecase_usecase_ArchivedTrackTelemetryInteractor_new([new])
  end
  subgraph T46_usecase_usecase_TelemetryAggregateServiceError["telemetry::TelemetryAggregateServiceError"]
    direction TB
    T46_usecase_usecase_TelemetryAggregateServiceError__self[TelemetryAggregateServiceError]
    T46_usecase_usecase_TelemetryAggregateServiceError_ReportUnavailable[ReportUnavailable]
    T46_usecase_usecase_TelemetryAggregateServiceError_EmitUnavailable[EmitUnavailable]
  end
  subgraph T45_usecase_usecase_TelemetryEmitDynamicPortError["telemetry::TelemetryEmitDynamicPortError"]
    direction TB
    T45_usecase_usecase_TelemetryEmitDynamicPortError__self[TelemetryEmitDynamicPortError]
    T45_usecase_usecase_TelemetryEmitDynamicPortError_EmitUnavailable[EmitUnavailable]
  end
  subgraph T35_usecase_usecase_TelemetryErrorEntry["telemetry::TelemetryErrorEntry"]
    direction TB
    T35_usecase_usecase_TelemetryErrorEntry__self[TelemetryErrorEntry]
  end
  subgraph T39_usecase_usecase_TelemetryHookBlockEntry["telemetry::TelemetryHookBlockEntry"]
    direction TB
    T39_usecase_usecase_TelemetryHookBlockEntry__self[TelemetryHookBlockEntry]
  end
  subgraph T38_usecase_usecase_TelemetryPhaseDuration["telemetry::TelemetryPhaseDuration"]
    direction TB
    T38_usecase_usecase_TelemetryPhaseDuration__self[TelemetryPhaseDuration]
  end
  subgraph T36_usecase_usecase_TelemetryReportError["telemetry::TelemetryReportError"]
    direction TB
    T36_usecase_usecase_TelemetryReportError__self[TelemetryReportError]
    T36_usecase_usecase_TelemetryReportError_TrackNotFound[TrackNotFound]
    T36_usecase_usecase_TelemetryReportError_ReportUnavailable[ReportUnavailable]
  end
  subgraph T37_usecase_usecase_TelemetryReportOutput["telemetry::TelemetryReportOutput"]
    direction TB
    T37_usecase_usecase_TelemetryReportOutput__self[TelemetryReportOutput]
  end
  subgraph R42_usecase_usecase_ArchivedTrackTelemetryPort["telemetry::ArchivedTrackTelemetryPort"]
    direction TB
    R42_usecase_usecase_ArchivedTrackTelemetryPort__self[ArchivedTrackTelemetryPort]
    R42_usecase_usecase_ArchivedTrackTelemetryPort_emit([emit])
  end
  subgraph R45_usecase_usecase_ArchivedTrackTelemetryService["telemetry::ArchivedTrackTelemetryService"]
    direction TB
    R45_usecase_usecase_ArchivedTrackTelemetryService__self[ArchivedTrackTelemetryService]
    R45_usecase_usecase_ArchivedTrackTelemetryService_emit([emit])
  end
  subgraph R41_usecase_usecase_TelemetryAggregateService["telemetry::TelemetryAggregateService"]
    direction TB
    R41_usecase_usecase_TelemetryAggregateService__self[TelemetryAggregateService]
    R41_usecase_usecase_TelemetryAggregateService_report([report])
    R41_usecase_usecase_TelemetryAggregateService_emit_archived([emit_archived])
  end
  subgraph R40_usecase_usecase_TelemetryEmitDynamicPort["telemetry::TelemetryEmitDynamicPort"]
    direction TB
    R40_usecase_usecase_TelemetryEmitDynamicPort__self[TelemetryEmitDynamicPort]
    R40_usecase_usecase_TelemetryEmitDynamicPort_emit_archived([emit_archived])
  end
  subgraph R35_usecase_usecase_TelemetryReportPort["telemetry::TelemetryReportPort"]
    direction TB
    R35_usecase_usecase_TelemetryReportPort__self[TelemetryReportPort]
    R35_usecase_usecase_TelemetryReportPort_aggregate([aggregate])
  end
  end
  subgraph usecase_usecase_module_track_resolution["usecase::track_resolution"]
    direction TB
  subgraph R32_usecase_usecase_BranchReaderPort["track_resolution::BranchReaderPort"]
    direction TB
    R32_usecase_usecase_BranchReaderPort__self[BranchReaderPort]
    R32_usecase_usecase_BranchReaderPort_current_branch([current_branch])
  end
  end
  subgraph usecase_usecase_module_track_service["usecase::track_service"]
    direction TB
  subgraph T34_usecase_usecase_TrackCommandOutput["track_service::TrackCommandOutput"]
    direction TB
    T34_usecase_usecase_TrackCommandOutput__self[TrackCommandOutput]
    T34_usecase_usecase_TrackCommandOutput_success([success])
    T34_usecase_usecase_TrackCommandOutput_failure([failure])
  end
  subgraph R28_usecase_usecase_TrackService["track_service::TrackService"]
    direction TB
    R28_usecase_usecase_TrackService__self[TrackService]
    R28_usecase_usecase_TrackService_init([init])
    R28_usecase_usecase_TrackService_transition([transition])
    R28_usecase_usecase_TrackService_resolve([resolve])
    R28_usecase_usecase_TrackService_views_sync([views_sync])
    R28_usecase_usecase_TrackService_branch_create([branch_create])
    R28_usecase_usecase_TrackService_branch_switch([branch_switch])
    R28_usecase_usecase_TrackService_views_validate([views_validate])
    R28_usecase_usecase_TrackService_add_task([add_task])
    R28_usecase_usecase_TrackService_set_override([set_override])
    R28_usecase_usecase_TrackService_clear_override([clear_override])
    R28_usecase_usecase_TrackService_next_task([next_task])
    R28_usecase_usecase_TrackService_task_counts([task_counts])
    R28_usecase_usecase_TrackService_archive([archive])
    R28_usecase_usecase_TrackService_detect_active([detect_active])
  end
  end
  subgraph usecase_usecase_module_verify["usecase::verify"]
    direction TB
  subgraph T29_usecase_usecase_VerifyOutcome["verify::VerifyOutcome"]
    direction TB
    T29_usecase_usecase_VerifyOutcome__self[VerifyOutcome]
    T29_usecase_usecase_VerifyOutcome_success([success])
    T29_usecase_usecase_VerifyOutcome_failure([failure])
  end
  subgraph T31_usecase_usecase_VerifyPortError["verify::VerifyPortError"]
    direction TB
    T31_usecase_usecase_VerifyPortError__self[VerifyPortError]
    T31_usecase_usecase_VerifyPortError_Unavailable[Unavailable]
  end
  subgraph R26_usecase_usecase_VerifyPort["verify::VerifyPort"]
    direction TB
    R26_usecase_usecase_VerifyPort__self[VerifyPort]
    R26_usecase_usecase_VerifyPort_verify_tech_stack([verify_tech_stack])
    R26_usecase_usecase_VerifyPort_verify_latest_track([verify_latest_track])
    R26_usecase_usecase_VerifyPort_verify_arch_docs([verify_arch_docs])
    R26_usecase_usecase_VerifyPort_verify_layers([verify_layers])
    R26_usecase_usecase_VerifyPort_verify_hooks_path([verify_hooks_path])
    R26_usecase_usecase_VerifyPort_verify_spec_attribution([verify_spec_attribution])
    R26_usecase_usecase_VerifyPort_verify_spec_frontmatter([verify_spec_frontmatter])
    R26_usecase_usecase_VerifyPort_verify_canonical_modules([verify_canonical_modules])
    R26_usecase_usecase_VerifyPort_verify_module_size([verify_module_size])
    R26_usecase_usecase_VerifyPort_verify_domain_purity([verify_domain_purity])
    R26_usecase_usecase_VerifyPort_verify_domain_strings([verify_domain_strings])
    R26_usecase_usecase_VerifyPort_verify_usecase_purity([verify_usecase_purity])
    R26_usecase_usecase_VerifyPort_verify_doc_links([verify_doc_links])
    R26_usecase_usecase_VerifyPort_verify_view_freshness([verify_view_freshness])
    R26_usecase_usecase_VerifyPort_verify_spec_signals([verify_spec_signals])
    R26_usecase_usecase_VerifyPort_verify_plan_artifact_refs([verify_plan_artifact_refs])
    R26_usecase_usecase_VerifyPort_verify_catalogue_spec_refs([verify_catalogue_spec_refs])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_arch["infrastructure::arch"]
    direction TB
  subgraph T43_infrastructure_infrastructure_FsArchAdapter["arch::FsArchAdapter"]
    direction TB
    T43_infrastructure_infrastructure_FsArchAdapter__self[FsArchAdapter]
    T43_infrastructure_infrastructure_FsArchAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_conventions["infrastructure::conventions"]
    direction TB
  subgraph T50_infrastructure_infrastructure_FsConventionsAdapter["conventions::FsConventionsAdapter"]
    direction TB
    T50_infrastructure_infrastructure_FsConventionsAdapter__self[FsConventionsAdapter]
    T50_infrastructure_infrastructure_FsConventionsAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_demo["infrastructure::demo"]
    direction TB
  subgraph T42_infrastructure_infrastructure_DemoRunError["demo::DemoRunError"]
    direction TB
    T42_infrastructure_infrastructure_DemoRunError__self[DemoRunError]
    T42_infrastructure_infrastructure_DemoRunError_Unavailable[Unavailable]
  end
  subgraph T43_infrastructure_infrastructure_FsDemoAdapter["demo::FsDemoAdapter"]
    direction TB
    T43_infrastructure_infrastructure_FsDemoAdapter__self[FsDemoAdapter]
    T43_infrastructure_infrastructure_FsDemoAdapter_new([new])
  end
  F68_infrastructure_infrastructure_infrastructure__demo__run_example_demo[[run_example_demo]]
  end
  subgraph infrastructure_infrastructure_module_dry_check["infrastructure::dry_check"]
    direction TB
  subgraph T52_infrastructure_infrastructure_FsDryCorpusMetaAdapter["dry_check::corpus_meta::FsDryCorpusMetaAdapter"]
    direction TB
    T52_infrastructure_infrastructure_FsDryCorpusMetaAdapter__self[FsDryCorpusMetaAdapter]
  end
  subgraph T51_infrastructure_infrastructure_GitDryCheckDiffGetter["dry_check::diff_getter::GitDryCheckDiffGetter"]
    direction TB
    T51_infrastructure_infrastructure_GitDryCheckDiffGetter__self[GitDryCheckDiffGetter]
  end
  subgraph T52_infrastructure_infrastructure_NoOpDryApprovalService["dry_check::noop_approval::NoOpDryApprovalService"]
    direction TB
    T52_infrastructure_infrastructure_NoOpDryApprovalService__self[NoOpDryApprovalService]
  end
  subgraph T47_infrastructure_infrastructure_RecordingDryAgent["dry_check::recording_agent::RecordingDryAgent"]
    direction TB
    T47_infrastructure_infrastructure_RecordingDryAgent__self[RecordingDryAgent]
    T47_infrastructure_infrastructure_RecordingDryAgent_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_file_port["infrastructure::file_port"]
    direction TB
  subgraph T48_infrastructure_infrastructure_FsFileWriteAdapter["file_port::FsFileWriteAdapter"]
    direction TB
    T48_infrastructure_infrastructure_FsFileWriteAdapter__self[FsFileWriteAdapter]
    T48_infrastructure_infrastructure_FsFileWriteAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_git_cli["infrastructure::git_cli"]
    direction TB
  subgraph T50_infrastructure_infrastructure_FsGitWorkflowAdapter["git_cli::workflow_adapter::FsGitWorkflowAdapter"]
    direction TB
    T50_infrastructure_infrastructure_FsGitWorkflowAdapter__self[FsGitWorkflowAdapter]
    T50_infrastructure_infrastructure_FsGitWorkflowAdapter_new([new])
  end
  subgraph T46_infrastructure_infrastructure_TrackBranchError["git_cli::TrackBranchError"]
    direction TB
    T46_infrastructure_infrastructure_TrackBranchError__self[TrackBranchError]
    T46_infrastructure_infrastructure_TrackBranchError_LoadFailed[LoadFailed]
    T46_infrastructure_infrastructure_TrackBranchError_contains([contains])
  end
  F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims[[collect_track_branch_claims]]
  F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch[[load_explicit_track_branch]]
  F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir[[load_explicit_track_branch_from_items_dir]]
  end
  subgraph infrastructure_infrastructure_module_pr_review["infrastructure::pr_review"]
    direction TB
  subgraph T48_infrastructure_infrastructure_SystemSleepAdapter["pr_review::SystemSleepAdapter"]
    direction TB
    T48_infrastructure_infrastructure_SystemSleepAdapter__self[SystemSleepAdapter]
  end
  end
  subgraph infrastructure_infrastructure_module_ref_verify["infrastructure::ref_verify"]
    direction TB
  subgraph T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter["ref_verify::driver_adapter::FsRefVerifyAggregateAdapter"]
    direction TB
    T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self[FsRefVerifyAggregateAdapter]
    T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter_new([new])
  end
  subgraph T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter["ref_verify::driver_adapter::FsRefVerifyCheckApprovedAdapter"]
    direction TB
    T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter__self[FsRefVerifyCheckApprovedAdapter]
    T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter_new([new])
  end
  subgraph T51_infrastructure_infrastructure_FsRefVerifyRunAdapter["ref_verify::driver_adapter::FsRefVerifyRunAdapter"]
    direction TB
    T51_infrastructure_infrastructure_FsRefVerifyRunAdapter__self[FsRefVerifyRunAdapter]
    T51_infrastructure_infrastructure_FsRefVerifyRunAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_semantic_dup["infrastructure::semantic_dup"]
    direction TB
  subgraph T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter["semantic_dup::fragment_extractor_adapter::CodeFragmentExtractorAdapter"]
    direction TB
    T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter__self[CodeFragmentExtractorAdapter]
    T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter_new([new])
  end
  subgraph T51_infrastructure_infrastructure_NoopSemanticIndexPort["semantic_dup::noop_adapter::NoopSemanticIndexPort"]
    direction TB
    T51_infrastructure_infrastructure_NoopSemanticIndexPort__self[NoopSemanticIndexPort]
  end
  subgraph T50_infrastructure_infrastructure_NullInsertIndexProxy["semantic_dup::null_insert_proxy::NullInsertIndexProxy"]
    direction TB
    T50_infrastructure_infrastructure_NullInsertIndexProxy__self[NullInsertIndexProxy]
    T50_infrastructure_infrastructure_NullInsertIndexProxy_new([new])
  end
  subgraph T54_infrastructure_infrastructure_PersistentIndexLockError["semantic_dup::null_insert_proxy::PersistentIndexLockError"]
    direction TB
    T54_infrastructure_infrastructure_PersistentIndexLockError__self[PersistentIndexLockError]
    T54_infrastructure_infrastructure_PersistentIndexLockError_LockFailed[LockFailed]
    T54_infrastructure_infrastructure_PersistentIndexLockError_contains([contains])
  end
  end
  subgraph infrastructure_infrastructure_module_spec["infrastructure::spec"]
    direction TB
  subgraph T53_infrastructure_infrastructure_FsSpecFileWriterAdapter["spec::writer::FsSpecFileWriterAdapter"]
    direction TB
    T53_infrastructure_infrastructure_FsSpecFileWriterAdapter__self[FsSpecFileWriterAdapter]
    T53_infrastructure_infrastructure_FsSpecFileWriterAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_telemetry["infrastructure::telemetry"]
    direction TB
  subgraph T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter["telemetry::archived_track::FsArchivedTrackTelemetryAdapter"]
    direction TB
    T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter__self[FsArchivedTrackTelemetryAdapter]
    T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter_new([new])
  end
  subgraph T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter["telemetry::report_adapter::FsTelemetryEmitDynamicAdapter"]
    direction TB
    T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter__self[FsTelemetryEmitDynamicAdapter]
    T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter_new([new])
  end
  subgraph T54_infrastructure_infrastructure_FsTelemetryReportAdapter["telemetry::report_adapter::FsTelemetryReportAdapter"]
    direction TB
    T54_infrastructure_infrastructure_FsTelemetryReportAdapter__self[FsTelemetryReportAdapter]
    T54_infrastructure_infrastructure_FsTelemetryReportAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_track["infrastructure::track"]
    direction TB
  subgraph T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter["track::gate_state::FsRefVerifyGateStateAdapter"]
    direction TB
    T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter__self[FsRefVerifyGateStateAdapter]
    T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter_new([new])
  end
  subgraph T54_infrastructure_infrastructure_FsReviewGateStateAdapter["track::gate_state::FsReviewGateStateAdapter"]
    direction TB
    T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self[FsReviewGateStateAdapter]
    T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_verify_adapter["infrastructure::verify_adapter"]
    direction TB
  subgraph T45_infrastructure_infrastructure_FsVerifyAdapter["verify_adapter::FsVerifyAdapter"]
    direction TB
    T45_infrastructure_infrastructure_FsVerifyAdapter__self[FsVerifyAdapter]
    T45_infrastructure_infrastructure_FsVerifyAdapter_new([new])
  end
  end
end
R24_usecase_usecase_ArchPort_render_tree --> T29_usecase_usecase_ArchPortError__self
R24_usecase_usecase_ArchPort_render_tree_full --> T29_usecase_usecase_ArchPortError__self
R24_usecase_usecase_ArchPort_render_members --> T29_usecase_usecase_ArchPortError__self
R24_usecase_usecase_ArchPort_render_direct_checks --> T29_usecase_usecase_ArchPortError__self
R31_usecase_usecase_ConventionsPort_add_convention --> T36_usecase_usecase_ConventionsPortError__self
R31_usecase_usecase_ConventionsPort_update_index --> T36_usecase_usecase_ConventionsPortError__self
R31_usecase_usecase_ConventionsPort_verify_index --> T36_usecase_usecase_ConventionsPortError__self
R31_usecase_usecase_ConventionsPort_verify_index --> T33_usecase_usecase_VerifyIndexResult__self
T30_usecase_usecase_DemoInteractor_new --> T30_usecase_usecase_DemoInteractor__self
R24_usecase_usecase_DemoPort_run --> T29_usecase_usecase_DemoPortError__self
R27_usecase_usecase_DemoService_run --> T29_usecase_usecase_DemoPortError__self
T45_usecase_usecase_DryFragmentPipelineInteractor_new --> T45_usecase_usecase_DryFragmentPipelineInteractor__self
R41_usecase_usecase_CodeFragmentExtractorPort_extract --> T42_usecase_usecase_CodeFragmentExtractorError__self
R42_usecase_usecase_DryFragmentPipelineService_derive_current_refs --o T42_usecase_usecase_DryFragmentPipelineCommand__self
R42_usecase_usecase_DryFragmentPipelineService_derive_current_refs --> T36_usecase_usecase_D4OrchestrationError__self
R42_usecase_usecase_DryFragmentPipelineService_derive_current_refs --> T41_usecase_usecase_DryFragmentPipelineOutput__self
F59_usecase_usecase_usecase__dry_check__shared__fragment_ref_of --> T35_usecase_usecase_DryCheckSharedError__self
T35_usecase_usecase_DryDriverInteractor_new --> T35_usecase_usecase_DryDriverInteractor__self
T32_usecase_usecase_DryDriverOutcome_success --> T32_usecase_usecase_DryDriverOutcome__self
T32_usecase_usecase_DryDriverOutcome_failure --> T32_usecase_usecase_DryDriverOutcome__self
R29_usecase_usecase_DryDriverPort_dry_write --o T35_usecase_usecase_DryWriteDriverInput__self
R29_usecase_usecase_DryDriverPort_dry_write --> T32_usecase_usecase_DryDriverOutcome__self
R29_usecase_usecase_DryDriverPort_dry_results --o T37_usecase_usecase_DryResultsDriverInput__self
R29_usecase_usecase_DryDriverPort_dry_results --> T32_usecase_usecase_DryDriverOutcome__self
R29_usecase_usecase_DryDriverPort_dry_check_approved --o T43_usecase_usecase_DryCheckApprovedDriverInput__self
R29_usecase_usecase_DryDriverPort_dry_check_approved --> T32_usecase_usecase_DryDriverOutcome__self
R29_usecase_usecase_DryDriverPort_dry_fix_local --o T38_usecase_usecase_DryFixLocalDriverInput__self
R29_usecase_usecase_DryDriverPort_dry_fix_local --> T32_usecase_usecase_DryDriverOutcome__self
R32_usecase_usecase_DryDriverService_dry_write --o T35_usecase_usecase_DryWriteDriverInput__self
R32_usecase_usecase_DryDriverService_dry_write --> T32_usecase_usecase_DryDriverOutcome__self
R32_usecase_usecase_DryDriverService_dry_results --o T37_usecase_usecase_DryResultsDriverInput__self
R32_usecase_usecase_DryDriverService_dry_results --> T32_usecase_usecase_DryDriverOutcome__self
R32_usecase_usecase_DryDriverService_dry_check_approved --o T43_usecase_usecase_DryCheckApprovedDriverInput__self
R32_usecase_usecase_DryDriverService_dry_check_approved --> T32_usecase_usecase_DryDriverOutcome__self
R32_usecase_usecase_DryDriverService_dry_fix_local --o T38_usecase_usecase_DryFixLocalDriverInput__self
R32_usecase_usecase_DryDriverService_dry_fix_local --> T32_usecase_usecase_DryDriverOutcome__self
T33_usecase_usecase_ExportSchemaError_FileWriteFailed --o T29_usecase_usecase_FilePortError__self
T38_usecase_usecase_ExportSchemaInteractor_new --> T38_usecase_usecase_ExportSchemaInteractor__self
R34_usecase_usecase_SchemaExporterPort_export_as_json --> T35_usecase_usecase_SchemaExporterError__self
R29_usecase_usecase_FileWritePort_write_atomic --> T29_usecase_usecase_FilePortError__self
T41_usecase_usecase_FixpointDryGateInteractor_new --> T41_usecase_usecase_FixpointDryGateInteractor__self
R36_usecase_usecase_DiffBaseResolverPort_resolve_diff_base --> T37_usecase_usecase_DiffBaseResolverError__self
R33_usecase_usecase_DryCorpusMetaPort_resolve_corpus_meta --> T34_usecase_usecase_DryCorpusMetaError__self
R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate --o T38_usecase_usecase_FixpointDryGateCommand__self
R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate --> T36_usecase_usecase_D4OrchestrationError__self
R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate --> T37_usecase_usecase_FixpointDryGateOutput__self
T37_usecase_usecase_GitWorkflowInteractor_new --> T37_usecase_usecase_GitWorkflowInteractor__self
R34_usecase_usecase_GitWorkflowService_stage_all --> T32_usecase_usecase_GitWorkflowError__self
R34_usecase_usecase_GitWorkflowService_stage_from_file --> T32_usecase_usecase_GitWorkflowError__self
R34_usecase_usecase_GitWorkflowService_commit_from_file --> T32_usecase_usecase_GitWorkflowError__self
R34_usecase_usecase_GitWorkflowService_note_from_file --> T32_usecase_usecase_GitWorkflowError__self
R34_usecase_usecase_GitWorkflowService_switch_and_pull --> T32_usecase_usecase_GitWorkflowError__self
R34_usecase_usecase_GitWorkflowService_unstage --> T32_usecase_usecase_GitWorkflowError__self
R34_usecase_usecase_GitWorkflowService_current_branch_track_id --> T32_usecase_usecase_GitWorkflowError__self
R31_usecase_usecase_ShellParserPort_split_shell --> T32_usecase_usecase_ShellParserError__self
T38_usecase_usecase_HookDispatchInteractor_new --> T38_usecase_usecase_HookDispatchInteractor__self
T33_usecase_usecase_HookVerdictOutput__self --o|decision| T35_usecase_usecase_HookVerdictDecision__self
R35_usecase_usecase_HookDispatchService_dispatch --o T35_usecase_usecase_HookDispatchCommand__self
R35_usecase_usecase_HookDispatchService_dispatch --> T33_usecase_usecase_HookDispatchError__self
R35_usecase_usecase_HookDispatchService_dispatch --> T33_usecase_usecase_HookVerdictOutput__self
T35_usecase_usecase_PrCommandInteractor_new --> T35_usecase_usecase_PrCommandInteractor__self
T31_usecase_usecase_PrCommandOutput_success --> T31_usecase_usecase_PrCommandOutput__self
T31_usecase_usecase_PrCommandOutput_failure --> T31_usecase_usecase_PrCommandOutput__self
T31_usecase_usecase_PrCommandOutput_with_exit_code --> T31_usecase_usecase_PrCommandOutput__self
R32_usecase_usecase_PrCommandService_push --> T31_usecase_usecase_PrCommandOutput__self
R32_usecase_usecase_PrCommandService_ensure --> T31_usecase_usecase_PrCommandOutput__self
R32_usecase_usecase_PrCommandService_status --> T31_usecase_usecase_PrCommandOutput__self
R32_usecase_usecase_PrCommandService_wait_and_merge --> T31_usecase_usecase_PrCommandOutput__self
R32_usecase_usecase_PrCommandService_trigger_review --> T31_usecase_usecase_PrCommandOutput__self
R32_usecase_usecase_PrCommandService_poll_review --> T31_usecase_usecase_PrCommandOutput__self
R32_usecase_usecase_PrCommandService_review_cycle --> T31_usecase_usecase_PrCommandOutput__self
T41_usecase_usecase_PrReviewPollingInteractor_new --> T41_usecase_usecase_PrReviewPollingInteractor__self
R39_usecase_usecase_PrListIssueCommentsPort_list_issue_comments --> T28_usecase_usecase_PrGhApiError__self
R35_usecase_usecase_PrListReactionsPort_list_reactions --> T28_usecase_usecase_PrGhApiError__self
R33_usecase_usecase_PrListReviewsPort_list_reviews --> T28_usecase_usecase_PrGhApiError__self
R29_usecase_usecase_PrRepoNwoPort_repo_nwo --> T30_usecase_usecase_PrRepoNwoError__self
R38_usecase_usecase_PrReviewPollingService_poll --o T38_usecase_usecase_PrReviewPollingCommand__self
R38_usecase_usecase_PrReviewPollingService_poll --> T36_usecase_usecase_D4OrchestrationError__self
R38_usecase_usecase_PrReviewPollingService_poll --> T37_usecase_usecase_PrReviewPollingOutput__self
T48_usecase_usecase_RefVerifyCheckApprovedInteractor_new --> T48_usecase_usecase_RefVerifyCheckApprovedInteractor__self
R41_usecase_usecase_RefVerifyAggregateService_run --> T36_usecase_usecase_RefVerifyDriverError__self
R41_usecase_usecase_RefVerifyAggregateService_run --> T35_usecase_usecase_RefVerifyRunOutcome__self
R41_usecase_usecase_RefVerifyAggregateService_check_approved --> T45_usecase_usecase_RefVerifyCheckApprovedOutcome__self
R41_usecase_usecase_RefVerifyAggregateService_check_approved --> T36_usecase_usecase_RefVerifyDriverError__self
R51_usecase_usecase_RefVerifyCheckApprovedDriverService_check_approved --> T45_usecase_usecase_RefVerifyCheckApprovedOutcome__self
R51_usecase_usecase_RefVerifyCheckApprovedDriverService_check_approved --> T36_usecase_usecase_RefVerifyDriverError__self
R45_usecase_usecase_RefVerifyCheckApprovedService_check_approved --> T36_usecase_usecase_CheckApprovedOutcome__self
R35_usecase_usecase_RefVerifyRunService_run --> T36_usecase_usecase_RefVerifyDriverError__self
R35_usecase_usecase_RefVerifyRunService_run --> T35_usecase_usecase_RefVerifyRunOutcome__self
T40_usecase_usecase_ReviewClassifyInteractor_new --> T40_usecase_usecase_ReviewClassifyInteractor__self
T37_usecase_usecase_ReviewFilesInteractor_new --> T37_usecase_usecase_ReviewFilesInteractor__self
T43_usecase_usecase_ReviewGetBriefingInteractor_new --> T43_usecase_usecase_ReviewGetBriefingInteractor__self
T39_usecase_usecase_ReviewResultsInteractor_new --> T39_usecase_usecase_ReviewResultsInteractor__self
T31_usecase_usecase_ReviewRoundType_parse --> T36_usecase_usecase_ReviewRoundTypeError__self
T31_usecase_usecase_ReviewRoundType_parse --> T31_usecase_usecase_ReviewRoundType__self
T40_usecase_usecase_ReviewRunLocalInteractor_new --> T40_usecase_usecase_ReviewRunLocalInteractor__self
T45_usecase_usecase_ReviewValidateScopeInteractor_new --> T45_usecase_usecase_ReviewValidateScopeInteractor__self
R37_usecase_usecase_ReviewClassifyService_classify --> T30_usecase_usecase_ReviewAuxError__self
R34_usecase_usecase_ReviewFilesService_files --> T30_usecase_usecase_ReviewAuxError__self
R40_usecase_usecase_ReviewGetBriefingService_get_briefing --> T30_usecase_usecase_ReviewAuxError__self
R36_usecase_usecase_ReviewResultsService_results --> T30_usecase_usecase_ReviewAuxError__self
R37_usecase_usecase_ReviewRunLocalService_run_local --> T36_usecase_usecase_ReviewRunLocalOutput__self
R29_usecase_usecase_ReviewService_run_codex --o T30_usecase_usecase_ReviewRunInput__self
R29_usecase_usecase_ReviewService_run_codex --> T30_usecase_usecase_RunReviewError__self
R29_usecase_usecase_ReviewService_run_codex --> T31_usecase_usecase_RunReviewOutput__self
R29_usecase_usecase_ReviewService_run_claude --o T30_usecase_usecase_ReviewRunInput__self
R29_usecase_usecase_ReviewService_run_claude --> T30_usecase_usecase_RunReviewError__self
R29_usecase_usecase_ReviewService_run_claude --> T31_usecase_usecase_RunReviewOutput__self
R29_usecase_usecase_ReviewService_run_local --> T36_usecase_usecase_ReviewRunLocalOutput__self
R29_usecase_usecase_ReviewService_run_fix_local --o T33_usecase_usecase_ReviewRunFixInput__self
R29_usecase_usecase_ReviewService_run_fix_local --> T33_usecase_usecase_RunReviewFixError__self
R29_usecase_usecase_ReviewService_run_fix_local --> T34_usecase_usecase_RunReviewFixOutput__self
R29_usecase_usecase_ReviewService_results --> T30_usecase_usecase_ReviewAuxError__self
R29_usecase_usecase_ReviewService_classify --> T30_usecase_usecase_ReviewAuxError__self
R29_usecase_usecase_ReviewService_files --> T30_usecase_usecase_ReviewAuxError__self
R29_usecase_usecase_ReviewService_validate_scope --> T30_usecase_usecase_ReviewAuxError__self
R29_usecase_usecase_ReviewService_get_briefing --> T30_usecase_usecase_ReviewAuxError__self
R42_usecase_usecase_ReviewValidateScopeService_validate_scope --> T30_usecase_usecase_ReviewAuxError__self
T43_usecase_usecase_SemanticDupDriverInteractor_new --> T43_usecase_usecase_SemanticDupDriverInteractor__self
T40_usecase_usecase_SemanticDupDriverOutcome_success --> T40_usecase_usecase_SemanticDupDriverOutcome__self
T40_usecase_usecase_SemanticDupDriverOutcome_failure --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R37_usecase_usecase_SemanticDupDriverPort_find_similar --o T38_usecase_usecase_FindSimilarDriverInput__self
R37_usecase_usecase_SemanticDupDriverPort_find_similar --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R37_usecase_usecase_SemanticDupDriverPort_index_build --o T37_usecase_usecase_IndexBuildDriverInput__self
R37_usecase_usecase_SemanticDupDriverPort_index_build --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R37_usecase_usecase_SemanticDupDriverPort_index_measure_quality --o T46_usecase_usecase_IndexMeasureQualityDriverInput__self
R37_usecase_usecase_SemanticDupDriverPort_index_measure_quality --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R37_usecase_usecase_SemanticDupDriverPort_dup_check --o T35_usecase_usecase_DupCheckDriverInput__self
R37_usecase_usecase_SemanticDupDriverPort_dup_check --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R40_usecase_usecase_SemanticDupDriverService_find_similar --o T38_usecase_usecase_FindSimilarDriverInput__self
R40_usecase_usecase_SemanticDupDriverService_find_similar --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R40_usecase_usecase_SemanticDupDriverService_index_build --o T37_usecase_usecase_IndexBuildDriverInput__self
R40_usecase_usecase_SemanticDupDriverService_index_build --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R40_usecase_usecase_SemanticDupDriverService_index_measure_quality --o T46_usecase_usecase_IndexMeasureQualityDriverInput__self
R40_usecase_usecase_SemanticDupDriverService_index_measure_quality --> T40_usecase_usecase_SemanticDupDriverOutcome__self
R40_usecase_usecase_SemanticDupDriverService_dup_check --o T35_usecase_usecase_DupCheckDriverInput__self
R40_usecase_usecase_SemanticDupDriverService_dup_check --> T40_usecase_usecase_SemanticDupDriverOutcome__self
T36_usecase_usecase_SignalGateInteractor_new --> T36_usecase_usecase_SignalGateInteractor__self
T32_usecase_usecase_SignalGateOutput__self --o|chain_outputs| T33_usecase_usecase_SignalChainOutput__self
R34_usecase_usecase_AdrChainRunnerPort_run_adr_chain --> T32_usecase_usecase_ChainRunnerError__self
R34_usecase_usecase_AdrChainRunnerPort_run_adr_chain --> T33_usecase_usecase_SignalChainOutput__self
R36_usecase_usecase_LayerChainRunnerPort_run_catalog_spec_chain --> T32_usecase_usecase_ChainRunnerError__self
R36_usecase_usecase_LayerChainRunnerPort_run_catalog_spec_chain --> T33_usecase_usecase_SignalChainOutput__self
R36_usecase_usecase_LayerChainRunnerPort_run_impl_catalog_chain --> T32_usecase_usecase_ChainRunnerError__self
R36_usecase_usecase_LayerChainRunnerPort_run_impl_catalog_chain --> T33_usecase_usecase_SignalChainOutput__self
R33_usecase_usecase_SignalGateService_run_gate --o T33_usecase_usecase_SignalGateCommand__self
R33_usecase_usecase_SignalGateService_run_gate --> T31_usecase_usecase_SignalGateError__self
R33_usecase_usecase_SignalGateService_run_gate --> T32_usecase_usecase_SignalGateOutput__self
R38_usecase_usecase_SpecAdrChainRunnerPort_run_spec_adr_chain --> T32_usecase_usecase_ChainRunnerError__self
R38_usecase_usecase_SpecAdrChainRunnerPort_run_spec_adr_chain --> T33_usecase_usecase_SignalChainOutput__self
T35_usecase_usecase_SignalCommandOutput_success --> T35_usecase_usecase_SignalCommandOutput__self
T35_usecase_usecase_SignalCommandOutput_failure --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_calc_adr_user --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_check_adr_user --o T30_usecase_usecase_SignalGateName__self
R29_usecase_usecase_SignalService_check_adr_user --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_calc_spec_adr --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_check_spec_adr --o T30_usecase_usecase_SignalGateName__self
R29_usecase_usecase_SignalService_check_spec_adr --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_calc_catalog_spec --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_check_catalog_spec --o T30_usecase_usecase_SignalGateName__self
R29_usecase_usecase_SignalService_check_catalog_spec --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_calc_impl_catalog --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_check_impl_catalog --o T30_usecase_usecase_SignalGateName__self
R29_usecase_usecase_SignalService_check_impl_catalog --> T35_usecase_usecase_SignalCommandOutput__self
R29_usecase_usecase_SignalService_check_gate --o T30_usecase_usecase_SignalGateName__self
R29_usecase_usecase_SignalService_check_gate --> T35_usecase_usecase_SignalCommandOutput__self
T39_usecase_usecase_SpecAdrSignalInteractor_new --> T39_usecase_usecase_SpecAdrSignalInteractor__self
R36_usecase_usecase_SpecAdrSignalService_calc_and_persist --o T36_usecase_usecase_SpecAdrSignalCommand__self
R36_usecase_usecase_SpecAdrSignalService_calc_and_persist --> T34_usecase_usecase_SpecAdrSignalError__self
R36_usecase_usecase_SpecAdrSignalService_calc_and_persist --> T35_usecase_usecase_SpecAdrSignalOutput__self
R34_usecase_usecase_SpecFileWriterPort_read_spec_json --> T34_usecase_usecase_SpecAdrSignalError__self
R34_usecase_usecase_SpecFileWriterPort_write_spec_json --> T34_usecase_usecase_SpecAdrSignalError__self
T48_usecase_usecase_ArchivedTrackTelemetryInteractor_new --> T48_usecase_usecase_ArchivedTrackTelemetryInteractor__self
T37_usecase_usecase_TelemetryReportOutput__self --o|phase_durations| T38_usecase_usecase_TelemetryPhaseDuration__self
T37_usecase_usecase_TelemetryReportOutput__self --o|errors| T35_usecase_usecase_TelemetryErrorEntry__self
T37_usecase_usecase_TelemetryReportOutput__self --o|hook_blocks| T39_usecase_usecase_TelemetryHookBlockEntry__self
R42_usecase_usecase_ArchivedTrackTelemetryPort_emit --> T43_usecase_usecase_ArchivedTrackTelemetryError__self
R45_usecase_usecase_ArchivedTrackTelemetryService_emit --o T45_usecase_usecase_ArchivedTrackTelemetryCommand__self
R45_usecase_usecase_ArchivedTrackTelemetryService_emit --> T43_usecase_usecase_ArchivedTrackTelemetryError__self
R41_usecase_usecase_TelemetryAggregateService_report --> T46_usecase_usecase_TelemetryAggregateServiceError__self
R41_usecase_usecase_TelemetryAggregateService_report --> T37_usecase_usecase_TelemetryReportOutput__self
R41_usecase_usecase_TelemetryAggregateService_emit_archived --> T46_usecase_usecase_TelemetryAggregateServiceError__self
R40_usecase_usecase_TelemetryEmitDynamicPort_emit_archived --> T45_usecase_usecase_TelemetryEmitDynamicPortError__self
R35_usecase_usecase_TelemetryReportPort_aggregate --> T36_usecase_usecase_TelemetryReportError__self
R35_usecase_usecase_TelemetryReportPort_aggregate --> T37_usecase_usecase_TelemetryReportOutput__self
T34_usecase_usecase_TrackCommandOutput_success --> T34_usecase_usecase_TrackCommandOutput__self
T34_usecase_usecase_TrackCommandOutput_failure --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_init --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_transition --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_resolve --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_views_sync --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_branch_create --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_branch_switch --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_views_validate --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_add_task --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_set_override --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_clear_override --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_next_task --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_task_counts --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_archive --> T34_usecase_usecase_TrackCommandOutput__self
R28_usecase_usecase_TrackService_detect_active --> T34_usecase_usecase_TrackCommandOutput__self
T29_usecase_usecase_VerifyOutcome_success --> T29_usecase_usecase_VerifyOutcome__self
T29_usecase_usecase_VerifyOutcome_failure --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_tech_stack --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_tech_stack --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_latest_track --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_latest_track --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_arch_docs --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_arch_docs --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_layers --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_layers --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_hooks_path --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_hooks_path --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_spec_attribution --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_spec_attribution --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_spec_frontmatter --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_spec_frontmatter --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_canonical_modules --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_canonical_modules --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_module_size --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_module_size --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_domain_purity --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_domain_purity --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_domain_strings --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_domain_strings --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_usecase_purity --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_usecase_purity --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_doc_links --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_doc_links --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_view_freshness --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_view_freshness --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_spec_signals --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_spec_signals --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_plan_artifact_refs --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_plan_artifact_refs --> T31_usecase_usecase_VerifyPortError__self
R26_usecase_usecase_VerifyPort_verify_catalogue_spec_refs --> T29_usecase_usecase_VerifyOutcome__self
R26_usecase_usecase_VerifyPort_verify_catalogue_spec_refs --> T31_usecase_usecase_VerifyPortError__self
T36_usecase_usecase_SignalGateInteractor__self -.impl.-> R33_usecase_usecase_SignalGateService__self
T39_usecase_usecase_SpecAdrSignalInteractor__self -.impl.-> R36_usecase_usecase_SpecAdrSignalService__self
T48_usecase_usecase_ArchivedTrackTelemetryInteractor__self -.impl.-> R45_usecase_usecase_ArchivedTrackTelemetryService__self
T45_usecase_usecase_DryFragmentPipelineInteractor__self -.impl.-> R42_usecase_usecase_DryFragmentPipelineService__self
T41_usecase_usecase_FixpointDryGateInteractor__self -.impl.-> R38_usecase_usecase_FixpointDryGateService__self
T41_usecase_usecase_PrReviewPollingInteractor__self -.impl.-> R38_usecase_usecase_PrReviewPollingService__self
T30_usecase_usecase_DemoInteractor__self -.impl.-> R27_usecase_usecase_DemoService__self
T35_usecase_usecase_DryDriverInteractor__self -.impl.-> R32_usecase_usecase_DryDriverService__self
T37_usecase_usecase_GitWorkflowInteractor__self -.impl.-> R34_usecase_usecase_GitWorkflowService__self
T35_usecase_usecase_PrCommandInteractor__self -.impl.-> R32_usecase_usecase_PrCommandService__self
T48_usecase_usecase_RefVerifyCheckApprovedInteractor__self -.impl.-> R45_usecase_usecase_RefVerifyCheckApprovedService__self
T40_usecase_usecase_ReviewClassifyInteractor__self -.impl.-> R37_usecase_usecase_ReviewClassifyService__self
T37_usecase_usecase_ReviewFilesInteractor__self -.impl.-> R34_usecase_usecase_ReviewFilesService__self
T43_usecase_usecase_ReviewGetBriefingInteractor__self -.impl.-> R40_usecase_usecase_ReviewGetBriefingService__self
T39_usecase_usecase_ReviewResultsInteractor__self -.impl.-> R36_usecase_usecase_ReviewResultsService__self
T40_usecase_usecase_ReviewRunLocalInteractor__self -.impl.-> R37_usecase_usecase_ReviewRunLocalService__self
T45_usecase_usecase_ReviewValidateScopeInteractor__self -.impl.-> R42_usecase_usecase_ReviewValidateScopeService__self
T43_usecase_usecase_SemanticDupDriverInteractor__self -.impl.-> R40_usecase_usecase_SemanticDupDriverService__self
T38_usecase_usecase_HookDispatchInteractor__self -.impl.-> R35_usecase_usecase_HookDispatchService__self
T43_infrastructure_infrastructure_FsArchAdapter_new --> T43_infrastructure_infrastructure_FsArchAdapter__self
T50_infrastructure_infrastructure_FsConventionsAdapter_new --> T50_infrastructure_infrastructure_FsConventionsAdapter__self
T43_infrastructure_infrastructure_FsDemoAdapter_new --> T43_infrastructure_infrastructure_FsDemoAdapter__self
F68_infrastructure_infrastructure_infrastructure__demo__run_example_demo --> T42_infrastructure_infrastructure_DemoRunError__self
T47_infrastructure_infrastructure_RecordingDryAgent_new --> T47_infrastructure_infrastructure_RecordingDryAgent__self
T48_infrastructure_infrastructure_FsFileWriteAdapter_new --> T48_infrastructure_infrastructure_FsFileWriteAdapter__self
T50_infrastructure_infrastructure_FsGitWorkflowAdapter_new --> T50_infrastructure_infrastructure_FsGitWorkflowAdapter__self
F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims --> T46_infrastructure_infrastructure_TrackBranchError__self
F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch --> T46_infrastructure_infrastructure_TrackBranchError__self
F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir --> T46_infrastructure_infrastructure_TrackBranchError__self
T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter_new --> T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self
T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter_new --> T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter__self
T51_infrastructure_infrastructure_FsRefVerifyRunAdapter_new --> T51_infrastructure_infrastructure_FsRefVerifyRunAdapter__self
T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter_new --> T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter__self
T50_infrastructure_infrastructure_NullInsertIndexProxy_new --> T50_infrastructure_infrastructure_NullInsertIndexProxy__self
T53_infrastructure_infrastructure_FsSpecFileWriterAdapter_new --> T53_infrastructure_infrastructure_FsSpecFileWriterAdapter__self
T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter_new --> T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter__self
T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter_new --> T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter__self
T54_infrastructure_infrastructure_FsTelemetryReportAdapter_new --> T54_infrastructure_infrastructure_FsTelemetryReportAdapter__self
T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter_new --> T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter__self
T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new --> T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self
T45_infrastructure_infrastructure_FsVerifyAdapter_new --> T45_infrastructure_infrastructure_FsVerifyAdapter__self
T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self -.impl.-> R35_usecase_usecase_ReviewGateStatePort__self
T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter__self -.impl.-> R38_usecase_usecase_RefVerifyGateStatePort__self
T47_infrastructure_infrastructure_RecordingDryAgent__self -.impl.-> R33_usecase_usecase_DryCheckAgentPort__self
T50_infrastructure_infrastructure_NullInsertIndexProxy__self -.impl.-> R33_usecase_usecase_SemanticIndexPort__self
T51_infrastructure_infrastructure_NoopSemanticIndexPort__self -.impl.-> R33_usecase_usecase_SemanticIndexPort__self
T52_infrastructure_infrastructure_NoOpDryApprovalService__self -.impl.-> R39_usecase_usecase_DryCheckApprovalService__self
T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter__self -.impl.-> R42_usecase_usecase_ArchivedTrackTelemetryPort__self
T53_infrastructure_infrastructure_FsSpecFileWriterAdapter__self -.impl.-> R34_usecase_usecase_SpecFileWriterPort__self
T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter__self -.impl.-> R41_usecase_usecase_CodeFragmentExtractorPort__self
T51_infrastructure_infrastructure_GitDryCheckDiffGetter__self -.impl.-> R34_usecase_usecase_DryCheckDiffSource__self
T52_infrastructure_infrastructure_FsDryCorpusMetaAdapter__self -.impl.-> R33_usecase_usecase_DryCorpusMetaPort__self
T48_infrastructure_infrastructure_SystemSleepAdapter__self -.impl.-> R25_usecase_usecase_SleepPort__self
T43_infrastructure_infrastructure_FsArchAdapter__self -.impl.-> R24_usecase_usecase_ArchPort__self
T50_infrastructure_infrastructure_FsConventionsAdapter__self -.impl.-> R31_usecase_usecase_ConventionsPort__self
T43_infrastructure_infrastructure_FsDemoAdapter__self -.impl.-> R24_usecase_usecase_DemoPort__self
T48_infrastructure_infrastructure_FsFileWriteAdapter__self -.impl.-> R29_usecase_usecase_FileWritePort__self
T50_infrastructure_infrastructure_FsGitWorkflowAdapter__self -.impl.-> R34_usecase_usecase_GitWorkflowService__self
T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter__self -.impl.-> R51_usecase_usecase_RefVerifyCheckApprovedDriverService__self
T51_infrastructure_infrastructure_FsRefVerifyRunAdapter__self -.impl.-> R35_usecase_usecase_RefVerifyRunService__self
T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter__self -.impl.-> R40_usecase_usecase_TelemetryEmitDynamicPort__self
T54_infrastructure_infrastructure_FsTelemetryReportAdapter__self -.impl.-> R35_usecase_usecase_TelemetryReportPort__self
T45_infrastructure_infrastructure_FsVerifyAdapter__self -.impl.-> R26_usecase_usecase_VerifyPort__self
T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self -.impl.-> R41_usecase_usecase_RefVerifyAggregateService__self
class T29_usecase_usecase_ArchPortError_Unavailable variant_node
class T29_usecase_usecase_ArchPortError__self error_type
class R24_usecase_usecase_ArchPort_render_tree method_node
class R24_usecase_usecase_ArchPort_render_tree_full method_node
class R24_usecase_usecase_ArchPort_render_members method_node
class R24_usecase_usecase_ArchPort_render_direct_checks method_node
class R24_usecase_usecase_ArchPort__self secondary_port
class T36_usecase_usecase_ConventionsPortError_Unavailable variant_node
class T36_usecase_usecase_ConventionsPortError__self error_type
class T33_usecase_usecase_VerifyIndexResult__self dto
class R31_usecase_usecase_ConventionsPort_add_convention method_node
class R31_usecase_usecase_ConventionsPort_update_index method_node
class R31_usecase_usecase_ConventionsPort_verify_index method_node
class R31_usecase_usecase_ConventionsPort__self secondary_port
class T36_usecase_usecase_D4OrchestrationError_DiffFragment variant_node
class T36_usecase_usecase_D4OrchestrationError_DryGate variant_node
class T36_usecase_usecase_D4OrchestrationError_PrPolling variant_node
class T36_usecase_usecase_D4OrchestrationError__self error_type
class T30_usecase_usecase_DemoInteractor_new method_node
class T30_usecase_usecase_DemoInteractor__self interactor
class T29_usecase_usecase_DemoPortError_Unavailable variant_node
class T29_usecase_usecase_DemoPortError__self error_type
class R24_usecase_usecase_DemoPort_run method_node
class R24_usecase_usecase_DemoPort__self secondary_port
class R27_usecase_usecase_DemoService_run method_node
class R27_usecase_usecase_DemoService__self app_service
class T42_usecase_usecase_CodeFragmentExtractorError_ExtractionFailed variant_node
class T42_usecase_usecase_CodeFragmentExtractorError__self error_type
class T35_usecase_usecase_DryCheckSharedError_InvalidContentHash variant_node
class T35_usecase_usecase_DryCheckSharedError_InvalidSourcePath variant_node
class T35_usecase_usecase_DryCheckSharedError__self error_type
class T42_usecase_usecase_DryFragmentPipelineCommand__self command
class T45_usecase_usecase_DryFragmentPipelineInteractor_new method_node
class T45_usecase_usecase_DryFragmentPipelineInteractor__self interactor
class T41_usecase_usecase_DryFragmentPipelineOutput__self dto
class R41_usecase_usecase_CodeFragmentExtractorPort_extract method_node
class R41_usecase_usecase_CodeFragmentExtractorPort__self secondary_port
class R33_usecase_usecase_DryCheckAgentPort_judge method_node
class R33_usecase_usecase_DryCheckAgentPort__self secondary_port
class R39_usecase_usecase_DryCheckApprovalService_check_approved method_node
class R39_usecase_usecase_DryCheckApprovalService__self secondary_port
class R34_usecase_usecase_DryCheckDiffSource_list_changed_hunks method_node
class R34_usecase_usecase_DryCheckDiffSource__self secondary_port
class R42_usecase_usecase_DryFragmentPipelineService_derive_current_refs method_node
class R42_usecase_usecase_DryFragmentPipelineService__self app_service
class F59_usecase_usecase_usecase__dry_check__shared__fragment_ref_of free_function
class F59_usecase_usecase_usecase__dry_check__shared__fragment_ref_of function_node
class T43_usecase_usecase_DryCheckApprovedDriverInput__self dto
class T35_usecase_usecase_DryDriverInteractor_new method_node
class T35_usecase_usecase_DryDriverInteractor__self interactor
class T32_usecase_usecase_DryDriverOutcome_success method_node
class T32_usecase_usecase_DryDriverOutcome_failure method_node
class T32_usecase_usecase_DryDriverOutcome__self dto
class T38_usecase_usecase_DryFixLocalDriverInput__self dto
class T37_usecase_usecase_DryResultsDriverInput__self dto
class T35_usecase_usecase_DryWriteDriverInput__self dto
class R29_usecase_usecase_DryDriverPort_dry_write method_node
class R29_usecase_usecase_DryDriverPort_dry_results method_node
class R29_usecase_usecase_DryDriverPort_dry_check_approved method_node
class R29_usecase_usecase_DryDriverPort_dry_fix_local method_node
class R29_usecase_usecase_DryDriverPort__self secondary_port
class R32_usecase_usecase_DryDriverService_dry_write method_node
class R32_usecase_usecase_DryDriverService_dry_results method_node
class R32_usecase_usecase_DryDriverService_dry_check_approved method_node
class R32_usecase_usecase_DryDriverService_dry_fix_local method_node
class R32_usecase_usecase_DryDriverService__self app_service
class T35_usecase_usecase_ExportSchemaCommand__self command
class T33_usecase_usecase_ExportSchemaError_ExportFailed variant_node
class T33_usecase_usecase_ExportSchemaError_SerializationFailed variant_node
class T33_usecase_usecase_ExportSchemaError_FileWriteFailed variant_node
class T33_usecase_usecase_ExportSchemaError__self error_type
class T38_usecase_usecase_ExportSchemaInteractor_new method_node
class T38_usecase_usecase_ExportSchemaInteractor__self interactor
class T35_usecase_usecase_SchemaExporterError_ExportFailed variant_node
class T35_usecase_usecase_SchemaExporterError__self error_type
class R34_usecase_usecase_SchemaExporterPort_export_as_json method_node
class R34_usecase_usecase_SchemaExporterPort__self secondary_port
class T29_usecase_usecase_FilePortError_Unavailable variant_node
class T29_usecase_usecase_FilePortError__self error_type
class R29_usecase_usecase_FileWritePort_write_atomic method_node
class R29_usecase_usecase_FileWritePort__self secondary_port
class T37_usecase_usecase_DiffBaseResolverError_Unavailable variant_node
class T37_usecase_usecase_DiffBaseResolverError__self error_type
class T34_usecase_usecase_DryCorpusMetaError_Unavailable variant_node
class T34_usecase_usecase_DryCorpusMetaError__self error_type
class T38_usecase_usecase_FixpointDryGateCommand__self command
class T41_usecase_usecase_FixpointDryGateInteractor_new method_node
class T41_usecase_usecase_FixpointDryGateInteractor__self interactor
class T37_usecase_usecase_FixpointDryGateOutput__self use_case
class R36_usecase_usecase_DiffBaseResolverPort_resolve_diff_base method_node
class R36_usecase_usecase_DiffBaseResolverPort__self secondary_port
class R38_usecase_usecase_DryApprovalFactoryPort_build_approval method_node
class R38_usecase_usecase_DryApprovalFactoryPort__self secondary_port
class R33_usecase_usecase_DryCorpusMetaPort_resolve_corpus_meta method_node
class R33_usecase_usecase_DryCorpusMetaPort__self secondary_port
class R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate method_node
class R38_usecase_usecase_FixpointDryGateService__self app_service
class R38_usecase_usecase_RefVerifyGateStatePort_ref_verify_status method_node
class R38_usecase_usecase_RefVerifyGateStatePort__self secondary_port
class R35_usecase_usecase_ReviewGateStatePort_review_status method_node
class R35_usecase_usecase_ReviewGateStatePort__self secondary_port
class T32_usecase_usecase_GitWorkflowError_Validation variant_node
class T32_usecase_usecase_GitWorkflowError_NoBranch variant_node
class T32_usecase_usecase_GitWorkflowError_DetachedHead variant_node
class T32_usecase_usecase_GitWorkflowError_BranchMismatch variant_node
class T32_usecase_usecase_GitWorkflowError_Message variant_node
class T32_usecase_usecase_GitWorkflowError_Unavailable variant_node
class T32_usecase_usecase_GitWorkflowError__self error_type
class T37_usecase_usecase_GitWorkflowInteractor_new method_node
class T37_usecase_usecase_GitWorkflowInteractor__self interactor
class R34_usecase_usecase_GitWorkflowService_stage_all method_node
class R34_usecase_usecase_GitWorkflowService_stage_from_file method_node
class R34_usecase_usecase_GitWorkflowService_commit_from_file method_node
class R34_usecase_usecase_GitWorkflowService_note_from_file method_node
class R34_usecase_usecase_GitWorkflowService_switch_and_pull method_node
class R34_usecase_usecase_GitWorkflowService_unstage method_node
class R34_usecase_usecase_GitWorkflowService_current_branch_track_id method_node
class R34_usecase_usecase_GitWorkflowService__self secondary_port
class T32_usecase_usecase_ShellParserError_ParseFailed variant_node
class T32_usecase_usecase_ShellParserError__self error_type
class R31_usecase_usecase_ShellParserPort_split_shell method_node
class R31_usecase_usecase_ShellParserPort__self secondary_port
class T35_usecase_usecase_HookDispatchCommand__self command
class T33_usecase_usecase_HookDispatchError_UnknownHookName variant_node
class T33_usecase_usecase_HookDispatchError_HandlerFailed variant_node
class T33_usecase_usecase_HookDispatchError__self error_type
class T38_usecase_usecase_HookDispatchInteractor_new method_node
class T38_usecase_usecase_HookDispatchInteractor__self interactor
class T35_usecase_usecase_HookVerdictDecision_Allow variant_node
class T35_usecase_usecase_HookVerdictDecision_Block variant_node
class T35_usecase_usecase_HookVerdictDecision__self dto
class T33_usecase_usecase_HookVerdictOutput__self dto
class R35_usecase_usecase_HookDispatchService_dispatch method_node
class R35_usecase_usecase_HookDispatchService_check_skill_compliance method_node
class R35_usecase_usecase_HookDispatchService__self app_service
class T35_usecase_usecase_PrCommandInteractor_new method_node
class T35_usecase_usecase_PrCommandInteractor__self interactor
class T31_usecase_usecase_PrCommandOutput_success method_node
class T31_usecase_usecase_PrCommandOutput_failure method_node
class T31_usecase_usecase_PrCommandOutput_with_exit_code method_node
class T31_usecase_usecase_PrCommandOutput__self dto
class R32_usecase_usecase_PrCommandService_push method_node
class R32_usecase_usecase_PrCommandService_ensure method_node
class R32_usecase_usecase_PrCommandService_status method_node
class R32_usecase_usecase_PrCommandService_wait_and_merge method_node
class R32_usecase_usecase_PrCommandService_trigger_review method_node
class R32_usecase_usecase_PrCommandService_poll_review method_node
class R32_usecase_usecase_PrCommandService_review_cycle method_node
class R32_usecase_usecase_PrCommandService__self app_service
class T28_usecase_usecase_PrGhApiError_ApiFailure variant_node
class T28_usecase_usecase_PrGhApiError__self error_type
class T30_usecase_usecase_PrRepoNwoError_Unavailable variant_node
class T30_usecase_usecase_PrRepoNwoError__self error_type
class T38_usecase_usecase_PrReviewPollingCommand__self command
class T41_usecase_usecase_PrReviewPollingInteractor_new method_node
class T41_usecase_usecase_PrReviewPollingInteractor__self interactor
class T37_usecase_usecase_PrReviewPollingOutput_ReviewFound variant_node
class T37_usecase_usecase_PrReviewPollingOutput_ZeroFindings variant_node
class T37_usecase_usecase_PrReviewPollingOutput_Timeout variant_node
class T37_usecase_usecase_PrReviewPollingOutput__self dto
class R39_usecase_usecase_PrListIssueCommentsPort_list_issue_comments method_node
class R39_usecase_usecase_PrListIssueCommentsPort__self secondary_port
class R35_usecase_usecase_PrListReactionsPort_list_reactions method_node
class R35_usecase_usecase_PrListReactionsPort__self secondary_port
class R33_usecase_usecase_PrListReviewsPort_list_reviews method_node
class R33_usecase_usecase_PrListReviewsPort__self secondary_port
class R29_usecase_usecase_PrRepoNwoPort_repo_nwo method_node
class R29_usecase_usecase_PrRepoNwoPort__self secondary_port
class R38_usecase_usecase_PrReviewPollingService_poll method_node
class R38_usecase_usecase_PrReviewPollingService__self app_service
class R25_usecase_usecase_SleepPort_sleep method_node
class R25_usecase_usecase_SleepPort__self secondary_port
class T36_usecase_usecase_CheckApprovedOutcome_NoPairs variant_node
class T36_usecase_usecase_CheckApprovedOutcome_AllApproved variant_node
class T36_usecase_usecase_CheckApprovedOutcome_NotApproved variant_node
class T36_usecase_usecase_CheckApprovedOutcome__self dto
class T48_usecase_usecase_RefVerifyCheckApprovedInteractor_new method_node
class T48_usecase_usecase_RefVerifyCheckApprovedInteractor__self interactor
class T45_usecase_usecase_RefVerifyCheckApprovedOutcome_NoPairs variant_node
class T45_usecase_usecase_RefVerifyCheckApprovedOutcome_AllApproved variant_node
class T45_usecase_usecase_RefVerifyCheckApprovedOutcome_NotApproved variant_node
class T45_usecase_usecase_RefVerifyCheckApprovedOutcome__self dto
class T36_usecase_usecase_RefVerifyDriverError_Unavailable variant_node
class T36_usecase_usecase_RefVerifyDriverError_Wiring variant_node
class T36_usecase_usecase_RefVerifyDriverError_Usecase variant_node
class T36_usecase_usecase_RefVerifyDriverError__self error_type
class T35_usecase_usecase_RefVerifyRunOutcome_Passed variant_node
class T35_usecase_usecase_RefVerifyRunOutcome_SemanticFailuresConfirmed variant_node
class T35_usecase_usecase_RefVerifyRunOutcome_HumanEscalationRequired variant_node
class T35_usecase_usecase_RefVerifyRunOutcome__self dto
class R41_usecase_usecase_RefVerifyAggregateService_run method_node
class R41_usecase_usecase_RefVerifyAggregateService_check_approved method_node
class R41_usecase_usecase_RefVerifyAggregateService__self secondary_port
class R51_usecase_usecase_RefVerifyCheckApprovedDriverService_check_approved method_node
class R51_usecase_usecase_RefVerifyCheckApprovedDriverService__self secondary_port
class R45_usecase_usecase_RefVerifyCheckApprovedService_check_approved method_node
class R45_usecase_usecase_RefVerifyCheckApprovedService__self app_service
class R35_usecase_usecase_RefVerifyRunService_run method_node
class R35_usecase_usecase_RefVerifyRunService__self secondary_port
class T30_usecase_usecase_ReviewAuxError_Failed variant_node
class T30_usecase_usecase_ReviewAuxError__self error_type
class T40_usecase_usecase_ReviewClassifyInteractor_new method_node
class T40_usecase_usecase_ReviewClassifyInteractor__self interactor
class T37_usecase_usecase_ReviewFilesInteractor_new method_node
class T37_usecase_usecase_ReviewFilesInteractor__self interactor
class T43_usecase_usecase_ReviewGetBriefingInteractor_new method_node
class T43_usecase_usecase_ReviewGetBriefingInteractor__self interactor
class T39_usecase_usecase_ReviewResultsInteractor_new method_node
class T39_usecase_usecase_ReviewResultsInteractor__self interactor
class T31_usecase_usecase_ReviewRoundType_Fast variant_node
class T31_usecase_usecase_ReviewRoundType_Final variant_node
class T31_usecase_usecase_ReviewRoundType_parse method_node
class T31_usecase_usecase_ReviewRoundType__self value_object
class T36_usecase_usecase_ReviewRoundTypeError_InvalidValue variant_node
class T36_usecase_usecase_ReviewRoundTypeError__self error_type
class T33_usecase_usecase_ReviewRunFixInput__self dto
class T30_usecase_usecase_ReviewRunInput__self dto
class T40_usecase_usecase_ReviewRunLocalInteractor_new method_node
class T40_usecase_usecase_ReviewRunLocalInteractor__self interactor
class T36_usecase_usecase_ReviewRunLocalOutput__self dto
class T45_usecase_usecase_ReviewValidateScopeInteractor_new method_node
class T45_usecase_usecase_ReviewValidateScopeInteractor__self interactor
class T30_usecase_usecase_RunReviewError_InvalidTrackId variant_node
class T30_usecase_usecase_RunReviewError_InvalidGroupName variant_node
class T30_usecase_usecase_RunReviewError_CompositionFailed variant_node
class T30_usecase_usecase_RunReviewError_ReviewerFailed variant_node
class T30_usecase_usecase_RunReviewError__self error_type
class T33_usecase_usecase_RunReviewFixError_InvalidScope variant_node
class T33_usecase_usecase_RunReviewFixError_InvalidTrackId variant_node
class T33_usecase_usecase_RunReviewFixError_InvalidRoundType variant_node
class T33_usecase_usecase_RunReviewFixError_SmokeTestFailed variant_node
class T33_usecase_usecase_RunReviewFixError_FixRunnerFailed variant_node
class T33_usecase_usecase_RunReviewFixError__self error_type
class T34_usecase_usecase_RunReviewFixOutput__self dto
class T31_usecase_usecase_RunReviewOutput__self dto
class R37_usecase_usecase_ReviewClassifyService_classify method_node
class R37_usecase_usecase_ReviewClassifyService__self app_service
class R34_usecase_usecase_ReviewFilesService_files method_node
class R34_usecase_usecase_ReviewFilesService__self app_service
class R40_usecase_usecase_ReviewGetBriefingService_get_briefing method_node
class R40_usecase_usecase_ReviewGetBriefingService__self app_service
class R36_usecase_usecase_ReviewResultsService_results method_node
class R36_usecase_usecase_ReviewResultsService__self app_service
class R37_usecase_usecase_ReviewRunLocalService_run_local method_node
class R37_usecase_usecase_ReviewRunLocalService__self app_service
class R29_usecase_usecase_ReviewService_run_codex method_node
class R29_usecase_usecase_ReviewService_run_claude method_node
class R29_usecase_usecase_ReviewService_run_local method_node
class R29_usecase_usecase_ReviewService_run_fix_local method_node
class R29_usecase_usecase_ReviewService_check_approved method_node
class R29_usecase_usecase_ReviewService_results method_node
class R29_usecase_usecase_ReviewService_classify method_node
class R29_usecase_usecase_ReviewService_files method_node
class R29_usecase_usecase_ReviewService_validate_scope method_node
class R29_usecase_usecase_ReviewService_get_briefing method_node
class R29_usecase_usecase_ReviewService_persist_commit_hash method_node
class R29_usecase_usecase_ReviewService__self app_service
class R42_usecase_usecase_ReviewValidateScopeService_validate_scope method_node
class R42_usecase_usecase_ReviewValidateScopeService__self app_service
class T35_usecase_usecase_ReviewWorkflowError_Serialize variant_node
class T35_usecase_usecase_ReviewWorkflowError_Validation variant_node
class T35_usecase_usecase_ReviewWorkflowError__self error_type
class R33_usecase_usecase_SemanticIndexPort_insert method_node
class R33_usecase_usecase_SemanticIndexPort_insert_batch method_node
class R33_usecase_usecase_SemanticIndexPort_delete_by_source_path method_node
class R33_usecase_usecase_SemanticIndexPort_search method_node
class R33_usecase_usecase_SemanticIndexPort__self secondary_port
class T35_usecase_usecase_DupCheckDriverInput__self dto
class T38_usecase_usecase_FindSimilarDriverInput__self dto
class T37_usecase_usecase_IndexBuildDriverInput__self dto
class T46_usecase_usecase_IndexMeasureQualityDriverInput__self dto
class T43_usecase_usecase_SemanticDupDriverInteractor_new method_node
class T43_usecase_usecase_SemanticDupDriverInteractor__self interactor
class T40_usecase_usecase_SemanticDupDriverOutcome_success method_node
class T40_usecase_usecase_SemanticDupDriverOutcome_failure method_node
class T40_usecase_usecase_SemanticDupDriverOutcome__self dto
class R37_usecase_usecase_SemanticDupDriverPort_find_similar method_node
class R37_usecase_usecase_SemanticDupDriverPort_index_build method_node
class R37_usecase_usecase_SemanticDupDriverPort_index_measure_quality method_node
class R37_usecase_usecase_SemanticDupDriverPort_dup_check method_node
class R37_usecase_usecase_SemanticDupDriverPort__self secondary_port
class R40_usecase_usecase_SemanticDupDriverService_find_similar method_node
class R40_usecase_usecase_SemanticDupDriverService_index_build method_node
class R40_usecase_usecase_SemanticDupDriverService_index_measure_quality method_node
class R40_usecase_usecase_SemanticDupDriverService_dup_check method_node
class R40_usecase_usecase_SemanticDupDriverService__self app_service
class T32_usecase_usecase_ChainRunnerError_ExecutionFailed variant_node
class T32_usecase_usecase_ChainRunnerError__self error_type
class T33_usecase_usecase_SignalChainOutput__self dto
class T33_usecase_usecase_SignalGateCommand__self command
class T31_usecase_usecase_SignalGateError_ChainExecutionFailed variant_node
class T31_usecase_usecase_SignalGateError_InvalidTrackId variant_node
class T31_usecase_usecase_SignalGateError_StrictnessConfigLoad variant_node
class T31_usecase_usecase_SignalGateError__self error_type
class T36_usecase_usecase_SignalGateInteractor_new method_node
class T36_usecase_usecase_SignalGateInteractor__self interactor
class T32_usecase_usecase_SignalGateOutput__self dto
class R34_usecase_usecase_AdrChainRunnerPort_run_adr_chain method_node
class R34_usecase_usecase_AdrChainRunnerPort__self secondary_port
class R36_usecase_usecase_LayerChainRunnerPort_run_catalog_spec_chain method_node
class R36_usecase_usecase_LayerChainRunnerPort_run_impl_catalog_chain method_node
class R36_usecase_usecase_LayerChainRunnerPort__self secondary_port
class R33_usecase_usecase_SignalGateService_run_gate method_node
class R33_usecase_usecase_SignalGateService__self app_service
class R38_usecase_usecase_SpecAdrChainRunnerPort_run_spec_adr_chain method_node
class R38_usecase_usecase_SpecAdrChainRunnerPort__self secondary_port
class T35_usecase_usecase_SignalCommandOutput_success method_node
class T35_usecase_usecase_SignalCommandOutput_failure method_node
class T35_usecase_usecase_SignalCommandOutput__self dto
class T30_usecase_usecase_SignalGateName_Commit variant_node
class T30_usecase_usecase_SignalGateName_Merge variant_node
class T30_usecase_usecase_SignalGateName__self dto
class R29_usecase_usecase_SignalService_calc_adr_user method_node
class R29_usecase_usecase_SignalService_check_adr_user method_node
class R29_usecase_usecase_SignalService_calc_spec_adr method_node
class R29_usecase_usecase_SignalService_check_spec_adr method_node
class R29_usecase_usecase_SignalService_calc_catalog_spec method_node
class R29_usecase_usecase_SignalService_check_catalog_spec method_node
class R29_usecase_usecase_SignalService_calc_impl_catalog method_node
class R29_usecase_usecase_SignalService_check_impl_catalog method_node
class R29_usecase_usecase_SignalService_check_gate method_node
class R29_usecase_usecase_SignalService__self app_service
class T36_usecase_usecase_SpecAdrSignalCommand__self command
class T34_usecase_usecase_SpecAdrSignalError_Read variant_node
class T34_usecase_usecase_SpecAdrSignalError_Decode variant_node
class T34_usecase_usecase_SpecAdrSignalError_Encode variant_node
class T34_usecase_usecase_SpecAdrSignalError_Write variant_node
class T34_usecase_usecase_SpecAdrSignalError__self error_type
class T39_usecase_usecase_SpecAdrSignalInteractor_new method_node
class T39_usecase_usecase_SpecAdrSignalInteractor__self interactor
class T35_usecase_usecase_SpecAdrSignalOutput__self dto
class R36_usecase_usecase_SpecAdrSignalService_calc_and_persist method_node
class R36_usecase_usecase_SpecAdrSignalService__self app_service
class R34_usecase_usecase_SpecFileWriterPort_read_spec_json method_node
class R34_usecase_usecase_SpecFileWriterPort_write_spec_json method_node
class R34_usecase_usecase_SpecFileWriterPort__self secondary_port
class T45_usecase_usecase_ArchivedTrackTelemetryCommand__self command
class T43_usecase_usecase_ArchivedTrackTelemetryError_EmitUnavailable variant_node
class T43_usecase_usecase_ArchivedTrackTelemetryError__self error_type
class T48_usecase_usecase_ArchivedTrackTelemetryInteractor_new method_node
class T48_usecase_usecase_ArchivedTrackTelemetryInteractor__self interactor
class T46_usecase_usecase_TelemetryAggregateServiceError_ReportUnavailable variant_node
class T46_usecase_usecase_TelemetryAggregateServiceError_EmitUnavailable variant_node
class T46_usecase_usecase_TelemetryAggregateServiceError__self error_type
class T45_usecase_usecase_TelemetryEmitDynamicPortError_EmitUnavailable variant_node
class T45_usecase_usecase_TelemetryEmitDynamicPortError__self error_type
class T35_usecase_usecase_TelemetryErrorEntry__self dto
class T39_usecase_usecase_TelemetryHookBlockEntry__self dto
class T38_usecase_usecase_TelemetryPhaseDuration__self dto
class T36_usecase_usecase_TelemetryReportError_TrackNotFound variant_node
class T36_usecase_usecase_TelemetryReportError_ReportUnavailable variant_node
class T36_usecase_usecase_TelemetryReportError__self error_type
class T37_usecase_usecase_TelemetryReportOutput__self dto
class R42_usecase_usecase_ArchivedTrackTelemetryPort_emit method_node
class R42_usecase_usecase_ArchivedTrackTelemetryPort__self secondary_port
class R45_usecase_usecase_ArchivedTrackTelemetryService_emit method_node
class R45_usecase_usecase_ArchivedTrackTelemetryService__self app_service
class R41_usecase_usecase_TelemetryAggregateService_report method_node
class R41_usecase_usecase_TelemetryAggregateService_emit_archived method_node
class R41_usecase_usecase_TelemetryAggregateService__self app_service
class R40_usecase_usecase_TelemetryEmitDynamicPort_emit_archived method_node
class R40_usecase_usecase_TelemetryEmitDynamicPort__self secondary_port
class R35_usecase_usecase_TelemetryReportPort_aggregate method_node
class R35_usecase_usecase_TelemetryReportPort__self secondary_port
class R32_usecase_usecase_BranchReaderPort_current_branch method_node
class R32_usecase_usecase_BranchReaderPort__self secondary_port
class T34_usecase_usecase_TrackCommandOutput_success method_node
class T34_usecase_usecase_TrackCommandOutput_failure method_node
class T34_usecase_usecase_TrackCommandOutput__self dto
class R28_usecase_usecase_TrackService_init method_node
class R28_usecase_usecase_TrackService_transition method_node
class R28_usecase_usecase_TrackService_resolve method_node
class R28_usecase_usecase_TrackService_views_sync method_node
class R28_usecase_usecase_TrackService_branch_create method_node
class R28_usecase_usecase_TrackService_branch_switch method_node
class R28_usecase_usecase_TrackService_views_validate method_node
class R28_usecase_usecase_TrackService_add_task method_node
class R28_usecase_usecase_TrackService_set_override method_node
class R28_usecase_usecase_TrackService_clear_override method_node
class R28_usecase_usecase_TrackService_next_task method_node
class R28_usecase_usecase_TrackService_task_counts method_node
class R28_usecase_usecase_TrackService_archive method_node
class R28_usecase_usecase_TrackService_detect_active method_node
class R28_usecase_usecase_TrackService__self app_service
class T29_usecase_usecase_VerifyOutcome_success method_node
class T29_usecase_usecase_VerifyOutcome_failure method_node
class T29_usecase_usecase_VerifyOutcome__self dto
class T31_usecase_usecase_VerifyPortError_Unavailable variant_node
class T31_usecase_usecase_VerifyPortError__self error_type
class R26_usecase_usecase_VerifyPort_verify_tech_stack method_node
class R26_usecase_usecase_VerifyPort_verify_latest_track method_node
class R26_usecase_usecase_VerifyPort_verify_arch_docs method_node
class R26_usecase_usecase_VerifyPort_verify_layers method_node
class R26_usecase_usecase_VerifyPort_verify_hooks_path method_node
class R26_usecase_usecase_VerifyPort_verify_spec_attribution method_node
class R26_usecase_usecase_VerifyPort_verify_spec_frontmatter method_node
class R26_usecase_usecase_VerifyPort_verify_canonical_modules method_node
class R26_usecase_usecase_VerifyPort_verify_module_size method_node
class R26_usecase_usecase_VerifyPort_verify_domain_purity method_node
class R26_usecase_usecase_VerifyPort_verify_domain_strings method_node
class R26_usecase_usecase_VerifyPort_verify_usecase_purity method_node
class R26_usecase_usecase_VerifyPort_verify_doc_links method_node
class R26_usecase_usecase_VerifyPort_verify_view_freshness method_node
class R26_usecase_usecase_VerifyPort_verify_spec_signals method_node
class R26_usecase_usecase_VerifyPort_verify_plan_artifact_refs method_node
class R26_usecase_usecase_VerifyPort_verify_catalogue_spec_refs method_node
class R26_usecase_usecase_VerifyPort__self secondary_port
class T43_infrastructure_infrastructure_FsArchAdapter_new method_node
class T43_infrastructure_infrastructure_FsArchAdapter__self secondary_adapter
class T50_infrastructure_infrastructure_FsConventionsAdapter_new method_node
class T50_infrastructure_infrastructure_FsConventionsAdapter__self secondary_adapter
class T42_infrastructure_infrastructure_DemoRunError_Unavailable variant_node
class T42_infrastructure_infrastructure_DemoRunError__self error_type
class T43_infrastructure_infrastructure_FsDemoAdapter_new method_node
class T43_infrastructure_infrastructure_FsDemoAdapter__self secondary_adapter
class F68_infrastructure_infrastructure_infrastructure__demo__run_example_demo free_function
class F68_infrastructure_infrastructure_infrastructure__demo__run_example_demo function_node
class T52_infrastructure_infrastructure_FsDryCorpusMetaAdapter__self secondary_adapter
class T51_infrastructure_infrastructure_GitDryCheckDiffGetter__self secondary_adapter
class T52_infrastructure_infrastructure_NoOpDryApprovalService__self secondary_adapter
class T47_infrastructure_infrastructure_RecordingDryAgent_new method_node
class T47_infrastructure_infrastructure_RecordingDryAgent__self secondary_adapter
class T48_infrastructure_infrastructure_FsFileWriteAdapter_new method_node
class T48_infrastructure_infrastructure_FsFileWriteAdapter__self secondary_adapter
class T50_infrastructure_infrastructure_FsGitWorkflowAdapter_new method_node
class T50_infrastructure_infrastructure_FsGitWorkflowAdapter__self secondary_adapter
class T46_infrastructure_infrastructure_TrackBranchError_LoadFailed variant_node
class T46_infrastructure_infrastructure_TrackBranchError_contains method_node
class T46_infrastructure_infrastructure_TrackBranchError__self error_type
class F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims free_function
class F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims function_node
class F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch free_function
class F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch function_node
class F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir free_function
class F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir function_node
class T48_infrastructure_infrastructure_SystemSleepAdapter__self secondary_adapter
class T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter_new method_node
class T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self secondary_adapter
class T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter_new method_node
class T61_infrastructure_infrastructure_FsRefVerifyCheckApprovedAdapter__self secondary_adapter
class T51_infrastructure_infrastructure_FsRefVerifyRunAdapter_new method_node
class T51_infrastructure_infrastructure_FsRefVerifyRunAdapter__self secondary_adapter
class T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter_new method_node
class T58_infrastructure_infrastructure_CodeFragmentExtractorAdapter__self secondary_adapter
class T51_infrastructure_infrastructure_NoopSemanticIndexPort__self secondary_adapter
class T50_infrastructure_infrastructure_NullInsertIndexProxy_new method_node
class T50_infrastructure_infrastructure_NullInsertIndexProxy__self secondary_adapter
class T54_infrastructure_infrastructure_PersistentIndexLockError_LockFailed variant_node
class T54_infrastructure_infrastructure_PersistentIndexLockError_contains method_node
class T54_infrastructure_infrastructure_PersistentIndexLockError__self error_type
class T53_infrastructure_infrastructure_FsSpecFileWriterAdapter_new method_node
class T53_infrastructure_infrastructure_FsSpecFileWriterAdapter__self secondary_adapter
class T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter_new method_node
class T61_infrastructure_infrastructure_FsArchivedTrackTelemetryAdapter__self secondary_adapter
class T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter_new method_node
class T59_infrastructure_infrastructure_FsTelemetryEmitDynamicAdapter__self secondary_adapter
class T54_infrastructure_infrastructure_FsTelemetryReportAdapter_new method_node
class T54_infrastructure_infrastructure_FsTelemetryReportAdapter__self secondary_adapter
class T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter_new method_node
class T57_infrastructure_infrastructure_FsRefVerifyGateStateAdapter__self secondary_adapter
class T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new method_node
class T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self secondary_adapter
class T45_infrastructure_infrastructure_FsVerifyAdapter_new method_node
class T45_infrastructure_infrastructure_FsVerifyAdapter__self secondary_adapter
```
