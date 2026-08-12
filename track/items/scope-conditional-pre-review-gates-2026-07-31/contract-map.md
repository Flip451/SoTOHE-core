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
classDef composition_root fill:#e0e7ff,stroke:#3730a3,stroke-width:2px
classDef domain_event fill:#fce7f3,stroke:#9d174d,stroke-width:1px
classDef domain_service fill:#fee2e2,stroke:#991b1b,stroke-width:1px
classDef dto fill:#f8fafc,stroke:#64748b,stroke-width:1px
classDef entity fill:#dbeafe,stroke:#1e40af,stroke-width:2px
classDef error_type fill:#fef2f2,stroke:#b91c1c,stroke-width:1px,stroke-dasharray:4 2
classDef event_policy fill:#fef3c7,stroke:#92400e,stroke-width:1px
classDef factory fill:#e0f2fe,stroke:#0369a1,stroke-width:1px
classDef free_function fill:#f5f3ff,stroke:#7c3aed,stroke-width:1px
classDef function_node fill:#f5f3ff,stroke:#a78bfa,stroke-width:1px
classDef interactor fill:#f0fdfa,stroke:#0d9488,stroke-width:1px
classDef method_node fill:#f8fafc,stroke:#cbd5e1,stroke-width:1px
classDef primary_adapter fill:#ecfccb,stroke:#3f6212,stroke-width:1px
classDef query fill:#f0f9ff,stroke:#0369a1,stroke-width:1px
classDef repository fill:#f5f3ff,stroke:#6d28d9,stroke-width:1px
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
  subgraph domain_domain_module_review_v2["domain::review_v2"]
    direction TB
  subgraph T27_domain_domain_MainScopeName["review_v2::types::MainScopeName"]
    direction TB
    T27_domain_domain_MainScopeName__self[MainScopeName]
    T27_domain_domain_MainScopeName_new([new])
    T27_domain_domain_MainScopeName_as_str([as_str])
  end
  subgraph T23_domain_domain_ScopeName["review_v2::types::ScopeName"]
    direction TB
    T23_domain_domain_ScopeName__self[ScopeName]
    T23_domain_domain_ScopeName_Main[Main]
    T23_domain_domain_ScopeName_Other[Other]
    T23_domain_domain_ScopeName_parse([parse])
  end
  subgraph T28_domain_domain_ScopeNameError["review_v2::error::ScopeNameError"]
    direction TB
    T28_domain_domain_ScopeNameError__self[ScopeNameError]
    T28_domain_domain_ScopeNameError_Empty[Empty]
    T28_domain_domain_ScopeNameError_NotAscii[NotAscii]
    T28_domain_domain_ScopeNameError_Reserved[Reserved]
  end
  F70_domain_domain_domain__review_v2__types__derive_review_approval_verdict[[derive_review_approval_verdict]]
  end
  subgraph domain_domain_module_task_contract["domain::task_contract"]
    direction TB
  subgraph T32_domain_domain_ContractedEntryRef["task_contract::ContractedEntryRef"]
    direction TB
    T32_domain_domain_ContractedEntryRef__self[ContractedEntryRef]
    T32_domain_domain_ContractedEntryRef_new([new])
    T32_domain_domain_ContractedEntryRef_layer([layer])
    T32_domain_domain_ContractedEntryRef_entry_key([entry_key])
  end
  subgraph T35_domain_domain_CoverageVerifyOutcome["task_contract::CoverageVerifyOutcome"]
    direction TB
    T35_domain_domain_CoverageVerifyOutcome__self[CoverageVerifyOutcome]
    T35_domain_domain_CoverageVerifyOutcome_Passed[Passed]
    T35_domain_domain_CoverageVerifyOutcome_Blocked[Blocked]
    T35_domain_domain_CoverageVerifyOutcome_blocked([blocked])
  end
  subgraph T31_domain_domain_CoverageViolation["task_contract::CoverageViolation"]
    direction TB
    T31_domain_domain_CoverageViolation__self[CoverageViolation]
    T31_domain_domain_CoverageViolation_MissingTaskContract[MissingTaskContract]
    T31_domain_domain_CoverageViolation_OrphanEntry[OrphanEntry]
    T31_domain_domain_CoverageViolation_InvalidEntryRef[InvalidEntryRef]
    T31_domain_domain_CoverageViolation_MissingSignalDocument[MissingSignalDocument]
    T31_domain_domain_CoverageViolation_InvalidTaskRef[InvalidTaskRef]
  end
  subgraph T34_domain_domain_PreReviewGateOutcome["task_contract::PreReviewGateOutcome"]
    direction TB
    T34_domain_domain_PreReviewGateOutcome__self[PreReviewGateOutcome]
    T34_domain_domain_PreReviewGateOutcome_Passed[Passed]
    T34_domain_domain_PreReviewGateOutcome_Blocked[Blocked]
    T34_domain_domain_PreReviewGateOutcome_blocked([blocked])
  end
  subgraph T36_domain_domain_PreReviewGateViolation["task_contract::PreReviewGateViolation"]
    direction TB
    T36_domain_domain_PreReviewGateViolation__self[PreReviewGateViolation]
    T36_domain_domain_PreReviewGateViolation_MissingTaskContract[MissingTaskContract]
    T36_domain_domain_PreReviewGateViolation_NonBlueSignal[NonBlueSignal]
  end
  subgraph T34_domain_domain_TaskContractDocument["task_contract::TaskContractDocument"]
    direction TB
    T34_domain_domain_TaskContractDocument__self[TaskContractDocument]
    T34_domain_domain_TaskContractDocument_new([new])
    T34_domain_domain_TaskContractDocument_schema_version([schema_version])
    T34_domain_domain_TaskContractDocument_track_id([track_id])
    T34_domain_domain_TaskContractDocument_entries([entries])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_capability_exec["usecase::capability_exec"]
    direction TB
  subgraph T40_usecase_usecase_CapabilityExecInteractor["capability_exec::CapabilityExecInteractor"]
    direction TB
    T40_usecase_usecase_CapabilityExecInteractor__self[CapabilityExecInteractor]
    T40_usecase_usecase_CapabilityExecInteractor_new([new])
  end
  subgraph T37_usecase_usecase_CapabilityExecRequest["capability_exec::CapabilityExecRequest"]
    direction TB
    T37_usecase_usecase_CapabilityExecRequest__self[CapabilityExecRequest]
  end
  subgraph R37_usecase_usecase_CapabilityExecService["capability_exec::CapabilityExecService"]
    direction TB
    R37_usecase_usecase_CapabilityExecService__self[CapabilityExecService]
    R37_usecase_usecase_CapabilityExecService_execute([execute])
  end
  end
  subgraph usecase_usecase_module_git_workflow["usecase::git_workflow"]
    direction TB
  subgraph T30_usecase_usecase_DiagnosticText["git_workflow::DiagnosticText"]
    direction TB
    T30_usecase_usecase_DiagnosticText__self[DiagnosticText]
    T30_usecase_usecase_DiagnosticText_new([new])
    T30_usecase_usecase_DiagnosticText_as_str([as_str])
  end
  end
  subgraph usecase_usecase_module_operator_command["usecase::operator_command"]
    direction TB
  subgraph T31_usecase_usecase_CommandArgument["operator_command::CommandArgument"]
    direction TB
    T31_usecase_usecase_CommandArgument__self[CommandArgument]
    T31_usecase_usecase_CommandArgument_try_new([try_new])
    T31_usecase_usecase_CommandArgument_as_str([as_str])
  end
  subgraph T27_usecase_usecase_CommandArgv["operator_command::CommandArgv"]
    direction TB
    T27_usecase_usecase_CommandArgv__self[CommandArgv]
    T27_usecase_usecase_CommandArgv_try_new([try_new])
    T27_usecase_usecase_CommandArgv_arguments([arguments])
    T27_usecase_usecase_CommandArgv_with_appended_arguments([with_appended_arguments])
  end
  subgraph T42_usecase_usecase_CommandArgvValidationError["operator_command::CommandArgvValidationError"]
    direction TB
    T42_usecase_usecase_CommandArgvValidationError__self[CommandArgvValidationError]
    T42_usecase_usecase_CommandArgvValidationError_Empty[Empty]
    T42_usecase_usecase_CommandArgvValidationError_RecursiveInvocation[RecursiveInvocation]
  end
  subgraph T38_usecase_usecase_CommandConfigLoadError["operator_command::CommandConfigLoadError"]
    direction TB
    T38_usecase_usecase_CommandConfigLoadError__self[CommandConfigLoadError]
    T38_usecase_usecase_CommandConfigLoadError_ReadFailed[ReadFailed]
    T38_usecase_usecase_CommandConfigLoadError_DecodeFailed[DecodeFailed]
    T38_usecase_usecase_CommandConfigLoadError_Invalid[Invalid]
  end
  subgraph T42_usecase_usecase_CommandConfigSchemaVersion["operator_command::CommandConfigSchemaVersion"]
    direction TB
    T42_usecase_usecase_CommandConfigSchemaVersion__self[CommandConfigSchemaVersion]
    T42_usecase_usecase_CommandConfigSchemaVersion_new([new])
    T42_usecase_usecase_CommandConfigSchemaVersion_as_u32([as_u32])
  end
  subgraph T44_usecase_usecase_CommandConfigValidationError["operator_command::CommandConfigValidationError"]
    direction TB
    T44_usecase_usecase_CommandConfigValidationError__self[CommandConfigValidationError]
    T44_usecase_usecase_CommandConfigValidationError_InvalidSchemaVersion[InvalidSchemaVersion]
    T44_usecase_usecase_CommandConfigValidationError_InvalidDeclarationId[InvalidDeclarationId]
    T44_usecase_usecase_CommandConfigValidationError_InvalidReviewScope[InvalidReviewScope]
    T44_usecase_usecase_CommandConfigValidationError_DuplicateDeclaration[DuplicateDeclaration]
    T44_usecase_usecase_CommandConfigValidationError_DuplicateScope[DuplicateScope]
    T44_usecase_usecase_CommandConfigValidationError_EmptyArgv[EmptyArgv]
    T44_usecase_usecase_CommandConfigValidationError_TimeoutOutOfRange[TimeoutOutOfRange]
    T44_usecase_usecase_CommandConfigValidationError_RecursiveInvocation[RecursiveInvocation]
    T44_usecase_usecase_CommandConfigValidationError_PersistedHostArgument[PersistedHostArgument]
  end
  subgraph T36_usecase_usecase_CommandDeclarationId["operator_command::CommandDeclarationId"]
    direction TB
    T36_usecase_usecase_CommandDeclarationId__self[CommandDeclarationId]
    T36_usecase_usecase_CommandDeclarationId_try_new([try_new])
    T36_usecase_usecase_CommandDeclarationId_as_str([as_str])
  end
  subgraph T51_usecase_usecase_CommandDeclarationIdValidationError["operator_command::CommandDeclarationIdValidationError"]
    direction TB
    T51_usecase_usecase_CommandDeclarationIdValidationError__self[CommandDeclarationIdValidationError]
    T51_usecase_usecase_CommandDeclarationIdValidationError_Empty[Empty]
  end
  subgraph T36_usecase_usecase_CommandSequenceIndex["operator_command::CommandSequenceIndex"]
    direction TB
    T36_usecase_usecase_CommandSequenceIndex__self[CommandSequenceIndex]
    T36_usecase_usecase_CommandSequenceIndex_new([new])
    T36_usecase_usecase_CommandSequenceIndex_as_usize([as_usize])
  end
  subgraph T37_usecase_usecase_CommandTimeoutSeconds["operator_command::CommandTimeoutSeconds"]
    direction TB
    T37_usecase_usecase_CommandTimeoutSeconds__self[CommandTimeoutSeconds]
    T37_usecase_usecase_CommandTimeoutSeconds_try_new([try_new])
    T37_usecase_usecase_CommandTimeoutSeconds_default_max([default_max])
    T37_usecase_usecase_CommandTimeoutSeconds_as_secs([as_secs])
  end
  subgraph T45_usecase_usecase_CommandTimeoutValidationError["operator_command::CommandTimeoutValidationError"]
    direction TB
    T45_usecase_usecase_CommandTimeoutValidationError__self[CommandTimeoutValidationError]
    T45_usecase_usecase_CommandTimeoutValidationError_OutOfRange[OutOfRange]
  end
  subgraph T33_usecase_usecase_ConfiguredCommand["operator_command::ConfiguredCommand"]
    direction TB
    T33_usecase_usecase_ConfiguredCommand__self[ConfiguredCommand]
    T33_usecase_usecase_ConfiguredCommand_try_new([try_new])
    T33_usecase_usecase_ConfiguredCommand_argv([argv])
    T33_usecase_usecase_ConfiguredCommand_timeout([timeout])
  end
  subgraph T48_usecase_usecase_ConfiguredCommandValidationError["operator_command::ConfiguredCommandValidationError"]
    direction TB
    T48_usecase_usecase_ConfiguredCommandValidationError__self[ConfiguredCommandValidationError]
    T48_usecase_usecase_ConfiguredCommandValidationError_Argv[Argv]
    T48_usecase_usecase_ConfiguredCommandValidationError_Timeout[Timeout]
    T48_usecase_usecase_ConfiguredCommandValidationError_PersistedHostArgument[PersistedHostArgument]
  end
  subgraph T39_usecase_usecase_OutputCaptureLimitBytes["operator_command::OutputCaptureLimitBytes"]
    direction TB
    T39_usecase_usecase_OutputCaptureLimitBytes__self[OutputCaptureLimitBytes]
    T39_usecase_usecase_OutputCaptureLimitBytes_one_mebibyte([one_mebibyte])
    T39_usecase_usecase_OutputCaptureLimitBytes_as_usize([as_usize])
  end
  subgraph T41_usecase_usecase_UnvalidatedTimeoutSeconds["operator_command::UnvalidatedTimeoutSeconds"]
    direction TB
    T41_usecase_usecase_UnvalidatedTimeoutSeconds__self[UnvalidatedTimeoutSeconds]
    T41_usecase_usecase_UnvalidatedTimeoutSeconds_new([new])
    T41_usecase_usecase_UnvalidatedTimeoutSeconds_as_u64([as_u64])
  end
  end
  subgraph usecase_usecase_module_phase_command["usecase::phase_command"]
    direction TB
  subgraph T34_usecase_usecase_PhaseCommandConfig["phase_command::PhaseCommandConfig"]
    direction TB
    T34_usecase_usecase_PhaseCommandConfig__self[PhaseCommandConfig]
    T34_usecase_usecase_PhaseCommandConfig_try_new([try_new])
    T34_usecase_usecase_PhaseCommandConfig_declaration([declaration])
  end
  subgraph T49_usecase_usecase_PhaseCommandConfigValidationError["phase_command::PhaseCommandConfigValidationError"]
    direction TB
    T49_usecase_usecase_PhaseCommandConfigValidationError__self[PhaseCommandConfigValidationError]
    T49_usecase_usecase_PhaseCommandConfigValidationError_InvalidSchemaVersion[InvalidSchemaVersion]
    T49_usecase_usecase_PhaseCommandConfigValidationError_DuplicateDeclaration[DuplicateDeclaration]
    T49_usecase_usecase_PhaseCommandConfigValidationError_into_command_config_validation_error([into_command_config_validation_error])
  end
  subgraph T39_usecase_usecase_PhaseCommandDeclaration["phase_command::PhaseCommandDeclaration"]
    direction TB
    T39_usecase_usecase_PhaseCommandDeclaration__self[PhaseCommandDeclaration]
    T39_usecase_usecase_PhaseCommandDeclaration_new([new])
    T39_usecase_usecase_PhaseCommandDeclaration_id([id])
    T39_usecase_usecase_PhaseCommandDeclaration_writer([writer])
    T39_usecase_usecase_PhaseCommandDeclaration_pre_entry_commands([pre_entry_commands])
  end
  subgraph T38_usecase_usecase_PhaseCommandEnterError["phase_command::PhaseCommandEnterError"]
    direction TB
    T38_usecase_usecase_PhaseCommandEnterError__self[PhaseCommandEnterError]
    T38_usecase_usecase_PhaseCommandEnterError_Config[Config]
    T38_usecase_usecase_PhaseCommandEnterError_UnknownPhase[UnknownPhase]
    T38_usecase_usecase_PhaseCommandEnterError_Runner[Runner]
  end
  subgraph T40_usecase_usecase_PhaseCommandEnterOutcome["phase_command::PhaseCommandEnterOutcome"]
    direction TB
    T40_usecase_usecase_PhaseCommandEnterOutcome__self[PhaseCommandEnterOutcome]
    T40_usecase_usecase_PhaseCommandEnterOutcome_Completed[Completed]
    T40_usecase_usecase_PhaseCommandEnterOutcome_Blocked[Blocked]
  end
  subgraph T40_usecase_usecase_PhaseCommandExplainError["phase_command::PhaseCommandExplainError"]
    direction TB
    T40_usecase_usecase_PhaseCommandExplainError__self[PhaseCommandExplainError]
    T40_usecase_usecase_PhaseCommandExplainError_Config[Config]
    T40_usecase_usecase_PhaseCommandExplainError_UnknownPhase[UnknownPhase]
  end
  subgraph T39_usecase_usecase_PhaseCommandExplanation["phase_command::PhaseCommandExplanation"]
    direction TB
    T39_usecase_usecase_PhaseCommandExplanation__self[PhaseCommandExplanation]
  end
  subgraph T38_usecase_usecase_PhaseCommandInteractor["phase_command::PhaseCommandInteractor"]
    direction TB
    T38_usecase_usecase_PhaseCommandInteractor__self[PhaseCommandInteractor]
    T38_usecase_usecase_PhaseCommandInteractor_new([new])
  end
  subgraph T33_usecase_usecase_PhaseEnterCommand["phase_command::PhaseEnterCommand"]
    direction TB
    T33_usecase_usecase_PhaseEnterCommand__self[PhaseEnterCommand]
  end
  subgraph T33_usecase_usecase_PhaseExplainQuery["phase_command::PhaseExplainQuery"]
    direction TB
    T33_usecase_usecase_PhaseExplainQuery__self[PhaseExplainQuery]
  end
  subgraph T36_usecase_usecase_PhaseValidateCommand["phase_command::PhaseValidateCommand"]
    direction TB
    T36_usecase_usecase_PhaseValidateCommand__self[PhaseValidateCommand]
  end
  subgraph R44_usecase_usecase_PhaseCommandConfigLoaderPort["phase_command::PhaseCommandConfigLoaderPort"]
    direction TB
    R44_usecase_usecase_PhaseCommandConfigLoaderPort__self[PhaseCommandConfigLoaderPort]
    R44_usecase_usecase_PhaseCommandConfigLoaderPort_load([load])
  end
  subgraph R35_usecase_usecase_PhaseCommandService["phase_command::PhaseCommandService"]
    direction TB
    R35_usecase_usecase_PhaseCommandService__self[PhaseCommandService]
    R35_usecase_usecase_PhaseCommandService_validate([validate])
    R35_usecase_usecase_PhaseCommandService_explain([explain])
    R35_usecase_usecase_PhaseCommandService_enter([enter])
  end
  end
  subgraph usecase_usecase_module_pre_review_command["usecase::pre_review_command"]
    direction TB
  subgraph T46_usecase_usecase_CurrentReviewTrackResolveError["pre_review_command::CurrentReviewTrackResolveError"]
    direction TB
    T46_usecase_usecase_CurrentReviewTrackResolveError__self[CurrentReviewTrackResolveError]
    T46_usecase_usecase_CurrentReviewTrackResolveError_ResolveFailed[ResolveFailed]
  end
  subgraph T38_usecase_usecase_PreReviewCommandConfig["pre_review_command::PreReviewCommandConfig"]
    direction TB
    T38_usecase_usecase_PreReviewCommandConfig__self[PreReviewCommandConfig]
    T38_usecase_usecase_PreReviewCommandConfig_try_new([try_new])
    T38_usecase_usecase_PreReviewCommandConfig_commands_for([commands_for])
  end
  subgraph T53_usecase_usecase_PreReviewCommandConfigValidationError["pre_review_command::PreReviewCommandConfigValidationError"]
    direction TB
    T53_usecase_usecase_PreReviewCommandConfigValidationError__self[PreReviewCommandConfigValidationError]
    T53_usecase_usecase_PreReviewCommandConfigValidationError_InvalidSchemaVersion[InvalidSchemaVersion]
    T53_usecase_usecase_PreReviewCommandConfigValidationError_DuplicateScope[DuplicateScope]
  end
  subgraph T47_usecase_usecase_PreReviewCommandDispatchCommand["pre_review_command::PreReviewCommandDispatchCommand"]
    direction TB
    T47_usecase_usecase_PreReviewCommandDispatchCommand__self[PreReviewCommandDispatchCommand]
  end
  subgraph T45_usecase_usecase_PreReviewCommandDispatchError["pre_review_command::PreReviewCommandDispatchError"]
    direction TB
    T45_usecase_usecase_PreReviewCommandDispatchError__self[PreReviewCommandDispatchError]
    T45_usecase_usecase_PreReviewCommandDispatchError_Config[Config]
    T45_usecase_usecase_PreReviewCommandDispatchError_UnknownScope[UnknownScope]
    T45_usecase_usecase_PreReviewCommandDispatchError_TrackResolution[TrackResolution]
    T45_usecase_usecase_PreReviewCommandDispatchError_TrackMismatch[TrackMismatch]
    T45_usecase_usecase_PreReviewCommandDispatchError_Runner[Runner]
  end
  subgraph T50_usecase_usecase_PreReviewCommandDispatchInteractor["pre_review_command::PreReviewCommandDispatchInteractor"]
    direction TB
    T50_usecase_usecase_PreReviewCommandDispatchInteractor__self[PreReviewCommandDispatchInteractor]
    T50_usecase_usecase_PreReviewCommandDispatchInteractor_new([new])
  end
  subgraph T47_usecase_usecase_PreReviewCommandDispatchOutcome["pre_review_command::PreReviewCommandDispatchOutcome"]
    direction TB
    T47_usecase_usecase_PreReviewCommandDispatchOutcome__self[PreReviewCommandDispatchOutcome]
    T47_usecase_usecase_PreReviewCommandDispatchOutcome_ReadyForReview[ReadyForReview]
    T47_usecase_usecase_PreReviewCommandDispatchOutcome_Blocked[Blocked]
  end
  subgraph T53_usecase_usecase_PreReviewCommandGatedReviewInteractor["pre_review_command::PreReviewCommandGatedReviewInteractor"]
    direction TB
    T53_usecase_usecase_PreReviewCommandGatedReviewInteractor__self[PreReviewCommandGatedReviewInteractor]
    T53_usecase_usecase_PreReviewCommandGatedReviewInteractor_new([new])
  end
  subgraph T48_usecase_usecase_PreReviewScopeCommandDeclaration["pre_review_command::PreReviewScopeCommandDeclaration"]
    direction TB
    T48_usecase_usecase_PreReviewScopeCommandDeclaration__self[PreReviewScopeCommandDeclaration]
    T48_usecase_usecase_PreReviewScopeCommandDeclaration_new([new])
    T48_usecase_usecase_PreReviewScopeCommandDeclaration_scope([scope])
    T48_usecase_usecase_PreReviewScopeCommandDeclaration_commands([commands])
  end
  subgraph T35_usecase_usecase_ReviewScopeSelector["pre_review_command::ReviewScopeSelector"]
    direction TB
    T35_usecase_usecase_ReviewScopeSelector__self[ReviewScopeSelector]
    T35_usecase_usecase_ReviewScopeSelector_Named[Named]
    T35_usecase_usecase_ReviewScopeSelector_Other[Other]
  end
  subgraph T35_usecase_usecase_ReviewTrackSelector["pre_review_command::ReviewTrackSelector"]
    direction TB
    T35_usecase_usecase_ReviewTrackSelector__self[ReviewTrackSelector]
    T35_usecase_usecase_ReviewTrackSelector_Explicit[Explicit]
    T35_usecase_usecase_ReviewTrackSelector_CurrentBranch[CurrentBranch]
  end
  subgraph R46_usecase_usecase_CurrentReviewTrackResolverPort["pre_review_command::CurrentReviewTrackResolverPort"]
    direction TB
    R46_usecase_usecase_CurrentReviewTrackResolverPort__self[CurrentReviewTrackResolverPort]
    R46_usecase_usecase_CurrentReviewTrackResolverPort_resolve([resolve])
  end
  subgraph R48_usecase_usecase_PreReviewCommandConfigLoaderPort["pre_review_command::PreReviewCommandConfigLoaderPort"]
    direction TB
    R48_usecase_usecase_PreReviewCommandConfigLoaderPort__self[PreReviewCommandConfigLoaderPort]
    R48_usecase_usecase_PreReviewCommandConfigLoaderPort_load([load])
  end
  subgraph R47_usecase_usecase_PreReviewCommandDispatchService["pre_review_command::PreReviewCommandDispatchService"]
    direction TB
    R47_usecase_usecase_PreReviewCommandDispatchService__self[PreReviewCommandDispatchService]
    R47_usecase_usecase_PreReviewCommandDispatchService_dispatch([dispatch])
  end
  end
  subgraph usecase_usecase_module_program_runner["usecase::program_runner"]
    direction TB
  subgraph T37_usecase_usecase_CapturedProgramOutput["program_runner::CapturedProgramOutput"]
    direction TB
    T37_usecase_usecase_CapturedProgramOutput__self[CapturedProgramOutput]
  end
  subgraph T48_usecase_usecase_ClassifiedProgramExecutionRecord["program_runner::ClassifiedProgramExecutionRecord"]
    direction TB
    T48_usecase_usecase_ClassifiedProgramExecutionRecord__self[ClassifiedProgramExecutionRecord]
    T48_usecase_usecase_ClassifiedProgramExecutionRecord_Succeeded[Succeeded]
    T48_usecase_usecase_ClassifiedProgramExecutionRecord_Failed[Failed]
  end
  subgraph T44_usecase_usecase_FailedProgramExecutionRecord["program_runner::FailedProgramExecutionRecord"]
    direction TB
    T44_usecase_usecase_FailedProgramExecutionRecord__self[FailedProgramExecutionRecord]
  end
  subgraph T38_usecase_usecase_ProgramExecutionRecord["program_runner::ProgramExecutionRecord"]
    direction TB
    T38_usecase_usecase_ProgramExecutionRecord__self[ProgramExecutionRecord]
    T38_usecase_usecase_ProgramExecutionRecord_classify([classify])
  end
  subgraph T31_usecase_usecase_ProgramExitCode["program_runner::ProgramExitCode"]
    direction TB
    T31_usecase_usecase_ProgramExitCode__self[ProgramExitCode]
    T31_usecase_usecase_ProgramExitCode_new([new])
    T31_usecase_usecase_ProgramExitCode_as_i32([as_i32])
  end
  subgraph T33_usecase_usecase_ProgramInvocation["program_runner::ProgramInvocation"]
    direction TB
    T33_usecase_usecase_ProgramInvocation__self[ProgramInvocation]
  end
  subgraph T35_usecase_usecase_ProgramOutputStream["program_runner::ProgramOutputStream"]
    direction TB
    T35_usecase_usecase_ProgramOutputStream__self[ProgramOutputStream]
    T35_usecase_usecase_ProgramOutputStream_Stdout[Stdout]
    T35_usecase_usecase_ProgramOutputStream_Stderr[Stderr]
  end
  subgraph T33_usecase_usecase_ProgramRunOutcome["program_runner::ProgramRunOutcome"]
    direction TB
    T33_usecase_usecase_ProgramRunOutcome__self[ProgramRunOutcome]
    T33_usecase_usecase_ProgramRunOutcome_Exited[Exited]
    T33_usecase_usecase_ProgramRunOutcome_TimedOut[TimedOut]
    T33_usecase_usecase_ProgramRunOutcome_OutputLimitExceeded[OutputLimitExceeded]
  end
  subgraph T34_usecase_usecase_ProgramRunnerError["program_runner::ProgramRunnerError"]
    direction TB
    T34_usecase_usecase_ProgramRunnerError__self[ProgramRunnerError]
    T34_usecase_usecase_ProgramRunnerError_SpawnFailed[SpawnFailed]
    T34_usecase_usecase_ProgramRunnerError_WaitFailed[WaitFailed]
    T34_usecase_usecase_ProgramRunnerError_TerminateFailed[TerminateFailed]
  end
  subgraph T48_usecase_usecase_SuccessfulProgramExecutionRecord["program_runner::SuccessfulProgramExecutionRecord"]
    direction TB
    T48_usecase_usecase_SuccessfulProgramExecutionRecord__self[SuccessfulProgramExecutionRecord]
  end
  subgraph R33_usecase_usecase_ProgramRunnerPort["program_runner::ProgramRunnerPort"]
    direction TB
    R33_usecase_usecase_ProgramRunnerPort__self[ProgramRunnerPort]
    R33_usecase_usecase_ProgramRunnerPort_run([run])
  end
  end
  subgraph usecase_usecase_module_ref_verify["usecase::ref_verify"]
    direction TB
  subgraph T36_usecase_usecase_RefVerifyChainFilter["ref_verify::driver_service::RefVerifyChainFilter"]
    direction TB
    T36_usecase_usecase_RefVerifyChainFilter__self[RefVerifyChainFilter]
    T36_usecase_usecase_RefVerifyChainFilter_Chain1[Chain1]
    T36_usecase_usecase_RefVerifyChainFilter_Chain2[Chain2]
    T36_usecase_usecase_RefVerifyChainFilter_All[All]
  end
  subgraph R41_usecase_usecase_RefVerifyAggregateService["ref_verify::driver_service::RefVerifyAggregateService"]
    direction TB
    R41_usecase_usecase_RefVerifyAggregateService__self[RefVerifyAggregateService]
    R41_usecase_usecase_RefVerifyAggregateService_run([run])
    R41_usecase_usecase_RefVerifyAggregateService_results([results])
  end
  subgraph R51_usecase_usecase_RefVerifyCheckApprovedDriverService["ref_verify::driver_service::RefVerifyCheckApprovedDriverService"]
    direction TB
    R51_usecase_usecase_RefVerifyCheckApprovedDriverService__self[RefVerifyCheckApprovedDriverService]
    R51_usecase_usecase_RefVerifyCheckApprovedDriverService_check_approved([check_approved])
  end
  end
  subgraph usecase_usecase_module_review_v2["usecase::review_v2"]
    direction TB
  subgraph T46_usecase_usecase_NonEmptyReviewerFindingsOutput["review_v2::review_aux::NonEmptyReviewerFindingsOutput"]
    direction TB
    T46_usecase_usecase_NonEmptyReviewerFindingsOutput__self[NonEmptyReviewerFindingsOutput]
    T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new([try_new])
    T46_usecase_usecase_NonEmptyReviewerFindingsOutput_as_slice([as_slice])
    T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new([try_new])
    T46_usecase_usecase_NonEmptyReviewerFindingsOutput_as_slice([as_slice])
  end
  subgraph T54_usecase_usecase_ReviewCheckZeroFindingsEvaluationError["review_v2::check_zero_findings::ReviewCheckZeroFindingsEvaluationError"]
    direction TB
    T54_usecase_usecase_ReviewCheckZeroFindingsEvaluationError__self[ReviewCheckZeroFindingsEvaluationError]
    T54_usecase_usecase_ReviewCheckZeroFindingsEvaluationError_EvaluationFailed[EvaluationFailed]
  end
  subgraph T49_usecase_usecase_ReviewCheckZeroFindingsInteractor["review_v2::check_zero_findings::ReviewCheckZeroFindingsInteractor"]
    direction TB
    T49_usecase_usecase_ReviewCheckZeroFindingsInteractor__self[ReviewCheckZeroFindingsInteractor]
    T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new([new])
    T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new([new])
  end
  subgraph T46_usecase_usecase_ReviewCheckZeroFindingsOutcome["review_v2::check_zero_findings::ReviewCheckZeroFindingsOutcome"]
    direction TB
    T46_usecase_usecase_ReviewCheckZeroFindingsOutcome__self[ReviewCheckZeroFindingsOutcome]
    T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_CurrentFinalZeroFindings[CurrentFinalZeroFindings]
    T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_MissingFinalVerdict[MissingFinalVerdict]
    T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_StaleFinalVerdict[StaleFinalVerdict]
    T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_FindingsRemain[FindingsRemain]
  end
  subgraph T44_usecase_usecase_ReviewCheckZeroFindingsQuery["review_v2::check_zero_findings::ReviewCheckZeroFindingsQuery"]
    direction TB
    T44_usecase_usecase_ReviewCheckZeroFindingsQuery__self[ReviewCheckZeroFindingsQuery]
    T44_usecase_usecase_ReviewCheckZeroFindingsQuery_try_new([try_new])
  end
  subgraph T54_usecase_usecase_ReviewCheckZeroFindingsValidationError["review_v2::check_zero_findings::ReviewCheckZeroFindingsValidationError"]
    direction TB
    T54_usecase_usecase_ReviewCheckZeroFindingsValidationError__self[ReviewCheckZeroFindingsValidationError]
    T54_usecase_usecase_ReviewCheckZeroFindingsValidationError_InvalidTrackId[InvalidTrackId]
    T54_usecase_usecase_ReviewCheckZeroFindingsValidationError_InvalidScope[InvalidScope]
  end
  subgraph T51_usecase_usecase_ReviewFindingsOutputValidationError["review_v2::review_aux::ReviewFindingsOutputValidationError"]
    direction TB
    T51_usecase_usecase_ReviewFindingsOutputValidationError__self[ReviewFindingsOutputValidationError]
    T51_usecase_usecase_ReviewFindingsOutputValidationError_Empty[Empty]
  end
  subgraph T35_usecase_usecase_ReviewFixResolution["review_v2::run_review_fix::ReviewFixResolution"]
    direction TB
    T35_usecase_usecase_ReviewFixResolution__self[ReviewFixResolution]
    T35_usecase_usecase_ReviewFixResolution_new([new])
    T35_usecase_usecase_ReviewFixResolution_track_id([track_id])
    T35_usecase_usecase_ReviewFixResolution_repository_root([repository_root])
    T35_usecase_usecase_ReviewFixResolution_new([new])
    T35_usecase_usecase_ReviewFixResolution_track_id([track_id])
    T35_usecase_usecase_ReviewFixResolution_repository_root([repository_root])
  end
  subgraph T36_usecase_usecase_ReviewFixRunnerError["review_v2::run_review_fix::ReviewFixRunnerError"]
    direction TB
    T36_usecase_usecase_ReviewFixRunnerError__self[ReviewFixRunnerError]
    T36_usecase_usecase_ReviewFixRunnerError_SmokeTestFailed[SmokeTestFailed]
    T36_usecase_usecase_ReviewFixRunnerError_SpawnFailed[SpawnFailed]
    T36_usecase_usecase_ReviewFixRunnerError_SentinelNotFound[SentinelNotFound]
    T36_usecase_usecase_ReviewFixRunnerError_SubagentDispatchRequired[SubagentDispatchRequired]
    T36_usecase_usecase_ReviewFixRunnerError_Unexpected[Unexpected]
  end
  subgraph T42_usecase_usecase_ReviewFixTrackResolveError["review_v2::run_review_fix::ReviewFixTrackResolveError"]
    direction TB
    T42_usecase_usecase_ReviewFixTrackResolveError__self[ReviewFixTrackResolveError]
    T42_usecase_usecase_ReviewFixTrackResolveError_BranchReadFailed[BranchReadFailed]
    T42_usecase_usecase_ReviewFixTrackResolveError_NonTrackBranch[NonTrackBranch]
  end
  subgraph T39_usecase_usecase_ReviewNotRequiredReason["review_v2::review_aux::ReviewNotRequiredReason"]
    direction TB
    T39_usecase_usecase_ReviewNotRequiredReason__self[ReviewNotRequiredReason]
    T39_usecase_usecase_ReviewNotRequiredReason_Empty[Empty]
    T39_usecase_usecase_ReviewNotRequiredReason_ZeroFindings[ZeroFindings]
  end
  subgraph T36_usecase_usecase_ReviewRequiredReason["review_v2::review_aux::ReviewRequiredReason"]
    direction TB
    T36_usecase_usecase_ReviewRequiredReason__self[ReviewRequiredReason]
    T36_usecase_usecase_ReviewRequiredReason_NotStarted[NotStarted]
    T36_usecase_usecase_ReviewRequiredReason_FindingsRemain[FindingsRemain]
    T36_usecase_usecase_ReviewRequiredReason_StaleHash[StaleHash]
  end
  subgraph T34_usecase_usecase_ReviewResultsError["review_v2::review_aux::ReviewResultsError"]
    direction TB
    T34_usecase_usecase_ReviewResultsError__self[ReviewResultsError]
    T34_usecase_usecase_ReviewResultsError_Failed[Failed]
    T34_usecase_usecase_ReviewResultsError_UnknownScope[UnknownScope]
    T34_usecase_usecase_ReviewResultsError_MissingScopeState[MissingScopeState]
  end
  subgraph T39_usecase_usecase_ReviewResultsInteractor["review_v2::review_aux::ReviewResultsInteractor"]
    direction TB
    T39_usecase_usecase_ReviewResultsInteractor__self[ReviewResultsInteractor]
    T39_usecase_usecase_ReviewResultsInteractor_new([new])
    T39_usecase_usecase_ReviewResultsInteractor_new([new])
  end
  subgraph T35_usecase_usecase_ReviewResultsOutput["review_v2::review_aux::ReviewResultsOutput"]
    direction TB
    T35_usecase_usecase_ReviewResultsOutput__self[ReviewResultsOutput]
  end
  subgraph T42_usecase_usecase_ReviewResultsScopeSnapshot["review_v2::review_aux::ReviewResultsScopeSnapshot"]
    direction TB
    T42_usecase_usecase_ReviewResultsScopeSnapshot__self[ReviewResultsScopeSnapshot]
  end
  subgraph T39_usecase_usecase_ReviewRoundResultOutput["review_v2::review_aux::ReviewRoundResultOutput"]
    direction TB
    T39_usecase_usecase_ReviewRoundResultOutput__self[ReviewRoundResultOutput]
  end
  subgraph T40_usecase_usecase_ReviewRoundResultVerdict["review_v2::review_aux::ReviewRoundResultVerdict"]
    direction TB
    T40_usecase_usecase_ReviewRoundResultVerdict__self[ReviewRoundResultVerdict]
    T40_usecase_usecase_ReviewRoundResultVerdict_ZeroFindings[ZeroFindings]
    T40_usecase_usecase_ReviewRoundResultVerdict_FindingsRemain[FindingsRemain]
  end
  subgraph T36_usecase_usecase_ReviewRunLocalOutput["review_v2::review_aux::ReviewRunLocalOutput"]
    direction TB
    T36_usecase_usecase_ReviewRunLocalOutput__self[ReviewRunLocalOutput]
  end
  subgraph T31_usecase_usecase_ReviewScopeName["review_v2::review_aux::ReviewScopeName"]
    direction TB
    T31_usecase_usecase_ReviewScopeName__self[ReviewScopeName]
    T31_usecase_usecase_ReviewScopeName_try_new([try_new])
    T31_usecase_usecase_ReviewScopeName_other([other])
    T31_usecase_usecase_ReviewScopeName_as_str([as_str])
    T31_usecase_usecase_ReviewScopeName_try_new([try_new])
    T31_usecase_usecase_ReviewScopeName_other([other])
    T31_usecase_usecase_ReviewScopeName_as_str([as_str])
  end
  subgraph T46_usecase_usecase_ReviewScopeNameValidationError["review_v2::review_aux::ReviewScopeNameValidationError"]
    direction TB
    T46_usecase_usecase_ReviewScopeNameValidationError__self[ReviewScopeNameValidationError]
    T46_usecase_usecase_ReviewScopeNameValidationError_Invalid[Invalid]
  end
  subgraph T39_usecase_usecase_ReviewScopeResultOutput["review_v2::review_aux::ReviewScopeResultOutput"]
    direction TB
    T39_usecase_usecase_ReviewScopeResultOutput__self[ReviewScopeResultOutput]
  end
  subgraph T38_usecase_usecase_ReviewScopeResultState["review_v2::review_aux::ReviewScopeResultState"]
    direction TB
    T38_usecase_usecase_ReviewScopeResultState__self[ReviewScopeResultState]
    T38_usecase_usecase_ReviewScopeResultState_RequiredNotStarted[RequiredNotStarted]
    T38_usecase_usecase_ReviewScopeResultState_RequiredFindingsRemain[RequiredFindingsRemain]
    T38_usecase_usecase_ReviewScopeResultState_RequiredStaleHash[RequiredStaleHash]
    T38_usecase_usecase_ReviewScopeResultState_Empty[Empty]
    T38_usecase_usecase_ReviewScopeResultState_Approved[Approved]
  end
  subgraph T43_usecase_usecase_ReviewScopeSelectionRequest["review_v2::review_aux::ReviewScopeSelectionRequest"]
    direction TB
    T43_usecase_usecase_ReviewScopeSelectionRequest__self[ReviewScopeSelectionRequest]
    T43_usecase_usecase_ReviewScopeSelectionRequest_NamedCandidate[NamedCandidate]
    T43_usecase_usecase_ReviewScopeSelectionRequest_All[All]
    T43_usecase_usecase_ReviewScopeSelectionRequest_try_new([try_new])
    T43_usecase_usecase_ReviewScopeSelectionRequest_try_new([try_new])
  end
  subgraph T51_usecase_usecase_ReviewScopeSelectionValidationError["review_v2::review_aux::ReviewScopeSelectionValidationError"]
    direction TB
    T51_usecase_usecase_ReviewScopeSelectionValidationError__self[ReviewScopeSelectionValidationError]
    T51_usecase_usecase_ReviewScopeSelectionValidationError_ScopeAndAll[ScopeAndAll]
    T51_usecase_usecase_ReviewScopeSelectionValidationError_InvalidScope[InvalidScope]
  end
  subgraph T33_usecase_usecase_ReviewStoredRound["review_v2::review_aux::ReviewStoredRound"]
    direction TB
    T33_usecase_usecase_ReviewStoredRound__self[ReviewStoredRound]
  end
  subgraph T40_usecase_usecase_ReviewStoredRoundVerdict["review_v2::review_aux::ReviewStoredRoundVerdict"]
    direction TB
    T40_usecase_usecase_ReviewStoredRoundVerdict__self[ReviewStoredRoundVerdict]
  end
  subgraph T38_usecase_usecase_ReviewStoredScopeState["review_v2::review_aux::ReviewStoredScopeState"]
    direction TB
    T38_usecase_usecase_ReviewStoredScopeState__self[ReviewStoredScopeState]
    T38_usecase_usecase_ReviewStoredScopeState_Required[Required]
    T38_usecase_usecase_ReviewStoredScopeState_NotRequired[NotRequired]
  end
  subgraph T43_usecase_usecase_ReviewStoredScopeStateEntry["review_v2::review_aux::ReviewStoredScopeStateEntry"]
    direction TB
    T43_usecase_usecase_ReviewStoredScopeStateEntry__self[ReviewStoredScopeStateEntry]
  end
  subgraph T29_usecase_usecase_ReviewTrackId["review_v2::run_review_fix::ReviewTrackId"]
    direction TB
    T29_usecase_usecase_ReviewTrackId__self[ReviewTrackId]
    T29_usecase_usecase_ReviewTrackId_try_new([try_new])
    T29_usecase_usecase_ReviewTrackId_as_str([as_str])
    T29_usecase_usecase_ReviewTrackId_try_new([try_new])
    T29_usecase_usecase_ReviewTrackId_as_str([as_str])
  end
  subgraph T44_usecase_usecase_ReviewTrackIdValidationError["review_v2::run_review_fix::ReviewTrackIdValidationError"]
    direction TB
    T44_usecase_usecase_ReviewTrackIdValidationError__self[ReviewTrackIdValidationError]
    T44_usecase_usecase_ReviewTrackIdValidationError_Invalid[Invalid]
  end
  subgraph T37_usecase_usecase_ReviewerFindingOutput["review_v2::review_aux::ReviewerFindingOutput"]
    direction TB
    T37_usecase_usecase_ReviewerFindingOutput__self[ReviewerFindingOutput]
  end
  subgraph T35_usecase_usecase_RunReviewFixCommand["review_v2::run_review_fix::RunReviewFixCommand"]
    direction TB
    T35_usecase_usecase_RunReviewFixCommand__self[RunReviewFixCommand]
    T35_usecase_usecase_RunReviewFixCommand_new_resolved([new_resolved])
    T35_usecase_usecase_RunReviewFixCommand_scope([scope])
    T35_usecase_usecase_RunReviewFixCommand_briefing_file([briefing_file])
    T35_usecase_usecase_RunReviewFixCommand_track_id([track_id])
    T35_usecase_usecase_RunReviewFixCommand_repository_root([repository_root])
    T35_usecase_usecase_RunReviewFixCommand_round_type([round_type])
    T35_usecase_usecase_RunReviewFixCommand_model([model])
    T35_usecase_usecase_RunReviewFixCommand_new_resolved([new_resolved])
    T35_usecase_usecase_RunReviewFixCommand_scope([scope])
    T35_usecase_usecase_RunReviewFixCommand_briefing_file([briefing_file])
    T35_usecase_usecase_RunReviewFixCommand_track_id([track_id])
    T35_usecase_usecase_RunReviewFixCommand_repository_root([repository_root])
    T35_usecase_usecase_RunReviewFixCommand_round_type([round_type])
    T35_usecase_usecase_RunReviewFixCommand_model([model])
  end
  subgraph T50_usecase_usecase_RunReviewFixCommandValidationError["review_v2::run_review_fix::RunReviewFixCommandValidationError"]
    direction TB
    T50_usecase_usecase_RunReviewFixCommandValidationError__self[RunReviewFixCommandValidationError]
    T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidScope[InvalidScope]
    T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidTrackId[InvalidTrackId]
    T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidRoundType[InvalidRoundType]
    T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidModel[InvalidModel]
  end
  subgraph T33_usecase_usecase_RunReviewFixError["review_v2::run_review_fix::RunReviewFixError"]
    direction TB
    T33_usecase_usecase_RunReviewFixError__self[RunReviewFixError]
    T33_usecase_usecase_RunReviewFixError_FixRunnerFailed[FixRunnerFailed]
    T33_usecase_usecase_RunReviewFixError_TrackResolution[TrackResolution]
    T33_usecase_usecase_RunReviewFixError_TrackMismatch[TrackMismatch]
  end
  subgraph T38_usecase_usecase_RunReviewFixInteractor["review_v2::run_review_fix::RunReviewFixInteractor"]
    direction TB
    T38_usecase_usecase_RunReviewFixInteractor__self[RunReviewFixInteractor]
    T38_usecase_usecase_RunReviewFixInteractor_new([new])
    T38_usecase_usecase_RunReviewFixInteractor_new([new])
  end
  subgraph T34_usecase_usecase_RunReviewFixOutput["review_v2::run_review_fix::RunReviewFixOutput"]
    direction TB
    T34_usecase_usecase_RunReviewFixOutput__self[RunReviewFixOutput]
  end
  subgraph T35_usecase_usecase_RunReviewFixRequest["review_v2::run_review_fix::RunReviewFixRequest"]
    direction TB
    T35_usecase_usecase_RunReviewFixRequest__self[RunReviewFixRequest]
    T35_usecase_usecase_RunReviewFixRequest_try_new([try_new])
    T35_usecase_usecase_RunReviewFixRequest_try_new([try_new])
  end
  subgraph T43_usecase_usecase_SubagentDispatchInstruction["review_v2::run_review_fix::SubagentDispatchInstruction"]
    direction TB
    T43_usecase_usecase_SubagentDispatchInstruction__self[SubagentDispatchInstruction]
  end
  subgraph T28_usecase_usecase_SubagentName["review_v2::run_review_fix::SubagentName"]
    direction TB
    T28_usecase_usecase_SubagentName__self[SubagentName]
    T28_usecase_usecase_SubagentName_try_new([try_new])
    T28_usecase_usecase_SubagentName_as_str([as_str])
    T28_usecase_usecase_SubagentName_try_new([try_new])
    T28_usecase_usecase_SubagentName_as_str([as_str])
  end
  subgraph T43_usecase_usecase_SubagentNameValidationError["review_v2::run_review_fix::SubagentNameValidationError"]
    direction TB
    T43_usecase_usecase_SubagentNameValidationError__self[SubagentNameValidationError]
    T43_usecase_usecase_SubagentNameValidationError_Invalid[Invalid]
  end
  subgraph R42_usecase_usecase_ReviewCheckApprovedService["review_v2::check_approved::ReviewCheckApprovedService"]
    direction TB
    R42_usecase_usecase_ReviewCheckApprovedService__self[ReviewCheckApprovedService]
    R42_usecase_usecase_ReviewCheckApprovedService_check_approved([check_approved])
  end
  subgraph R46_usecase_usecase_ReviewCheckZeroFindingsService["review_v2::check_zero_findings::ReviewCheckZeroFindingsService"]
    direction TB
    R46_usecase_usecase_ReviewCheckZeroFindingsService__self[ReviewCheckZeroFindingsService]
    R46_usecase_usecase_ReviewCheckZeroFindingsService_check_zero_findings([check_zero_findings])
  end
  subgraph R48_usecase_usecase_ReviewCheckZeroFindingsStatePort["review_v2::check_zero_findings::ReviewCheckZeroFindingsStatePort"]
    direction TB
    R48_usecase_usecase_ReviewCheckZeroFindingsStatePort__self[ReviewCheckZeroFindingsStatePort]
    R48_usecase_usecase_ReviewCheckZeroFindingsStatePort_state_for([state_for])
  end
  subgraph R31_usecase_usecase_ReviewFixRunner["review_v2::run_review_fix::ReviewFixRunner"]
    direction TB
    R31_usecase_usecase_ReviewFixRunner__self[ReviewFixRunner]
    R31_usecase_usecase_ReviewFixRunner_run_fix([run_fix])
  end
  subgraph R42_usecase_usecase_ReviewFixTrackResolverPort["review_v2::run_review_fix::ReviewFixTrackResolverPort"]
    direction TB
    R42_usecase_usecase_ReviewFixTrackResolverPort__self[ReviewFixTrackResolverPort]
    R42_usecase_usecase_ReviewFixTrackResolverPort_resolve_current_track([resolve_current_track])
  end
  subgraph R38_usecase_usecase_ReviewResultsRoundPort["review_v2::review_aux::ReviewResultsRoundPort"]
    direction TB
    R38_usecase_usecase_ReviewResultsRoundPort__self[ReviewResultsRoundPort]
    R38_usecase_usecase_ReviewResultsRoundPort_load_scope_rounds([load_scope_rounds])
  end
  subgraph R38_usecase_usecase_ReviewResultsScopePort["review_v2::review_aux::ReviewResultsScopePort"]
    direction TB
    R38_usecase_usecase_ReviewResultsScopePort__self[ReviewResultsScopePort]
    R38_usecase_usecase_ReviewResultsScopePort_load_scope_snapshot([load_scope_snapshot])
  end
  subgraph R36_usecase_usecase_ReviewResultsService["review_v2::review_aux::ReviewResultsService"]
    direction TB
    R36_usecase_usecase_ReviewResultsService__self[ReviewResultsService]
    R36_usecase_usecase_ReviewResultsService_results([results])
  end
  subgraph R38_usecase_usecase_ReviewResultsStatePort["review_v2::review_aux::ReviewResultsStatePort"]
    direction TB
    R38_usecase_usecase_ReviewResultsStatePort__self[ReviewResultsStatePort]
    R38_usecase_usecase_ReviewResultsStatePort_load_scope_states([load_scope_states])
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
    R29_usecase_usecase_ReviewService_check_approved([check_approved])
    R29_usecase_usecase_ReviewService_classify([classify])
    R29_usecase_usecase_ReviewService_files([files])
    R29_usecase_usecase_ReviewService_validate_scope([validate_scope])
    R29_usecase_usecase_ReviewService_get_briefing([get_briefing])
    R29_usecase_usecase_ReviewService_persist_commit_hash([persist_commit_hash])
  end
  subgraph R35_usecase_usecase_RunReviewFixService["review_v2::run_review_fix::RunReviewFixService"]
    direction TB
    R35_usecase_usecase_RunReviewFixService__self[RunReviewFixService]
    R35_usecase_usecase_RunReviewFixService_run([run])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_operator_command_config["infrastructure::operator_command_config"]
    direction TB
  subgraph T48_infrastructure_infrastructure_CommandArgumentDto["operator_command_config::CommandArgumentDto"]
    direction TB
    T48_infrastructure_infrastructure_CommandArgumentDto__self[CommandArgumentDto]
  end
  subgraph T44_infrastructure_infrastructure_CommandArgvDto["operator_command_config::CommandArgvDto"]
    direction TB
    T44_infrastructure_infrastructure_CommandArgvDto__self[CommandArgvDto]
  end
  subgraph T59_infrastructure_infrastructure_CommandConfigSchemaVersionDto["operator_command_config::CommandConfigSchemaVersionDto"]
    direction TB
    T59_infrastructure_infrastructure_CommandConfigSchemaVersionDto__self[CommandConfigSchemaVersionDto]
  end
  subgraph T53_infrastructure_infrastructure_CommandDeclarationIdDto["operator_command_config::CommandDeclarationIdDto"]
    direction TB
    T53_infrastructure_infrastructure_CommandDeclarationIdDto__self[CommandDeclarationIdDto]
  end
  subgraph T54_infrastructure_infrastructure_CommandTimeoutSecondsDto["operator_command_config::CommandTimeoutSecondsDto"]
    direction TB
    T54_infrastructure_infrastructure_CommandTimeoutSecondsDto__self[CommandTimeoutSecondsDto]
  end
  subgraph T50_infrastructure_infrastructure_ConfiguredCommandDto["operator_command_config::ConfiguredCommandDto"]
    direction TB
    T50_infrastructure_infrastructure_ConfiguredCommandDto__self[ConfiguredCommandDto]
  end
  subgraph T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader["operator_command_config::FsPhaseCommandConfigLoader"]
    direction TB
    T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader__self[FsPhaseCommandConfigLoader]
    T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader_new([new])
  end
  subgraph T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader["operator_command_config::FsPreReviewCommandConfigLoader"]
    direction TB
    T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader__self[FsPreReviewCommandConfigLoader]
    T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader_new([new])
  end
  subgraph T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver["operator_command_config::GitCurrentReviewTrackResolver"]
    direction TB
    T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver__self[GitCurrentReviewTrackResolver]
    T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver_new([new])
  end
  subgraph T51_infrastructure_infrastructure_PhaseCommandConfigDto["operator_command_config::PhaseCommandConfigDto"]
    direction TB
    T51_infrastructure_infrastructure_PhaseCommandConfigDto__self[PhaseCommandConfigDto]
  end
  subgraph T56_infrastructure_infrastructure_PhaseCommandDeclarationDto["operator_command_config::PhaseCommandDeclarationDto"]
    direction TB
    T56_infrastructure_infrastructure_PhaseCommandDeclarationDto__self[PhaseCommandDeclarationDto]
  end
  subgraph T55_infrastructure_infrastructure_PreReviewCommandConfigDto["operator_command_config::PreReviewCommandConfigDto"]
    direction TB
    T55_infrastructure_infrastructure_PreReviewCommandConfigDto__self[PreReviewCommandConfigDto]
  end
  subgraph T65_infrastructure_infrastructure_PreReviewScopeCommandDeclarationDto["operator_command_config::PreReviewScopeCommandDeclarationDto"]
    direction TB
    T65_infrastructure_infrastructure_PreReviewScopeCommandDeclarationDto__self[PreReviewScopeCommandDeclarationDto]
  end
  subgraph T48_infrastructure_infrastructure_ReviewScopeNameDto["operator_command_config::ReviewScopeNameDto"]
    direction TB
    T48_infrastructure_infrastructure_ReviewScopeNameDto__self[ReviewScopeNameDto]
  end
  F98_infrastructure_infrastructure_infrastructure__operator_command_config__decode_phase_command_config[[decode_phase_command_config]]
  F103_infrastructure_infrastructure_infrastructure__operator_command_config__decode_pre_review_command_config[[decode_pre_review_command_config]]
  end
  subgraph infrastructure_infrastructure_module_program_runner["infrastructure::program_runner"]
    direction TB
  subgraph T50_infrastructure_infrastructure_ProcessProgramRunner["program_runner::ProcessProgramRunner"]
    direction TB
    T50_infrastructure_infrastructure_ProcessProgramRunner__self[ProcessProgramRunner]
    T50_infrastructure_infrastructure_ProcessProgramRunner_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_ref_verify["infrastructure::ref_verify"]
    direction TB
  subgraph T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter["ref_verify::driver_adapter::FsRefVerifyAggregateAdapter"]
    direction TB
    T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self[FsRefVerifyAggregateAdapter]
    T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  subgraph T50_infrastructure_infrastructure_CodexReviewFixRunner["review_v2::review_fix_runner::CodexReviewFixRunner"]
    direction TB
    T50_infrastructure_infrastructure_CodexReviewFixRunner__self[CodexReviewFixRunner]
    T50_infrastructure_infrastructure_CodexReviewFixRunner_new([new])
  end
  subgraph T55_infrastructure_infrastructure_GitReviewFixTrackResolver["review_v2::review_fix_track_resolver::GitReviewFixTrackResolver"]
    direction TB
    T55_infrastructure_infrastructure_GitReviewFixTrackResolver__self[GitReviewFixTrackResolver]
  end
  subgraph T49_infrastructure_infrastructure_ResultsRoundAdapter["review_v2::results::ResultsRoundAdapter"]
    direction TB
    T49_infrastructure_infrastructure_ResultsRoundAdapter__self[ResultsRoundAdapter]
  end
  subgraph T49_infrastructure_infrastructure_ResultsScopeAdapter["review_v2::results::ResultsScopeAdapter"]
    direction TB
    T49_infrastructure_infrastructure_ResultsScopeAdapter__self[ResultsScopeAdapter]
  end
  subgraph T49_infrastructure_infrastructure_ResultsStateAdapter["review_v2::results::ResultsStateAdapter"]
    direction TB
    T49_infrastructure_infrastructure_ResultsStateAdapter__self[ResultsStateAdapter]
  end
  subgraph T65_infrastructure_infrastructure_ReviewCheckZeroFindingsStateAdapter["review_v2::check_zero_findings::ReviewCheckZeroFindingsStateAdapter"]
    direction TB
    T65_infrastructure_infrastructure_ReviewCheckZeroFindingsStateAdapter__self[ReviewCheckZeroFindingsStateAdapter]
  end
  subgraph T52_infrastructure_infrastructure_ReviewFixRunnerAdapter["review_v2::review_fix_runner::ReviewFixRunnerAdapter"]
    direction TB
    T52_infrastructure_infrastructure_ReviewFixRunnerAdapter__self[ReviewFixRunnerAdapter]
  end
  end
  subgraph infrastructure_infrastructure_module_task_contract_codec["infrastructure::task_contract_codec"]
    direction TB
  subgraph T51_infrastructure_infrastructure_ContractedEntryRefDto["task_contract_codec::ContractedEntryRefDto"]
    direction TB
    T51_infrastructure_infrastructure_ContractedEntryRefDto__self[ContractedEntryRefDto]
  end
  subgraph T41_infrastructure_infrastructure_EntryKeyDto["task_contract_codec::EntryKeyDto"]
    direction TB
    T41_infrastructure_infrastructure_EntryKeyDto__self[EntryKeyDto]
  end
  subgraph T40_infrastructure_infrastructure_LayerIdDto["task_contract_codec::LayerIdDto"]
    direction TB
    T40_infrastructure_infrastructure_LayerIdDto__self[LayerIdDto]
  end
  subgraph T53_infrastructure_infrastructure_TaskContractDocumentDto["task_contract_codec::TaskContractDocumentDto"]
    direction TB
    T53_infrastructure_infrastructure_TaskContractDocumentDto__self[TaskContractDocumentDto]
  end
  subgraph T58_infrastructure_infrastructure_TaskContractSchemaVersionDto["task_contract_codec::TaskContractSchemaVersionDto"]
    direction TB
    T58_infrastructure_infrastructure_TaskContractSchemaVersionDto__self[TaskContractSchemaVersionDto]
  end
  subgraph T39_infrastructure_infrastructure_TaskIdDto["task_contract_codec::TaskIdDto"]
    direction TB
    T39_infrastructure_infrastructure_TaskIdDto__self[TaskIdDto]
  end
  subgraph T40_infrastructure_infrastructure_TrackIdDto["task_contract_codec::TrackIdDto"]
    direction TB
    T40_infrastructure_infrastructure_TrackIdDto__self[TrackIdDto]
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_capability["cli_driver::capability"]
    direction TB
  subgraph T38_cli_driver_cli_driver_CapabilityDriver["capability::CapabilityDriver"]
    direction TB
    T38_cli_driver_cli_driver_CapabilityDriver__self[CapabilityDriver]
    T38_cli_driver_cli_driver_CapabilityDriver_new([new])
    T38_cli_driver_cli_driver_CapabilityDriver_handle([handle])
  end
  subgraph T47_cli_driver_cli_driver_CapabilityExecDriverInput["capability::CapabilityExecDriverInput"]
    direction TB
    T47_cli_driver_cli_driver_CapabilityExecDriverInput__self[CapabilityExecDriverInput]
  end
  end
  subgraph cli_driver_cli_driver_module_phase_command["cli_driver::phase_command"]
    direction TB
  subgraph T40_cli_driver_cli_driver_PhaseCommandDriver["phase_command::PhaseCommandDriver"]
    direction TB
    T40_cli_driver_cli_driver_PhaseCommandDriver__self[PhaseCommandDriver]
    T40_cli_driver_cli_driver_PhaseCommandDriver_new([new])
    T40_cli_driver_cli_driver_PhaseCommandDriver_handle([handle])
  end
  subgraph T39_cli_driver_cli_driver_PhaseCommandInput["phase_command::PhaseCommandInput"]
    direction TB
    T39_cli_driver_cli_driver_PhaseCommandInput__self[PhaseCommandInput]
    T39_cli_driver_cli_driver_PhaseCommandInput_Validate[Validate]
    T39_cli_driver_cli_driver_PhaseCommandInput_Explain[Explain]
    T39_cli_driver_cli_driver_PhaseCommandInput_Enter[Enter]
  end
  subgraph T32_cli_driver_cli_driver_PhaseIdArg["phase_command::PhaseIdArg"]
    direction TB
    T32_cli_driver_cli_driver_PhaseIdArg__self[PhaseIdArg]
    T32_cli_driver_cli_driver_PhaseIdArg_new([new])
    T32_cli_driver_cli_driver_PhaseIdArg_as_declaration_id([as_declaration_id])
  end
  end
  subgraph cli_driver_cli_driver_module_ref_verify["cli_driver::ref_verify"]
    direction TB
  subgraph T42_cli_driver_cli_driver_RefVerifyChainSelect["ref_verify::RefVerifyChainSelect"]
    direction TB
    T42_cli_driver_cli_driver_RefVerifyChainSelect__self[RefVerifyChainSelect]
    T42_cli_driver_cli_driver_RefVerifyChainSelect_Chain1[Chain1]
    T42_cli_driver_cli_driver_RefVerifyChainSelect_Chain2[Chain2]
    T42_cli_driver_cli_driver_RefVerifyChainSelect_All[All]
  end
  subgraph T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput["ref_verify::RefVerifyCheckApprovedInput"]
    direction TB
    T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput__self[RefVerifyCheckApprovedInput]
  end
  subgraph T37_cli_driver_cli_driver_RefVerifyDriver["ref_verify::RefVerifyDriver"]
    direction TB
    T37_cli_driver_cli_driver_RefVerifyDriver__self[RefVerifyDriver]
    T37_cli_driver_cli_driver_RefVerifyDriver_new([new])
    T37_cli_driver_cli_driver_RefVerifyDriver_handle([handle])
  end
  end
  subgraph cli_driver_cli_driver_module_render["cli_driver::render"]
    direction TB
  subgraph T36_cli_driver_cli_driver_CommandOutcome["render::CommandOutcome"]
    direction TB
    T36_cli_driver_cli_driver_CommandOutcome__self[CommandOutcome]
    T36_cli_driver_cli_driver_CommandOutcome_success([success])
    T36_cli_driver_cli_driver_CommandOutcome_failure([failure])
  end
  end
  subgraph cli_driver_cli_driver_module_review["cli_driver::review"]
    direction TB
  subgraph T44_cli_driver_cli_driver_ReviewCheckRoundSelect["review::ReviewCheckRoundSelect"]
    direction TB
    T44_cli_driver_cli_driver_ReviewCheckRoundSelect__self[ReviewCheckRoundSelect]
    T44_cli_driver_cli_driver_ReviewCheckRoundSelect_Final[Final]
  end
  subgraph T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput["review::ReviewCheckZeroFindingsInput"]
    direction TB
    T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput__self[ReviewCheckZeroFindingsInput]
    T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_try_new([try_new])
    T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_into_query([into_query])
  end
  subgraph T34_cli_driver_cli_driver_ReviewDriver["review::ReviewDriver"]
    direction TB
    T34_cli_driver_cli_driver_ReviewDriver__self[ReviewDriver]
    T34_cli_driver_cli_driver_ReviewDriver_new([new])
    T34_cli_driver_cli_driver_ReviewDriver_handle([handle])
    T34_cli_driver_cli_driver_ReviewDriver_new([new])
    T34_cli_driver_cli_driver_ReviewDriver_handle([handle])
  end
  subgraph T37_cli_driver_cli_driver_ReviewFixDriver["review::ReviewFixDriver"]
    direction TB
    T37_cli_driver_cli_driver_ReviewFixDriver__self[ReviewFixDriver]
    T37_cli_driver_cli_driver_ReviewFixDriver_new([new])
    T37_cli_driver_cli_driver_ReviewFixDriver_handle([handle])
    T37_cli_driver_cli_driver_ReviewFixDriver_new([new])
    T37_cli_driver_cli_driver_ReviewFixDriver_handle([handle])
  end
  subgraph T36_cli_driver_cli_driver_ReviewFixInput["review::ReviewFixInput"]
    direction TB
    T36_cli_driver_cli_driver_ReviewFixInput__self[ReviewFixInput]
    T36_cli_driver_cli_driver_ReviewFixInput_new([new])
    T36_cli_driver_cli_driver_ReviewFixInput_into_parts([into_parts])
    T36_cli_driver_cli_driver_ReviewFixInput_new([new])
    T36_cli_driver_cli_driver_ReviewFixInput_into_parts([into_parts])
  end
  subgraph T33_cli_driver_cli_driver_ReviewInput["review::ReviewInput"]
    direction TB
    T33_cli_driver_cli_driver_ReviewInput__self[ReviewInput]
    T33_cli_driver_cli_driver_ReviewInput_RunCodex[RunCodex]
    T33_cli_driver_cli_driver_ReviewInput_RunClaude[RunClaude]
    T33_cli_driver_cli_driver_ReviewInput_RunLocal[RunLocal]
    T33_cli_driver_cli_driver_ReviewInput_CheckApproved[CheckApproved]
    T33_cli_driver_cli_driver_ReviewInput_CheckZeroFindings[CheckZeroFindings]
    T33_cli_driver_cli_driver_ReviewInput_Results[Results]
    T33_cli_driver_cli_driver_ReviewInput_Classify[Classify]
    T33_cli_driver_cli_driver_ReviewInput_Files[Files]
    T33_cli_driver_cli_driver_ReviewInput_ValidateScope[ValidateScope]
    T33_cli_driver_cli_driver_ReviewInput_GetBriefing[GetBriefing]
    T33_cli_driver_cli_driver_ReviewInput_PersistCommitHash[PersistCommitHash]
  end
  subgraph T40_cli_driver_cli_driver_ReviewResultsInput["review::ReviewResultsInput"]
    direction TB
    T40_cli_driver_cli_driver_ReviewResultsInput__self[ReviewResultsInput]
    T40_cli_driver_cli_driver_ReviewResultsInput_try_new([try_new])
    T40_cli_driver_cli_driver_ReviewResultsInput_into_parts([into_parts])
    T40_cli_driver_cli_driver_ReviewResultsInput_try_new([try_new])
    T40_cli_driver_cli_driver_ReviewResultsInput_into_parts([into_parts])
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_capability["cli_composition::capability"]
    direction TB
  subgraph T57_cli_composition_cli_composition_CapabilityCompositionRoot["capability::CapabilityCompositionRoot"]
    direction TB
    T57_cli_composition_cli_composition_CapabilityCompositionRoot__self[CapabilityCompositionRoot]
    T57_cli_composition_cli_composition_CapabilityCompositionRoot_new([new])
    T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover([discover])
    T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver([capability_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_phase_command["cli_composition::phase_command"]
    direction TB
  subgraph T52_cli_composition_cli_composition_PhaseCompositionRoot["phase_command::PhaseCompositionRoot"]
    direction TB
    T52_cli_composition_cli_composition_PhaseCompositionRoot__self[PhaseCompositionRoot]
    T52_cli_composition_cli_composition_PhaseCompositionRoot_build([build])
  end
  end
  subgraph cli_composition_cli_composition_module_ref_verify["cli_composition::ref_verify"]
    direction TB
  subgraph T56_cli_composition_cli_composition_RefVerifyCompositionRoot["ref_verify::RefVerifyCompositionRoot"]
    direction TB
    T56_cli_composition_cli_composition_RefVerifyCompositionRoot__self[RefVerifyCompositionRoot]
    T56_cli_composition_cli_composition_RefVerifyCompositionRoot_new([new])
    T56_cli_composition_cli_composition_RefVerifyCompositionRoot_ref_verify_driver([ref_verify_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_review_v2["cli_composition::review_v2"]
    direction TB
  subgraph T53_cli_composition_cli_composition_ReviewCompositionRoot["review_v2::shim::ReviewCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_ReviewCompositionRoot__self[ReviewCompositionRoot]
    T53_cli_composition_cli_composition_ReviewCompositionRoot_new([new])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver([review_driver])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver([review_fix_driver])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_new([new])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver([review_driver])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver([review_fix_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph T18_cli_cli_CliCommand["CliCommand"]
    direction TB
    T18_cli_cli_CliCommand__self[CliCommand]
    T18_cli_cli_CliCommand_Arch[Arch]
    T18_cli_cli_CliCommand_AdrBaseline[AdrBaseline]
    T18_cli_cli_CliCommand_Conventions[Conventions]
    T18_cli_cli_CliCommand_Domain[Domain]
    T18_cli_cli_CliCommand_Guard[Guard]
    T18_cli_cli_CliCommand_Hook[Hook]
    T18_cli_cli_CliCommand_Maintenance[Maintenance]
    T18_cli_cli_CliCommand_Track[Track]
    T18_cli_cli_CliCommand_Git[Git]
    T18_cli_cli_CliCommand_Pr[Pr]
    T18_cli_cli_CliCommand_Capability[Capability]
    T18_cli_cli_CliCommand_Phase[Phase]
    T18_cli_cli_CliCommand_Review[Review]
    T18_cli_cli_CliCommand_File[File]
    T18_cli_cli_CliCommand_Verify[Verify]
    T18_cli_cli_CliCommand_FindSimilar[FindSimilar]
    T18_cli_cli_CliCommand_DupIndex[DupIndex]
    T18_cli_cli_CliCommand_DupCheck[DupCheck]
    T18_cli_cli_CliCommand_Telemetry[Telemetry]
    T18_cli_cli_CliCommand_Dry[Dry]
    T18_cli_cli_CliCommand_RefVerify[RefVerify]
    T18_cli_cli_CliCommand_TestObligation[TestObligation]
    T18_cli_cli_CliCommand_Signal[Signal]
    T18_cli_cli_CliCommand_TaskContract[TaskContract]
    T18_cli_cli_CliCommand_Catalog[Catalog]
    T18_cli_cli_CliCommand_CatalogueLint[CatalogueLint]
    T18_cli_cli_CliCommand_Template[Template]
    T18_cli_cli_CliCommand_CodexRuntime[CodexRuntime]
    T18_cli_cli_CliCommand_BatchPlan[BatchPlan]
    T18_cli_cli_CliCommand_Demo[Demo]
  end
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T26_cli_cli_CapabilityExecArgs["commands::capability::CapabilityExecArgs"]
    direction TB
    T26_cli_cli_CapabilityExecArgs__self[CapabilityExecArgs]
  end
  subgraph T25_cli_cli_CheckApprovedArgs["commands::ref_verify::CheckApprovedArgs"]
    direction TB
    T25_cli_cli_CheckApprovedArgs__self[CheckApprovedArgs]
  end
  subgraph T29_cli_cli_CheckZeroFindingsArgs["commands::review::CheckZeroFindingsArgs"]
    direction TB
    T29_cli_cli_CheckZeroFindingsArgs__self[CheckZeroFindingsArgs]
  end
  subgraph T20_cli_cli_PhaseCommand["commands::phase::PhaseCommand"]
    direction TB
    T20_cli_cli_PhaseCommand__self[PhaseCommand]
    T20_cli_cli_PhaseCommand_Validate[Validate]
    T20_cli_cli_PhaseCommand_Explain[Explain]
    T20_cli_cli_PhaseCommand_Enter[Enter]
  end
  subgraph T22_cli_cli_PhaseEnterArgs["commands::phase::PhaseEnterArgs"]
    direction TB
    T22_cli_cli_PhaseEnterArgs__self[PhaseEnterArgs]
  end
  subgraph T19_cli_cli_PhaseIdArgs["commands::phase::PhaseIdArgs"]
    direction TB
    T19_cli_cli_PhaseIdArgs__self[PhaseIdArgs]
  end
  subgraph T25_cli_cli_PhaseValidateArgs["commands::phase::PhaseValidateArgs"]
    direction TB
    T25_cli_cli_PhaseValidateArgs__self[PhaseValidateArgs]
  end
  subgraph T30_cli_cli_RefVerifyCheckChainArg["commands::ref_verify::RefVerifyCheckChainArg"]
    direction TB
    T30_cli_cli_RefVerifyCheckChainArg__self[RefVerifyCheckChainArg]
    T30_cli_cli_RefVerifyCheckChainArg_Chain1[Chain1]
    T30_cli_cli_RefVerifyCheckChainArg_Chain2[Chain2]
  end
  subgraph T31_cli_cli_ReviewCheckApprovedArgs["commands::review::ReviewCheckApprovedArgs"]
    direction TB
    T31_cli_cli_ReviewCheckApprovedArgs__self[ReviewCheckApprovedArgs]
  end
  subgraph T27_cli_cli_ReviewCheckRoundArg["commands::review::ReviewCheckRoundArg"]
    direction TB
    T27_cli_cli_ReviewCheckRoundArg__self[ReviewCheckRoundArg]
    T27_cli_cli_ReviewCheckRoundArg_Final[Final]
  end
  subgraph T21_cli_cli_ReviewCommand["commands::review::ReviewCommand"]
    direction TB
    T21_cli_cli_ReviewCommand__self[ReviewCommand]
    T21_cli_cli_ReviewCommand_Local[Local]
    T21_cli_cli_ReviewCommand_FixLocal[FixLocal]
    T21_cli_cli_ReviewCommand_CheckApproved[CheckApproved]
    T21_cli_cli_ReviewCommand_CheckZeroFindings[CheckZeroFindings]
    T21_cli_cli_ReviewCommand_Results[Results]
    T21_cli_cli_ReviewCommand_Classify[Classify]
    T21_cli_cli_ReviewCommand_Files[Files]
  end
  F52_cli_cli_cli__commands__capability__into_driver_input[[into_driver_input]]
  F37_cli_cli_cli__commands__phase__execute[[execute]]
  F49_cli_cli_cli__commands__phase__execute_with_driver[[execute_with_driver]]
  F48_cli_cli_cli__commands__phase__input_from_command[[input_from_command]]
  F69_cli_cli_cli__commands__ref_verify__execute_check_approved_with_driver[[execute_check_approved_with_driver]]
  F53_cli_cli_cli__commands__review__execute_check_approved[[execute_check_approved]]
  F58_cli_cli_cli__commands__review__execute_check_zero_findings[[execute_check_zero_findings]]
  end
end
T27_domain_domain_MainScopeName_new --> T28_domain_domain_ScopeNameError__self
T27_domain_domain_MainScopeName_new --> T27_domain_domain_MainScopeName__self
T23_domain_domain_ScopeName_Main --o T27_domain_domain_MainScopeName__self
T23_domain_domain_ScopeName_parse --> T28_domain_domain_ScopeNameError__self
T23_domain_domain_ScopeName_parse --> T23_domain_domain_ScopeName__self
T32_domain_domain_ContractedEntryRef_new --> T32_domain_domain_ContractedEntryRef__self
T35_domain_domain_CoverageVerifyOutcome_Blocked --o T31_domain_domain_CoverageViolation__self
T35_domain_domain_CoverageVerifyOutcome_blocked --o T31_domain_domain_CoverageViolation__self
T35_domain_domain_CoverageVerifyOutcome_blocked --> T35_domain_domain_CoverageVerifyOutcome__self
T31_domain_domain_CoverageViolation_OrphanEntry --o T32_domain_domain_ContractedEntryRef__self
T31_domain_domain_CoverageViolation_InvalidEntryRef --o T32_domain_domain_ContractedEntryRef__self
T31_domain_domain_CoverageViolation_InvalidTaskRef --o T32_domain_domain_ContractedEntryRef__self
T34_domain_domain_PreReviewGateOutcome_Blocked --o T36_domain_domain_PreReviewGateViolation__self
T34_domain_domain_PreReviewGateOutcome_blocked --o T36_domain_domain_PreReviewGateViolation__self
T34_domain_domain_PreReviewGateOutcome_blocked --> T34_domain_domain_PreReviewGateOutcome__self
T36_domain_domain_PreReviewGateViolation_NonBlueSignal --o T32_domain_domain_ContractedEntryRef__self
T34_domain_domain_TaskContractDocument_new --o T32_domain_domain_ContractedEntryRef__self
T34_domain_domain_TaskContractDocument_new --> T34_domain_domain_TaskContractDocument__self
T34_domain_domain_TaskContractDocument_entries --> T32_domain_domain_ContractedEntryRef__self
T40_usecase_usecase_CapabilityExecInteractor_new --> T40_usecase_usecase_CapabilityExecInteractor__self
R37_usecase_usecase_CapabilityExecService_execute --o T37_usecase_usecase_CapabilityExecRequest__self
T30_usecase_usecase_DiagnosticText_new --> T30_usecase_usecase_DiagnosticText__self
T31_usecase_usecase_CommandArgument_try_new --> T31_usecase_usecase_CommandArgument__self
T27_usecase_usecase_CommandArgv_try_new --o T31_usecase_usecase_CommandArgument__self
T27_usecase_usecase_CommandArgv_try_new --> T42_usecase_usecase_CommandArgvValidationError__self
T27_usecase_usecase_CommandArgv_try_new --> T27_usecase_usecase_CommandArgv__self
T27_usecase_usecase_CommandArgv_arguments --> T31_usecase_usecase_CommandArgument__self
T27_usecase_usecase_CommandArgv_with_appended_arguments --> T42_usecase_usecase_CommandArgvValidationError__self
T27_usecase_usecase_CommandArgv_with_appended_arguments --> T27_usecase_usecase_CommandArgv__self
T42_usecase_usecase_CommandArgvValidationError_RecursiveInvocation --o|prefix| T31_usecase_usecase_CommandArgument__self
T38_usecase_usecase_CommandConfigLoadError_Invalid --o T44_usecase_usecase_CommandConfigValidationError__self
T42_usecase_usecase_CommandConfigSchemaVersion_new --> T42_usecase_usecase_CommandConfigSchemaVersion__self
T44_usecase_usecase_CommandConfigValidationError_InvalidSchemaVersion --o|actual| T42_usecase_usecase_CommandConfigSchemaVersion__self
T44_usecase_usecase_CommandConfigValidationError_DuplicateDeclaration --o T36_usecase_usecase_CommandDeclarationId__self
T44_usecase_usecase_CommandConfigValidationError_DuplicateScope --o T23_domain_domain_ScopeName__self
T44_usecase_usecase_CommandConfigValidationError_TimeoutOutOfRange --o|seconds| T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T44_usecase_usecase_CommandConfigValidationError_RecursiveInvocation --o|prefix| T31_usecase_usecase_CommandArgument__self
T36_usecase_usecase_CommandDeclarationId_try_new --> T51_usecase_usecase_CommandDeclarationIdValidationError__self
T36_usecase_usecase_CommandDeclarationId_try_new --> T36_usecase_usecase_CommandDeclarationId__self
T36_usecase_usecase_CommandSequenceIndex_new --> T36_usecase_usecase_CommandSequenceIndex__self
T37_usecase_usecase_CommandTimeoutSeconds_try_new --o T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T37_usecase_usecase_CommandTimeoutSeconds_try_new --> T45_usecase_usecase_CommandTimeoutValidationError__self
T37_usecase_usecase_CommandTimeoutSeconds_try_new --> T37_usecase_usecase_CommandTimeoutSeconds__self
T37_usecase_usecase_CommandTimeoutSeconds_default_max --> T37_usecase_usecase_CommandTimeoutSeconds__self
T45_usecase_usecase_CommandTimeoutValidationError_OutOfRange --o|seconds| T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T33_usecase_usecase_ConfiguredCommand_try_new --o T31_usecase_usecase_CommandArgument__self
T33_usecase_usecase_ConfiguredCommand_try_new --o T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T33_usecase_usecase_ConfiguredCommand_try_new --> T48_usecase_usecase_ConfiguredCommandValidationError__self
T33_usecase_usecase_ConfiguredCommand_try_new --> T33_usecase_usecase_ConfiguredCommand__self
T33_usecase_usecase_ConfiguredCommand_argv --> T27_usecase_usecase_CommandArgv__self
T33_usecase_usecase_ConfiguredCommand_timeout --> T37_usecase_usecase_CommandTimeoutSeconds__self
T48_usecase_usecase_ConfiguredCommandValidationError_Argv --o T42_usecase_usecase_CommandArgvValidationError__self
T48_usecase_usecase_ConfiguredCommandValidationError_Timeout --o T45_usecase_usecase_CommandTimeoutValidationError__self
T39_usecase_usecase_OutputCaptureLimitBytes_one_mebibyte --> T39_usecase_usecase_OutputCaptureLimitBytes__self
T41_usecase_usecase_UnvalidatedTimeoutSeconds_new --> T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T34_usecase_usecase_PhaseCommandConfig_try_new --o T42_usecase_usecase_CommandConfigSchemaVersion__self
T34_usecase_usecase_PhaseCommandConfig_try_new --o T39_usecase_usecase_PhaseCommandDeclaration__self
T34_usecase_usecase_PhaseCommandConfig_try_new --> T49_usecase_usecase_PhaseCommandConfigValidationError__self
T34_usecase_usecase_PhaseCommandConfig_try_new --> T34_usecase_usecase_PhaseCommandConfig__self
T34_usecase_usecase_PhaseCommandConfig_declaration --o T36_usecase_usecase_CommandDeclarationId__self
T34_usecase_usecase_PhaseCommandConfig_declaration --> T39_usecase_usecase_PhaseCommandDeclaration__self
T49_usecase_usecase_PhaseCommandConfigValidationError_InvalidSchemaVersion --o|actual| T42_usecase_usecase_CommandConfigSchemaVersion__self
T49_usecase_usecase_PhaseCommandConfigValidationError_DuplicateDeclaration --o T36_usecase_usecase_CommandDeclarationId__self
T49_usecase_usecase_PhaseCommandConfigValidationError_into_command_config_validation_error --> T44_usecase_usecase_CommandConfigValidationError__self
T39_usecase_usecase_PhaseCommandDeclaration_new --o T36_usecase_usecase_CommandDeclarationId__self
T39_usecase_usecase_PhaseCommandDeclaration_new --o T33_usecase_usecase_ConfiguredCommand__self
T39_usecase_usecase_PhaseCommandDeclaration_new --o T33_usecase_usecase_ConfiguredCommand__self
T39_usecase_usecase_PhaseCommandDeclaration_new --> T39_usecase_usecase_PhaseCommandDeclaration__self
T39_usecase_usecase_PhaseCommandDeclaration_id --> T36_usecase_usecase_CommandDeclarationId__self
T39_usecase_usecase_PhaseCommandDeclaration_writer --> T33_usecase_usecase_ConfiguredCommand__self
T39_usecase_usecase_PhaseCommandDeclaration_pre_entry_commands --> T33_usecase_usecase_ConfiguredCommand__self
T38_usecase_usecase_PhaseCommandEnterError_Config --o T38_usecase_usecase_CommandConfigLoadError__self
T38_usecase_usecase_PhaseCommandEnterError_UnknownPhase --o T36_usecase_usecase_CommandDeclarationId__self
T38_usecase_usecase_PhaseCommandEnterError_Runner --o T34_usecase_usecase_ProgramRunnerError__self
T40_usecase_usecase_PhaseCommandEnterOutcome_Completed --o|pre_entry_records| T48_usecase_usecase_SuccessfulProgramExecutionRecord__self
T40_usecase_usecase_PhaseCommandEnterOutcome_Completed --o|writer_record| T48_usecase_usecase_SuccessfulProgramExecutionRecord__self
T40_usecase_usecase_PhaseCommandEnterOutcome_Blocked --o|completed| T48_usecase_usecase_SuccessfulProgramExecutionRecord__self
T40_usecase_usecase_PhaseCommandEnterOutcome_Blocked --o|failed| T44_usecase_usecase_FailedProgramExecutionRecord__self
T40_usecase_usecase_PhaseCommandExplainError_Config --o T38_usecase_usecase_CommandConfigLoadError__self
T40_usecase_usecase_PhaseCommandExplainError_UnknownPhase --o T36_usecase_usecase_CommandDeclarationId__self
T39_usecase_usecase_PhaseCommandExplanation__self --o|phase_id| T36_usecase_usecase_CommandDeclarationId__self
T39_usecase_usecase_PhaseCommandExplanation__self --o|pre_entry_commands| T33_usecase_usecase_ConfiguredCommand__self
T39_usecase_usecase_PhaseCommandExplanation__self --o|writer| T33_usecase_usecase_ConfiguredCommand__self
T39_usecase_usecase_PhaseCommandExplanation__self --o|output_limit| T39_usecase_usecase_OutputCaptureLimitBytes__self
T38_usecase_usecase_PhaseCommandInteractor_new --o R44_usecase_usecase_PhaseCommandConfigLoaderPort__self
T38_usecase_usecase_PhaseCommandInteractor_new --o R33_usecase_usecase_ProgramRunnerPort__self
T38_usecase_usecase_PhaseCommandInteractor_new --> T38_usecase_usecase_PhaseCommandInteractor__self
T33_usecase_usecase_PhaseEnterCommand__self --o|phase_id| T36_usecase_usecase_CommandDeclarationId__self
T33_usecase_usecase_PhaseExplainQuery__self --o|phase_id| T36_usecase_usecase_CommandDeclarationId__self
R44_usecase_usecase_PhaseCommandConfigLoaderPort_load --> T38_usecase_usecase_CommandConfigLoadError__self
R44_usecase_usecase_PhaseCommandConfigLoaderPort_load --> T34_usecase_usecase_PhaseCommandConfig__self
R35_usecase_usecase_PhaseCommandService_validate --o T36_usecase_usecase_PhaseValidateCommand__self
R35_usecase_usecase_PhaseCommandService_validate --> T38_usecase_usecase_CommandConfigLoadError__self
R35_usecase_usecase_PhaseCommandService_explain --o T33_usecase_usecase_PhaseExplainQuery__self
R35_usecase_usecase_PhaseCommandService_explain --> T40_usecase_usecase_PhaseCommandExplainError__self
R35_usecase_usecase_PhaseCommandService_explain --> T39_usecase_usecase_PhaseCommandExplanation__self
R35_usecase_usecase_PhaseCommandService_enter --o T33_usecase_usecase_PhaseEnterCommand__self
R35_usecase_usecase_PhaseCommandService_enter --> T38_usecase_usecase_PhaseCommandEnterError__self
R35_usecase_usecase_PhaseCommandService_enter --> T40_usecase_usecase_PhaseCommandEnterOutcome__self
T38_usecase_usecase_PreReviewCommandConfig_try_new --o T42_usecase_usecase_CommandConfigSchemaVersion__self
T38_usecase_usecase_PreReviewCommandConfig_try_new --o T48_usecase_usecase_PreReviewScopeCommandDeclaration__self
T38_usecase_usecase_PreReviewCommandConfig_try_new --> T53_usecase_usecase_PreReviewCommandConfigValidationError__self
T38_usecase_usecase_PreReviewCommandConfig_try_new --> T38_usecase_usecase_PreReviewCommandConfig__self
T38_usecase_usecase_PreReviewCommandConfig_commands_for --o T23_domain_domain_ScopeName__self
T38_usecase_usecase_PreReviewCommandConfig_commands_for --> T33_usecase_usecase_ConfiguredCommand__self
T53_usecase_usecase_PreReviewCommandConfigValidationError_InvalidSchemaVersion --o|actual| T42_usecase_usecase_CommandConfigSchemaVersion__self
T53_usecase_usecase_PreReviewCommandConfigValidationError_DuplicateScope --o T23_domain_domain_ScopeName__self
T47_usecase_usecase_PreReviewCommandDispatchCommand__self --o|track| T35_usecase_usecase_ReviewTrackSelector__self
T47_usecase_usecase_PreReviewCommandDispatchCommand__self --o|scope| T35_usecase_usecase_ReviewScopeSelector__self
T45_usecase_usecase_PreReviewCommandDispatchError_Config --o T38_usecase_usecase_CommandConfigLoadError__self
T45_usecase_usecase_PreReviewCommandDispatchError_UnknownScope --o T23_domain_domain_ScopeName__self
T45_usecase_usecase_PreReviewCommandDispatchError_TrackResolution --o T46_usecase_usecase_CurrentReviewTrackResolveError__self
T45_usecase_usecase_PreReviewCommandDispatchError_Runner --o T34_usecase_usecase_ProgramRunnerError__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --o R48_usecase_usecase_PreReviewCommandConfigLoaderPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --o R46_usecase_usecase_CurrentReviewTrackResolverPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --o R33_usecase_usecase_ProgramRunnerPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --> T50_usecase_usecase_PreReviewCommandDispatchInteractor__self
T47_usecase_usecase_PreReviewCommandDispatchOutcome_ReadyForReview --o|records| T48_usecase_usecase_SuccessfulProgramExecutionRecord__self
T47_usecase_usecase_PreReviewCommandDispatchOutcome_Blocked --o|completed| T48_usecase_usecase_SuccessfulProgramExecutionRecord__self
T47_usecase_usecase_PreReviewCommandDispatchOutcome_Blocked --o|failed| T44_usecase_usecase_FailedProgramExecutionRecord__self
T53_usecase_usecase_PreReviewCommandGatedReviewInteractor_new --o R29_usecase_usecase_ReviewService__self
T53_usecase_usecase_PreReviewCommandGatedReviewInteractor_new --o R47_usecase_usecase_PreReviewCommandDispatchService__self
T53_usecase_usecase_PreReviewCommandGatedReviewInteractor_new --> T53_usecase_usecase_PreReviewCommandGatedReviewInteractor__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_new --o T23_domain_domain_ScopeName__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_new --o T33_usecase_usecase_ConfiguredCommand__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_new --> T48_usecase_usecase_PreReviewScopeCommandDeclaration__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_scope --> T23_domain_domain_ScopeName__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_commands --> T33_usecase_usecase_ConfiguredCommand__self
T35_usecase_usecase_ReviewScopeSelector_Named --o T27_domain_domain_MainScopeName__self
R46_usecase_usecase_CurrentReviewTrackResolverPort_resolve --> T46_usecase_usecase_CurrentReviewTrackResolveError__self
R48_usecase_usecase_PreReviewCommandConfigLoaderPort_load --> T38_usecase_usecase_CommandConfigLoadError__self
R48_usecase_usecase_PreReviewCommandConfigLoaderPort_load --> T38_usecase_usecase_PreReviewCommandConfig__self
R47_usecase_usecase_PreReviewCommandDispatchService_dispatch --o T47_usecase_usecase_PreReviewCommandDispatchCommand__self
R47_usecase_usecase_PreReviewCommandDispatchService_dispatch --> T45_usecase_usecase_PreReviewCommandDispatchError__self
R47_usecase_usecase_PreReviewCommandDispatchService_dispatch --> T47_usecase_usecase_PreReviewCommandDispatchOutcome__self
T48_usecase_usecase_ClassifiedProgramExecutionRecord_Succeeded --o T48_usecase_usecase_SuccessfulProgramExecutionRecord__self
T48_usecase_usecase_ClassifiedProgramExecutionRecord_Failed --o T44_usecase_usecase_FailedProgramExecutionRecord__self
T38_usecase_usecase_ProgramExecutionRecord_classify --> T48_usecase_usecase_ClassifiedProgramExecutionRecord__self
T38_usecase_usecase_ProgramExecutionRecord__self --o|sequence_index| T36_usecase_usecase_CommandSequenceIndex__self
T38_usecase_usecase_ProgramExecutionRecord__self --o|command| T33_usecase_usecase_ConfiguredCommand__self
T38_usecase_usecase_ProgramExecutionRecord__self --o|invoked_argv| T27_usecase_usecase_CommandArgv__self
T38_usecase_usecase_ProgramExecutionRecord__self --o|outcome| T33_usecase_usecase_ProgramRunOutcome__self
T31_usecase_usecase_ProgramExitCode_new --> T31_usecase_usecase_ProgramExitCode__self
T33_usecase_usecase_ProgramInvocation__self --o|argv| T27_usecase_usecase_CommandArgv__self
T33_usecase_usecase_ProgramInvocation__self --o|timeout| T37_usecase_usecase_CommandTimeoutSeconds__self
T33_usecase_usecase_ProgramInvocation__self --o|stdout_limit| T39_usecase_usecase_OutputCaptureLimitBytes__self
T33_usecase_usecase_ProgramInvocation__self --o|stderr_limit| T39_usecase_usecase_OutputCaptureLimitBytes__self
T33_usecase_usecase_ProgramRunOutcome_Exited --o|exit_code| T31_usecase_usecase_ProgramExitCode__self
T33_usecase_usecase_ProgramRunOutcome_Exited --o|output| T37_usecase_usecase_CapturedProgramOutput__self
T33_usecase_usecase_ProgramRunOutcome_TimedOut --o|output| T37_usecase_usecase_CapturedProgramOutput__self
T33_usecase_usecase_ProgramRunOutcome_OutputLimitExceeded --o|stream| T35_usecase_usecase_ProgramOutputStream__self
T33_usecase_usecase_ProgramRunOutcome_OutputLimitExceeded --o|output| T37_usecase_usecase_CapturedProgramOutput__self
R33_usecase_usecase_ProgramRunnerPort_run --o T33_usecase_usecase_ProgramInvocation__self
R33_usecase_usecase_ProgramRunnerPort_run --> T33_usecase_usecase_ProgramRunOutcome__self
R33_usecase_usecase_ProgramRunnerPort_run --> T34_usecase_usecase_ProgramRunnerError__self
R41_usecase_usecase_RefVerifyAggregateService_results --o T36_usecase_usecase_RefVerifyChainFilter__self
R51_usecase_usecase_RefVerifyCheckApprovedDriverService_check_approved --o T36_usecase_usecase_RefVerifyChainFilter__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new --o T37_usecase_usecase_ReviewerFindingOutput__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new --> T51_usecase_usecase_ReviewFindingsOutputValidationError__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new --> T46_usecase_usecase_NonEmptyReviewerFindingsOutput__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_as_slice --> T37_usecase_usecase_ReviewerFindingOutput__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new --o T37_usecase_usecase_ReviewerFindingOutput__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new --> T51_usecase_usecase_ReviewFindingsOutputValidationError__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new --> T46_usecase_usecase_NonEmptyReviewerFindingsOutput__self
T46_usecase_usecase_NonEmptyReviewerFindingsOutput_as_slice --> T37_usecase_usecase_ReviewerFindingOutput__self
T54_usecase_usecase_ReviewCheckZeroFindingsEvaluationError_EvaluationFailed --o T30_usecase_usecase_DiagnosticText__self
T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new --o R48_usecase_usecase_ReviewCheckZeroFindingsStatePort__self
T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new --> T49_usecase_usecase_ReviewCheckZeroFindingsInteractor__self
T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new --o R48_usecase_usecase_ReviewCheckZeroFindingsStatePort__self
T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new --> T49_usecase_usecase_ReviewCheckZeroFindingsInteractor__self
T44_usecase_usecase_ReviewCheckZeroFindingsQuery_try_new --> T54_usecase_usecase_ReviewCheckZeroFindingsValidationError__self
T44_usecase_usecase_ReviewCheckZeroFindingsQuery_try_new --> T44_usecase_usecase_ReviewCheckZeroFindingsQuery__self
T54_usecase_usecase_ReviewCheckZeroFindingsValidationError_InvalidTrackId --o T30_usecase_usecase_DiagnosticText__self
T54_usecase_usecase_ReviewCheckZeroFindingsValidationError_InvalidScope --o T30_usecase_usecase_DiagnosticText__self
T35_usecase_usecase_ReviewFixResolution_new --o T29_usecase_usecase_ReviewTrackId__self
T35_usecase_usecase_ReviewFixResolution_new --> T35_usecase_usecase_ReviewFixResolution__self
T35_usecase_usecase_ReviewFixResolution_track_id --> T29_usecase_usecase_ReviewTrackId__self
T35_usecase_usecase_ReviewFixResolution_new --o T29_usecase_usecase_ReviewTrackId__self
T35_usecase_usecase_ReviewFixResolution_new --> T35_usecase_usecase_ReviewFixResolution__self
T35_usecase_usecase_ReviewFixResolution_track_id --> T29_usecase_usecase_ReviewTrackId__self
T36_usecase_usecase_ReviewFixRunnerError_SmokeTestFailed --o T30_usecase_usecase_DiagnosticText__self
T36_usecase_usecase_ReviewFixRunnerError_SpawnFailed --o T30_usecase_usecase_DiagnosticText__self
T36_usecase_usecase_ReviewFixRunnerError_SentinelNotFound --o T30_usecase_usecase_DiagnosticText__self
T36_usecase_usecase_ReviewFixRunnerError_SubagentDispatchRequired --o T43_usecase_usecase_SubagentDispatchInstruction__self
T36_usecase_usecase_ReviewFixRunnerError_Unexpected --o T30_usecase_usecase_DiagnosticText__self
T42_usecase_usecase_ReviewFixTrackResolveError_BranchReadFailed --o T30_usecase_usecase_DiagnosticText__self
T42_usecase_usecase_ReviewFixTrackResolveError_NonTrackBranch --o T30_usecase_usecase_DiagnosticText__self
T34_usecase_usecase_ReviewResultsError_Failed --o T30_usecase_usecase_DiagnosticText__self
T34_usecase_usecase_ReviewResultsError_UnknownScope --o T31_usecase_usecase_ReviewScopeName__self
T34_usecase_usecase_ReviewResultsError_MissingScopeState --o T31_usecase_usecase_ReviewScopeName__self
T39_usecase_usecase_ReviewResultsInteractor_new --o R38_usecase_usecase_ReviewResultsScopePort__self
T39_usecase_usecase_ReviewResultsInteractor_new --o R38_usecase_usecase_ReviewResultsStatePort__self
T39_usecase_usecase_ReviewResultsInteractor_new --o R38_usecase_usecase_ReviewResultsRoundPort__self
T39_usecase_usecase_ReviewResultsInteractor_new --> T39_usecase_usecase_ReviewResultsInteractor__self
T39_usecase_usecase_ReviewResultsInteractor_new --o R38_usecase_usecase_ReviewResultsScopePort__self
T39_usecase_usecase_ReviewResultsInteractor_new --o R38_usecase_usecase_ReviewResultsStatePort__self
T39_usecase_usecase_ReviewResultsInteractor_new --o R38_usecase_usecase_ReviewResultsRoundPort__self
T39_usecase_usecase_ReviewResultsInteractor_new --> T39_usecase_usecase_ReviewResultsInteractor__self
T35_usecase_usecase_ReviewResultsOutput__self --o|scopes| T39_usecase_usecase_ReviewScopeResultOutput__self
T42_usecase_usecase_ReviewResultsScopeSnapshot__self --o|configured_scopes| T31_usecase_usecase_ReviewScopeName__self
T39_usecase_usecase_ReviewRoundResultOutput__self --o|verdict| T40_usecase_usecase_ReviewRoundResultVerdict__self
T40_usecase_usecase_ReviewRoundResultVerdict_FindingsRemain --o T46_usecase_usecase_NonEmptyReviewerFindingsOutput__self
T36_usecase_usecase_ReviewRunLocalOutput__self --o|diagnostics| T30_usecase_usecase_DiagnosticText__self
T31_usecase_usecase_ReviewScopeName_try_new --> T46_usecase_usecase_ReviewScopeNameValidationError__self
T31_usecase_usecase_ReviewScopeName_try_new --> T31_usecase_usecase_ReviewScopeName__self
T31_usecase_usecase_ReviewScopeName_other --> T31_usecase_usecase_ReviewScopeName__self
T31_usecase_usecase_ReviewScopeName_try_new --> T46_usecase_usecase_ReviewScopeNameValidationError__self
T31_usecase_usecase_ReviewScopeName_try_new --> T31_usecase_usecase_ReviewScopeName__self
T31_usecase_usecase_ReviewScopeName_other --> T31_usecase_usecase_ReviewScopeName__self
T46_usecase_usecase_ReviewScopeNameValidationError_Invalid --o T30_usecase_usecase_DiagnosticText__self
T39_usecase_usecase_ReviewScopeResultOutput__self --o|scope| T31_usecase_usecase_ReviewScopeName__self
T39_usecase_usecase_ReviewScopeResultOutput__self --o|state| T38_usecase_usecase_ReviewScopeResultState__self
T39_usecase_usecase_ReviewScopeResultOutput__self --o|rounds| T39_usecase_usecase_ReviewRoundResultOutput__self
T43_usecase_usecase_ReviewScopeSelectionRequest_NamedCandidate --o T31_usecase_usecase_ReviewScopeName__self
T43_usecase_usecase_ReviewScopeSelectionRequest_try_new --> T51_usecase_usecase_ReviewScopeSelectionValidationError__self
T43_usecase_usecase_ReviewScopeSelectionRequest_try_new --> T43_usecase_usecase_ReviewScopeSelectionRequest__self
T43_usecase_usecase_ReviewScopeSelectionRequest_try_new --> T51_usecase_usecase_ReviewScopeSelectionValidationError__self
T43_usecase_usecase_ReviewScopeSelectionRequest_try_new --> T43_usecase_usecase_ReviewScopeSelectionRequest__self
T51_usecase_usecase_ReviewScopeSelectionValidationError_InvalidScope --o T30_usecase_usecase_DiagnosticText__self
T33_usecase_usecase_ReviewStoredRound__self ---|alias_of| T39_usecase_usecase_ReviewRoundResultOutput__self
T40_usecase_usecase_ReviewStoredRoundVerdict__self ---|alias_of| T40_usecase_usecase_ReviewRoundResultVerdict__self
T38_usecase_usecase_ReviewStoredScopeState_Required --o T36_usecase_usecase_ReviewRequiredReason__self
T38_usecase_usecase_ReviewStoredScopeState_NotRequired --o T39_usecase_usecase_ReviewNotRequiredReason__self
T43_usecase_usecase_ReviewStoredScopeStateEntry__self --o|scope| T31_usecase_usecase_ReviewScopeName__self
T43_usecase_usecase_ReviewStoredScopeStateEntry__self --o|state| T38_usecase_usecase_ReviewStoredScopeState__self
T29_usecase_usecase_ReviewTrackId_try_new --> T44_usecase_usecase_ReviewTrackIdValidationError__self
T29_usecase_usecase_ReviewTrackId_try_new --> T29_usecase_usecase_ReviewTrackId__self
T29_usecase_usecase_ReviewTrackId_try_new --> T44_usecase_usecase_ReviewTrackIdValidationError__self
T29_usecase_usecase_ReviewTrackId_try_new --> T29_usecase_usecase_ReviewTrackId__self
T44_usecase_usecase_ReviewTrackIdValidationError_Invalid --o T30_usecase_usecase_DiagnosticText__self
T37_usecase_usecase_ReviewerFindingOutput__self --o|message| T30_usecase_usecase_DiagnosticText__self
T35_usecase_usecase_RunReviewFixCommand_new_resolved --o T31_usecase_usecase_ReviewScopeName__self
T35_usecase_usecase_RunReviewFixCommand_new_resolved --o T35_usecase_usecase_ReviewFixResolution__self
T35_usecase_usecase_RunReviewFixCommand_new_resolved --> T35_usecase_usecase_RunReviewFixCommand__self
T35_usecase_usecase_RunReviewFixCommand_new_resolved --o T31_usecase_usecase_ReviewScopeName__self
T35_usecase_usecase_RunReviewFixCommand_new_resolved --o T35_usecase_usecase_ReviewFixResolution__self
T35_usecase_usecase_RunReviewFixCommand_new_resolved --> T35_usecase_usecase_RunReviewFixCommand__self
T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidScope --o T30_usecase_usecase_DiagnosticText__self
T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidTrackId --o T30_usecase_usecase_DiagnosticText__self
T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidRoundType --o T30_usecase_usecase_DiagnosticText__self
T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidModel --o T30_usecase_usecase_DiagnosticText__self
T33_usecase_usecase_RunReviewFixError_FixRunnerFailed --o T36_usecase_usecase_ReviewFixRunnerError__self
T33_usecase_usecase_RunReviewFixError_TrackResolution --o T42_usecase_usecase_ReviewFixTrackResolveError__self
T33_usecase_usecase_RunReviewFixError_TrackMismatch --o|explicit| T29_usecase_usecase_ReviewTrackId__self
T33_usecase_usecase_RunReviewFixError_TrackMismatch --o|resolved| T29_usecase_usecase_ReviewTrackId__self
T38_usecase_usecase_RunReviewFixInteractor_new --o R42_usecase_usecase_ReviewFixTrackResolverPort__self
T38_usecase_usecase_RunReviewFixInteractor_new --o R31_usecase_usecase_ReviewFixRunner__self
T38_usecase_usecase_RunReviewFixInteractor_new --> T38_usecase_usecase_RunReviewFixInteractor__self
T38_usecase_usecase_RunReviewFixInteractor_new --o R42_usecase_usecase_ReviewFixTrackResolverPort__self
T38_usecase_usecase_RunReviewFixInteractor_new --o R31_usecase_usecase_ReviewFixRunner__self
T38_usecase_usecase_RunReviewFixInteractor_new --> T38_usecase_usecase_RunReviewFixInteractor__self
T35_usecase_usecase_RunReviewFixRequest_try_new --> T50_usecase_usecase_RunReviewFixCommandValidationError__self
T35_usecase_usecase_RunReviewFixRequest_try_new --> T35_usecase_usecase_RunReviewFixRequest__self
T35_usecase_usecase_RunReviewFixRequest_try_new --> T50_usecase_usecase_RunReviewFixCommandValidationError__self
T35_usecase_usecase_RunReviewFixRequest_try_new --> T35_usecase_usecase_RunReviewFixRequest__self
T43_usecase_usecase_SubagentDispatchInstruction__self --o|agent| T28_usecase_usecase_SubagentName__self
T43_usecase_usecase_SubagentDispatchInstruction__self --o|scope| T31_usecase_usecase_ReviewScopeName__self
T43_usecase_usecase_SubagentDispatchInstruction__self --o|track_id| T29_usecase_usecase_ReviewTrackId__self
T28_usecase_usecase_SubagentName_try_new --> T28_usecase_usecase_SubagentName__self
T28_usecase_usecase_SubagentName_try_new --> T43_usecase_usecase_SubagentNameValidationError__self
T28_usecase_usecase_SubagentName_try_new --> T28_usecase_usecase_SubagentName__self
T28_usecase_usecase_SubagentName_try_new --> T43_usecase_usecase_SubagentNameValidationError__self
T43_usecase_usecase_SubagentNameValidationError_Invalid --o T30_usecase_usecase_DiagnosticText__self
R46_usecase_usecase_ReviewCheckZeroFindingsService_check_zero_findings --o T44_usecase_usecase_ReviewCheckZeroFindingsQuery__self
R46_usecase_usecase_ReviewCheckZeroFindingsService_check_zero_findings --> T54_usecase_usecase_ReviewCheckZeroFindingsEvaluationError__self
R46_usecase_usecase_ReviewCheckZeroFindingsService_check_zero_findings --> T46_usecase_usecase_ReviewCheckZeroFindingsOutcome__self
R48_usecase_usecase_ReviewCheckZeroFindingsStatePort_state_for --o T23_domain_domain_ScopeName__self
R31_usecase_usecase_ReviewFixRunner_run_fix --o T35_usecase_usecase_RunReviewFixCommand__self
R31_usecase_usecase_ReviewFixRunner_run_fix --> T36_usecase_usecase_ReviewFixRunnerError__self
R31_usecase_usecase_ReviewFixRunner_run_fix --> T34_usecase_usecase_RunReviewFixOutput__self
R42_usecase_usecase_ReviewFixTrackResolverPort_resolve_current_track --> T35_usecase_usecase_ReviewFixResolution__self
R42_usecase_usecase_ReviewFixTrackResolverPort_resolve_current_track --> T42_usecase_usecase_ReviewFixTrackResolveError__self
R38_usecase_usecase_ReviewResultsRoundPort_load_scope_rounds --o T31_usecase_usecase_ReviewScopeName__self
R38_usecase_usecase_ReviewResultsRoundPort_load_scope_rounds --> T34_usecase_usecase_ReviewResultsError__self
R38_usecase_usecase_ReviewResultsRoundPort_load_scope_rounds --> T33_usecase_usecase_ReviewStoredRound__self
R38_usecase_usecase_ReviewResultsScopePort_load_scope_snapshot --> T34_usecase_usecase_ReviewResultsError__self
R38_usecase_usecase_ReviewResultsScopePort_load_scope_snapshot --> T42_usecase_usecase_ReviewResultsScopeSnapshot__self
R36_usecase_usecase_ReviewResultsService_results --o T43_usecase_usecase_ReviewScopeSelectionRequest__self
R36_usecase_usecase_ReviewResultsService_results --> T34_usecase_usecase_ReviewResultsError__self
R36_usecase_usecase_ReviewResultsService_results --> T35_usecase_usecase_ReviewResultsOutput__self
R38_usecase_usecase_ReviewResultsStatePort_load_scope_states --> T34_usecase_usecase_ReviewResultsError__self
R38_usecase_usecase_ReviewResultsStatePort_load_scope_states --> T43_usecase_usecase_ReviewStoredScopeStateEntry__self
R37_usecase_usecase_ReviewRunLocalService_run_local --> T36_usecase_usecase_ReviewRunLocalOutput__self
R29_usecase_usecase_ReviewService_run_local --> T36_usecase_usecase_ReviewRunLocalOutput__self
R35_usecase_usecase_RunReviewFixService_run --o T35_usecase_usecase_RunReviewFixRequest__self
R35_usecase_usecase_RunReviewFixService_run --> T33_usecase_usecase_RunReviewFixError__self
R35_usecase_usecase_RunReviewFixService_run --> T34_usecase_usecase_RunReviewFixOutput__self
T40_usecase_usecase_CapabilityExecInteractor__self -.impl.-> R37_usecase_usecase_CapabilityExecService__self
T38_usecase_usecase_PhaseCommandInteractor__self -.impl.-> R35_usecase_usecase_PhaseCommandService__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor__self -.impl.-> R47_usecase_usecase_PreReviewCommandDispatchService__self
T53_usecase_usecase_PreReviewCommandGatedReviewInteractor__self -.impl.-> R29_usecase_usecase_ReviewService__self
T49_usecase_usecase_ReviewCheckZeroFindingsInteractor__self -.impl.-> R46_usecase_usecase_ReviewCheckZeroFindingsService__self
T39_usecase_usecase_ReviewResultsInteractor__self -.impl.-> R36_usecase_usecase_ReviewResultsService__self
T38_usecase_usecase_RunReviewFixInteractor__self -.impl.-> R35_usecase_usecase_RunReviewFixService__self
T44_infrastructure_infrastructure_CommandArgvDto__self --o|arguments| T48_infrastructure_infrastructure_CommandArgumentDto__self
T50_infrastructure_infrastructure_ConfiguredCommandDto__self --o|argv| T44_infrastructure_infrastructure_CommandArgvDto__self
T50_infrastructure_infrastructure_ConfiguredCommandDto__self --o|timeout_seconds| T54_infrastructure_infrastructure_CommandTimeoutSecondsDto__self
T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader_new --> T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader__self
T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader_new --> T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader__self
T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver_new --> T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver__self
T51_infrastructure_infrastructure_PhaseCommandConfigDto__self --o|schema_version| T59_infrastructure_infrastructure_CommandConfigSchemaVersionDto__self
T51_infrastructure_infrastructure_PhaseCommandConfigDto__self --o|phases| T56_infrastructure_infrastructure_PhaseCommandDeclarationDto__self
T56_infrastructure_infrastructure_PhaseCommandDeclarationDto__self --o|id| T53_infrastructure_infrastructure_CommandDeclarationIdDto__self
T56_infrastructure_infrastructure_PhaseCommandDeclarationDto__self --o|writer| T50_infrastructure_infrastructure_ConfiguredCommandDto__self
T56_infrastructure_infrastructure_PhaseCommandDeclarationDto__self --o|pre_entry_commands| T50_infrastructure_infrastructure_ConfiguredCommandDto__self
T55_infrastructure_infrastructure_PreReviewCommandConfigDto__self --o|schema_version| T59_infrastructure_infrastructure_CommandConfigSchemaVersionDto__self
T55_infrastructure_infrastructure_PreReviewCommandConfigDto__self --o|scopes| T65_infrastructure_infrastructure_PreReviewScopeCommandDeclarationDto__self
T65_infrastructure_infrastructure_PreReviewScopeCommandDeclarationDto__self --o|scope| T48_infrastructure_infrastructure_ReviewScopeNameDto__self
T65_infrastructure_infrastructure_PreReviewScopeCommandDeclarationDto__self --o|commands| T50_infrastructure_infrastructure_ConfiguredCommandDto__self
F98_infrastructure_infrastructure_infrastructure__operator_command_config__decode_phase_command_config --o T51_infrastructure_infrastructure_PhaseCommandConfigDto__self
F98_infrastructure_infrastructure_infrastructure__operator_command_config__decode_phase_command_config --> T44_usecase_usecase_CommandConfigValidationError__self
F98_infrastructure_infrastructure_infrastructure__operator_command_config__decode_phase_command_config --> T34_usecase_usecase_PhaseCommandConfig__self
F103_infrastructure_infrastructure_infrastructure__operator_command_config__decode_pre_review_command_config --o T55_infrastructure_infrastructure_PreReviewCommandConfigDto__self
F103_infrastructure_infrastructure_infrastructure__operator_command_config__decode_pre_review_command_config --> T44_usecase_usecase_CommandConfigValidationError__self
F103_infrastructure_infrastructure_infrastructure__operator_command_config__decode_pre_review_command_config --> T38_usecase_usecase_PreReviewCommandConfig__self
T50_infrastructure_infrastructure_ProcessProgramRunner_new --> T50_infrastructure_infrastructure_ProcessProgramRunner__self
T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter_new --> T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self
T50_infrastructure_infrastructure_CodexReviewFixRunner_new --> T50_infrastructure_infrastructure_CodexReviewFixRunner__self
T51_infrastructure_infrastructure_ContractedEntryRefDto__self --o|layer| T40_infrastructure_infrastructure_LayerIdDto__self
T51_infrastructure_infrastructure_ContractedEntryRefDto__self --o|entry_key| T41_infrastructure_infrastructure_EntryKeyDto__self
T53_infrastructure_infrastructure_TaskContractDocumentDto__self --o|schema_version| T58_infrastructure_infrastructure_TaskContractSchemaVersionDto__self
T53_infrastructure_infrastructure_TaskContractDocumentDto__self --o|track_id| T40_infrastructure_infrastructure_TrackIdDto__self
T53_infrastructure_infrastructure_TaskContractDocumentDto__self --o|entries| T51_infrastructure_infrastructure_ContractedEntryRefDto__self
T53_infrastructure_infrastructure_TaskContractDocumentDto__self --o|entries| T39_infrastructure_infrastructure_TaskIdDto__self
T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader__self -.impl.-> R44_usecase_usecase_PhaseCommandConfigLoaderPort__self
T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader__self -.impl.-> R48_usecase_usecase_PreReviewCommandConfigLoaderPort__self
T50_infrastructure_infrastructure_ProcessProgramRunner__self -.impl.-> R33_usecase_usecase_ProgramRunnerPort__self
T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver__self -.impl.-> R46_usecase_usecase_CurrentReviewTrackResolverPort__self
T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self -.impl.-> R41_usecase_usecase_RefVerifyAggregateService__self
T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self -.impl.-> R51_usecase_usecase_RefVerifyCheckApprovedDriverService__self
T65_infrastructure_infrastructure_ReviewCheckZeroFindingsStateAdapter__self -.impl.-> R48_usecase_usecase_ReviewCheckZeroFindingsStatePort__self
T49_infrastructure_infrastructure_ResultsScopeAdapter__self -.impl.-> R38_usecase_usecase_ReviewResultsScopePort__self
T49_infrastructure_infrastructure_ResultsStateAdapter__self -.impl.-> R38_usecase_usecase_ReviewResultsStatePort__self
T49_infrastructure_infrastructure_ResultsRoundAdapter__self -.impl.-> R38_usecase_usecase_ReviewResultsRoundPort__self
T52_infrastructure_infrastructure_ReviewFixRunnerAdapter__self -.impl.-> R31_usecase_usecase_ReviewFixRunner__self
T55_infrastructure_infrastructure_GitReviewFixTrackResolver__self -.impl.-> R42_usecase_usecase_ReviewFixTrackResolverPort__self
T50_infrastructure_infrastructure_CodexReviewFixRunner__self -.impl.-> R31_usecase_usecase_ReviewFixRunner__self
T38_cli_driver_cli_driver_CapabilityDriver_new --o R37_usecase_usecase_CapabilityExecService__self
T38_cli_driver_cli_driver_CapabilityDriver_new --> T38_cli_driver_cli_driver_CapabilityDriver__self
T38_cli_driver_cli_driver_CapabilityDriver_handle --o T47_cli_driver_cli_driver_CapabilityExecDriverInput__self
T38_cli_driver_cli_driver_CapabilityDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T40_cli_driver_cli_driver_PhaseCommandDriver_new --o R35_usecase_usecase_PhaseCommandService__self
T40_cli_driver_cli_driver_PhaseCommandDriver_new --> T40_cli_driver_cli_driver_PhaseCommandDriver__self
T40_cli_driver_cli_driver_PhaseCommandDriver_handle --o T39_cli_driver_cli_driver_PhaseCommandInput__self
T40_cli_driver_cli_driver_PhaseCommandDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T39_cli_driver_cli_driver_PhaseCommandInput_Explain --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
T39_cli_driver_cli_driver_PhaseCommandInput_Enter --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
T32_cli_driver_cli_driver_PhaseIdArg_new --o T36_usecase_usecase_CommandDeclarationId__self
T32_cli_driver_cli_driver_PhaseIdArg_new --> T32_cli_driver_cli_driver_PhaseIdArg__self
T32_cli_driver_cli_driver_PhaseIdArg_as_declaration_id --> T36_usecase_usecase_CommandDeclarationId__self
T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput__self --o|chain| T42_cli_driver_cli_driver_RefVerifyChainSelect__self
T37_cli_driver_cli_driver_RefVerifyDriver_new --o R41_usecase_usecase_RefVerifyAggregateService__self
T37_cli_driver_cli_driver_RefVerifyDriver_new --> T37_cli_driver_cli_driver_RefVerifyDriver__self
T37_cli_driver_cli_driver_RefVerifyDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_CommandOutcome_success --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_CommandOutcome_failure --> T36_cli_driver_cli_driver_CommandOutcome__self
T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_try_new --o T44_cli_driver_cli_driver_ReviewCheckRoundSelect__self
T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_try_new --> T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput__self
T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_try_new --> T54_usecase_usecase_ReviewCheckZeroFindingsValidationError__self
T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_into_query --> T44_usecase_usecase_ReviewCheckZeroFindingsQuery__self
T34_cli_driver_cli_driver_ReviewDriver_new --o R29_usecase_usecase_ReviewService__self
T34_cli_driver_cli_driver_ReviewDriver_new --o R36_usecase_usecase_ReviewResultsService__self
T34_cli_driver_cli_driver_ReviewDriver_new --o R46_usecase_usecase_ReviewCheckZeroFindingsService__self
T34_cli_driver_cli_driver_ReviewDriver_new --> T34_cli_driver_cli_driver_ReviewDriver__self
T34_cli_driver_cli_driver_ReviewDriver_handle --o T33_cli_driver_cli_driver_ReviewInput__self
T34_cli_driver_cli_driver_ReviewDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T34_cli_driver_cli_driver_ReviewDriver_new --o R29_usecase_usecase_ReviewService__self
T34_cli_driver_cli_driver_ReviewDriver_new --o R36_usecase_usecase_ReviewResultsService__self
T34_cli_driver_cli_driver_ReviewDriver_new --o R46_usecase_usecase_ReviewCheckZeroFindingsService__self
T34_cli_driver_cli_driver_ReviewDriver_new --> T34_cli_driver_cli_driver_ReviewDriver__self
T34_cli_driver_cli_driver_ReviewDriver_handle --o T33_cli_driver_cli_driver_ReviewInput__self
T34_cli_driver_cli_driver_ReviewDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T37_cli_driver_cli_driver_ReviewFixDriver_new --o R35_usecase_usecase_RunReviewFixService__self
T37_cli_driver_cli_driver_ReviewFixDriver_new --> T37_cli_driver_cli_driver_ReviewFixDriver__self
T37_cli_driver_cli_driver_ReviewFixDriver_handle --o T36_cli_driver_cli_driver_ReviewFixInput__self
T37_cli_driver_cli_driver_ReviewFixDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T37_cli_driver_cli_driver_ReviewFixDriver_new --o R35_usecase_usecase_RunReviewFixService__self
T37_cli_driver_cli_driver_ReviewFixDriver_new --> T37_cli_driver_cli_driver_ReviewFixDriver__self
T37_cli_driver_cli_driver_ReviewFixDriver_handle --o T36_cli_driver_cli_driver_ReviewFixInput__self
T37_cli_driver_cli_driver_ReviewFixDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_ReviewFixInput_new --> T36_cli_driver_cli_driver_ReviewFixInput__self
T36_cli_driver_cli_driver_ReviewFixInput_new --> T36_cli_driver_cli_driver_ReviewFixInput__self
T33_cli_driver_cli_driver_ReviewInput_CheckZeroFindings --o T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput__self
T33_cli_driver_cli_driver_ReviewInput_Results --o T40_cli_driver_cli_driver_ReviewResultsInput__self
T40_cli_driver_cli_driver_ReviewResultsInput_try_new --> T40_cli_driver_cli_driver_ReviewResultsInput__self
T40_cli_driver_cli_driver_ReviewResultsInput_try_new --> T51_usecase_usecase_ReviewScopeSelectionValidationError__self
T40_cli_driver_cli_driver_ReviewResultsInput_into_parts --> T43_usecase_usecase_ReviewScopeSelectionRequest__self
T40_cli_driver_cli_driver_ReviewResultsInput_try_new --> T40_cli_driver_cli_driver_ReviewResultsInput__self
T40_cli_driver_cli_driver_ReviewResultsInput_try_new --> T51_usecase_usecase_ReviewScopeSelectionValidationError__self
T40_cli_driver_cli_driver_ReviewResultsInput_into_parts --> T43_usecase_usecase_ReviewScopeSelectionRequest__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_new --> T57_cli_composition_cli_composition_CapabilityCompositionRoot__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover --> T57_cli_composition_cli_composition_CapabilityCompositionRoot__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver --> T38_cli_driver_cli_driver_CapabilityDriver__self
T52_cli_composition_cli_composition_PhaseCompositionRoot_build --> T40_cli_driver_cli_driver_PhaseCommandDriver__self
T56_cli_composition_cli_composition_RefVerifyCompositionRoot_new --> T56_cli_composition_cli_composition_RefVerifyCompositionRoot__self
T56_cli_composition_cli_composition_RefVerifyCompositionRoot_ref_verify_driver --> T37_cli_driver_cli_driver_RefVerifyDriver__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_new --> T53_cli_composition_cli_composition_ReviewCompositionRoot__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver --> T34_cli_driver_cli_driver_ReviewDriver__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver --> T37_cli_driver_cli_driver_ReviewFixDriver__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_new --> T53_cli_composition_cli_composition_ReviewCompositionRoot__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver --> T34_cli_driver_cli_driver_ReviewDriver__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver --> T37_cli_driver_cli_driver_ReviewFixDriver__self
T18_cli_cli_CliCommand_Phase --o|cmd| T20_cli_cli_PhaseCommand__self
T18_cli_cli_CliCommand_Review --o|cmd| T21_cli_cli_ReviewCommand__self
T25_cli_cli_CheckApprovedArgs__self --o|chain| T30_cli_cli_RefVerifyCheckChainArg__self
T29_cli_cli_CheckZeroFindingsArgs__self --o|round| T27_cli_cli_ReviewCheckRoundArg__self
T20_cli_cli_PhaseCommand_Validate --o T25_cli_cli_PhaseValidateArgs__self
T20_cli_cli_PhaseCommand_Explain --o T19_cli_cli_PhaseIdArgs__self
T20_cli_cli_PhaseCommand_Enter --o T22_cli_cli_PhaseEnterArgs__self
T22_cli_cli_PhaseEnterArgs__self --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
T19_cli_cli_PhaseIdArgs__self --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
T21_cli_cli_ReviewCommand_CheckApproved --o T31_cli_cli_ReviewCheckApprovedArgs__self
T21_cli_cli_ReviewCommand_CheckZeroFindings --o T29_cli_cli_CheckZeroFindingsArgs__self
F52_cli_cli_cli__commands__capability__into_driver_input --o T26_cli_cli_CapabilityExecArgs__self
F52_cli_cli_cli__commands__capability__into_driver_input --> T47_cli_driver_cli_driver_CapabilityExecDriverInput__self
F37_cli_cli_cli__commands__phase__execute --o T20_cli_cli_PhaseCommand__self
F49_cli_cli_cli__commands__phase__execute_with_driver --o T20_cli_cli_PhaseCommand__self
F49_cli_cli_cli__commands__phase__execute_with_driver --o T40_cli_driver_cli_driver_PhaseCommandDriver__self
F48_cli_cli_cli__commands__phase__input_from_command --o T20_cli_cli_PhaseCommand__self
F48_cli_cli_cli__commands__phase__input_from_command --> T39_cli_driver_cli_driver_PhaseCommandInput__self
F69_cli_cli_cli__commands__ref_verify__execute_check_approved_with_driver --o T25_cli_cli_CheckApprovedArgs__self
F69_cli_cli_cli__commands__ref_verify__execute_check_approved_with_driver --o T37_cli_driver_cli_driver_RefVerifyDriver__self
F53_cli_cli_cli__commands__review__execute_check_approved --o T31_cli_cli_ReviewCheckApprovedArgs__self
F58_cli_cli_cli__commands__review__execute_check_zero_findings --o T29_cli_cli_CheckZeroFindingsArgs__self
class T27_domain_domain_MainScopeName_new method_node
class T27_domain_domain_MainScopeName_as_str method_node
class T27_domain_domain_MainScopeName__self value_object
class T23_domain_domain_ScopeName_Main variant_node
class T23_domain_domain_ScopeName_Other variant_node
class T23_domain_domain_ScopeName_parse method_node
class T23_domain_domain_ScopeName__self value_object
class T28_domain_domain_ScopeNameError_Empty variant_node
class T28_domain_domain_ScopeNameError_NotAscii variant_node
class T28_domain_domain_ScopeNameError_Reserved variant_node
class T28_domain_domain_ScopeNameError__self error_type
class F70_domain_domain_domain__review_v2__types__derive_review_approval_verdict free_function
class F70_domain_domain_domain__review_v2__types__derive_review_approval_verdict function_node
class T32_domain_domain_ContractedEntryRef_new method_node
class T32_domain_domain_ContractedEntryRef_layer method_node
class T32_domain_domain_ContractedEntryRef_entry_key method_node
class T32_domain_domain_ContractedEntryRef__self value_object
class T35_domain_domain_CoverageVerifyOutcome_Passed variant_node
class T35_domain_domain_CoverageVerifyOutcome_Blocked variant_node
class T35_domain_domain_CoverageVerifyOutcome_blocked method_node
class T35_domain_domain_CoverageVerifyOutcome__self value_object
class T31_domain_domain_CoverageViolation_MissingTaskContract variant_node
class T31_domain_domain_CoverageViolation_OrphanEntry variant_node
class T31_domain_domain_CoverageViolation_InvalidEntryRef variant_node
class T31_domain_domain_CoverageViolation_MissingSignalDocument variant_node
class T31_domain_domain_CoverageViolation_InvalidTaskRef variant_node
class T31_domain_domain_CoverageViolation__self value_object
class T34_domain_domain_PreReviewGateOutcome_Passed variant_node
class T34_domain_domain_PreReviewGateOutcome_Blocked variant_node
class T34_domain_domain_PreReviewGateOutcome_blocked method_node
class T34_domain_domain_PreReviewGateOutcome__self value_object
class T36_domain_domain_PreReviewGateViolation_MissingTaskContract variant_node
class T36_domain_domain_PreReviewGateViolation_NonBlueSignal variant_node
class T36_domain_domain_PreReviewGateViolation__self value_object
class T34_domain_domain_TaskContractDocument_new method_node
class T34_domain_domain_TaskContractDocument_schema_version method_node
class T34_domain_domain_TaskContractDocument_track_id method_node
class T34_domain_domain_TaskContractDocument_entries method_node
class T34_domain_domain_TaskContractDocument__self value_object
class T40_usecase_usecase_CapabilityExecInteractor_new method_node
class T40_usecase_usecase_CapabilityExecInteractor__self interactor
class T37_usecase_usecase_CapabilityExecRequest__self command
class R37_usecase_usecase_CapabilityExecService_execute method_node
class R37_usecase_usecase_CapabilityExecService__self app_service
class T30_usecase_usecase_DiagnosticText_new method_node
class T30_usecase_usecase_DiagnosticText_as_str method_node
class T30_usecase_usecase_DiagnosticText__self value_object
class T31_usecase_usecase_CommandArgument_try_new method_node
class T31_usecase_usecase_CommandArgument_as_str method_node
class T31_usecase_usecase_CommandArgument__self value_object
class T27_usecase_usecase_CommandArgv_try_new method_node
class T27_usecase_usecase_CommandArgv_arguments method_node
class T27_usecase_usecase_CommandArgv_with_appended_arguments method_node
class T27_usecase_usecase_CommandArgv__self value_object
class T42_usecase_usecase_CommandArgvValidationError_Empty variant_node
class T42_usecase_usecase_CommandArgvValidationError_RecursiveInvocation variant_node
class T42_usecase_usecase_CommandArgvValidationError__self error_type
class T38_usecase_usecase_CommandConfigLoadError_ReadFailed variant_node
class T38_usecase_usecase_CommandConfigLoadError_DecodeFailed variant_node
class T38_usecase_usecase_CommandConfigLoadError_Invalid variant_node
class T38_usecase_usecase_CommandConfigLoadError__self error_type
class T42_usecase_usecase_CommandConfigSchemaVersion_new method_node
class T42_usecase_usecase_CommandConfigSchemaVersion_as_u32 method_node
class T42_usecase_usecase_CommandConfigSchemaVersion__self value_object
class T44_usecase_usecase_CommandConfigValidationError_InvalidSchemaVersion variant_node
class T44_usecase_usecase_CommandConfigValidationError_InvalidDeclarationId variant_node
class T44_usecase_usecase_CommandConfigValidationError_InvalidReviewScope variant_node
class T44_usecase_usecase_CommandConfigValidationError_DuplicateDeclaration variant_node
class T44_usecase_usecase_CommandConfigValidationError_DuplicateScope variant_node
class T44_usecase_usecase_CommandConfigValidationError_EmptyArgv variant_node
class T44_usecase_usecase_CommandConfigValidationError_TimeoutOutOfRange variant_node
class T44_usecase_usecase_CommandConfigValidationError_RecursiveInvocation variant_node
class T44_usecase_usecase_CommandConfigValidationError_PersistedHostArgument variant_node
class T44_usecase_usecase_CommandConfigValidationError__self error_type
class T36_usecase_usecase_CommandDeclarationId_try_new method_node
class T36_usecase_usecase_CommandDeclarationId_as_str method_node
class T36_usecase_usecase_CommandDeclarationId__self value_object
class T51_usecase_usecase_CommandDeclarationIdValidationError_Empty variant_node
class T51_usecase_usecase_CommandDeclarationIdValidationError__self error_type
class T36_usecase_usecase_CommandSequenceIndex_new method_node
class T36_usecase_usecase_CommandSequenceIndex_as_usize method_node
class T36_usecase_usecase_CommandSequenceIndex__self value_object
class T37_usecase_usecase_CommandTimeoutSeconds_try_new method_node
class T37_usecase_usecase_CommandTimeoutSeconds_default_max method_node
class T37_usecase_usecase_CommandTimeoutSeconds_as_secs method_node
class T37_usecase_usecase_CommandTimeoutSeconds__self value_object
class T45_usecase_usecase_CommandTimeoutValidationError_OutOfRange variant_node
class T45_usecase_usecase_CommandTimeoutValidationError__self error_type
class T33_usecase_usecase_ConfiguredCommand_try_new method_node
class T33_usecase_usecase_ConfiguredCommand_argv method_node
class T33_usecase_usecase_ConfiguredCommand_timeout method_node
class T33_usecase_usecase_ConfiguredCommand__self value_object
class T48_usecase_usecase_ConfiguredCommandValidationError_Argv variant_node
class T48_usecase_usecase_ConfiguredCommandValidationError_Timeout variant_node
class T48_usecase_usecase_ConfiguredCommandValidationError_PersistedHostArgument variant_node
class T48_usecase_usecase_ConfiguredCommandValidationError__self error_type
class T39_usecase_usecase_OutputCaptureLimitBytes_one_mebibyte method_node
class T39_usecase_usecase_OutputCaptureLimitBytes_as_usize method_node
class T39_usecase_usecase_OutputCaptureLimitBytes__self value_object
class T41_usecase_usecase_UnvalidatedTimeoutSeconds_new method_node
class T41_usecase_usecase_UnvalidatedTimeoutSeconds_as_u64 method_node
class T41_usecase_usecase_UnvalidatedTimeoutSeconds__self value_object
class T34_usecase_usecase_PhaseCommandConfig_try_new method_node
class T34_usecase_usecase_PhaseCommandConfig_declaration method_node
class T34_usecase_usecase_PhaseCommandConfig__self value_object
class T49_usecase_usecase_PhaseCommandConfigValidationError_InvalidSchemaVersion variant_node
class T49_usecase_usecase_PhaseCommandConfigValidationError_DuplicateDeclaration variant_node
class T49_usecase_usecase_PhaseCommandConfigValidationError_into_command_config_validation_error method_node
class T49_usecase_usecase_PhaseCommandConfigValidationError__self error_type
class T39_usecase_usecase_PhaseCommandDeclaration_new method_node
class T39_usecase_usecase_PhaseCommandDeclaration_id method_node
class T39_usecase_usecase_PhaseCommandDeclaration_writer method_node
class T39_usecase_usecase_PhaseCommandDeclaration_pre_entry_commands method_node
class T39_usecase_usecase_PhaseCommandDeclaration__self value_object
class T38_usecase_usecase_PhaseCommandEnterError_Config variant_node
class T38_usecase_usecase_PhaseCommandEnterError_UnknownPhase variant_node
class T38_usecase_usecase_PhaseCommandEnterError_Runner variant_node
class T38_usecase_usecase_PhaseCommandEnterError__self error_type
class T40_usecase_usecase_PhaseCommandEnterOutcome_Completed variant_node
class T40_usecase_usecase_PhaseCommandEnterOutcome_Blocked variant_node
class T40_usecase_usecase_PhaseCommandEnterOutcome__self dto
class T40_usecase_usecase_PhaseCommandExplainError_Config variant_node
class T40_usecase_usecase_PhaseCommandExplainError_UnknownPhase variant_node
class T40_usecase_usecase_PhaseCommandExplainError__self error_type
class T39_usecase_usecase_PhaseCommandExplanation__self dto
class T38_usecase_usecase_PhaseCommandInteractor_new method_node
class T38_usecase_usecase_PhaseCommandInteractor__self interactor
class T33_usecase_usecase_PhaseEnterCommand__self command
class T33_usecase_usecase_PhaseExplainQuery__self query
class T36_usecase_usecase_PhaseValidateCommand__self command
class R44_usecase_usecase_PhaseCommandConfigLoaderPort_load method_node
class R44_usecase_usecase_PhaseCommandConfigLoaderPort__self secondary_port
class R35_usecase_usecase_PhaseCommandService_validate method_node
class R35_usecase_usecase_PhaseCommandService_explain method_node
class R35_usecase_usecase_PhaseCommandService_enter method_node
class R35_usecase_usecase_PhaseCommandService__self app_service
class T46_usecase_usecase_CurrentReviewTrackResolveError_ResolveFailed variant_node
class T46_usecase_usecase_CurrentReviewTrackResolveError__self error_type
class T38_usecase_usecase_PreReviewCommandConfig_try_new method_node
class T38_usecase_usecase_PreReviewCommandConfig_commands_for method_node
class T38_usecase_usecase_PreReviewCommandConfig__self value_object
class T53_usecase_usecase_PreReviewCommandConfigValidationError_InvalidSchemaVersion variant_node
class T53_usecase_usecase_PreReviewCommandConfigValidationError_DuplicateScope variant_node
class T53_usecase_usecase_PreReviewCommandConfigValidationError__self error_type
class T47_usecase_usecase_PreReviewCommandDispatchCommand__self command
class T45_usecase_usecase_PreReviewCommandDispatchError_Config variant_node
class T45_usecase_usecase_PreReviewCommandDispatchError_UnknownScope variant_node
class T45_usecase_usecase_PreReviewCommandDispatchError_TrackResolution variant_node
class T45_usecase_usecase_PreReviewCommandDispatchError_TrackMismatch variant_node
class T45_usecase_usecase_PreReviewCommandDispatchError_Runner variant_node
class T45_usecase_usecase_PreReviewCommandDispatchError__self error_type
class T50_usecase_usecase_PreReviewCommandDispatchInteractor_new method_node
class T50_usecase_usecase_PreReviewCommandDispatchInteractor__self interactor
class T47_usecase_usecase_PreReviewCommandDispatchOutcome_ReadyForReview variant_node
class T47_usecase_usecase_PreReviewCommandDispatchOutcome_Blocked variant_node
class T47_usecase_usecase_PreReviewCommandDispatchOutcome__self dto
class T53_usecase_usecase_PreReviewCommandGatedReviewInteractor_new method_node
class T53_usecase_usecase_PreReviewCommandGatedReviewInteractor__self interactor
class T48_usecase_usecase_PreReviewScopeCommandDeclaration_new method_node
class T48_usecase_usecase_PreReviewScopeCommandDeclaration_scope method_node
class T48_usecase_usecase_PreReviewScopeCommandDeclaration_commands method_node
class T48_usecase_usecase_PreReviewScopeCommandDeclaration__self value_object
class T35_usecase_usecase_ReviewScopeSelector_Named variant_node
class T35_usecase_usecase_ReviewScopeSelector_Other variant_node
class T35_usecase_usecase_ReviewScopeSelector__self value_object
class T35_usecase_usecase_ReviewTrackSelector_Explicit variant_node
class T35_usecase_usecase_ReviewTrackSelector_CurrentBranch variant_node
class T35_usecase_usecase_ReviewTrackSelector__self value_object
class R46_usecase_usecase_CurrentReviewTrackResolverPort_resolve method_node
class R46_usecase_usecase_CurrentReviewTrackResolverPort__self secondary_port
class R48_usecase_usecase_PreReviewCommandConfigLoaderPort_load method_node
class R48_usecase_usecase_PreReviewCommandConfigLoaderPort__self secondary_port
class R47_usecase_usecase_PreReviewCommandDispatchService_dispatch method_node
class R47_usecase_usecase_PreReviewCommandDispatchService__self app_service
class T37_usecase_usecase_CapturedProgramOutput__self dto
class T48_usecase_usecase_ClassifiedProgramExecutionRecord_Succeeded variant_node
class T48_usecase_usecase_ClassifiedProgramExecutionRecord_Failed variant_node
class T48_usecase_usecase_ClassifiedProgramExecutionRecord__self dto
class T44_usecase_usecase_FailedProgramExecutionRecord__self dto
class T38_usecase_usecase_ProgramExecutionRecord_classify method_node
class T38_usecase_usecase_ProgramExecutionRecord__self dto
class T31_usecase_usecase_ProgramExitCode_new method_node
class T31_usecase_usecase_ProgramExitCode_as_i32 method_node
class T31_usecase_usecase_ProgramExitCode__self value_object
class T33_usecase_usecase_ProgramInvocation__self dto
class T35_usecase_usecase_ProgramOutputStream_Stdout variant_node
class T35_usecase_usecase_ProgramOutputStream_Stderr variant_node
class T35_usecase_usecase_ProgramOutputStream__self value_object
class T33_usecase_usecase_ProgramRunOutcome_Exited variant_node
class T33_usecase_usecase_ProgramRunOutcome_TimedOut variant_node
class T33_usecase_usecase_ProgramRunOutcome_OutputLimitExceeded variant_node
class T33_usecase_usecase_ProgramRunOutcome__self dto
class T34_usecase_usecase_ProgramRunnerError_SpawnFailed variant_node
class T34_usecase_usecase_ProgramRunnerError_WaitFailed variant_node
class T34_usecase_usecase_ProgramRunnerError_TerminateFailed variant_node
class T34_usecase_usecase_ProgramRunnerError__self error_type
class T48_usecase_usecase_SuccessfulProgramExecutionRecord__self dto
class R33_usecase_usecase_ProgramRunnerPort_run method_node
class R33_usecase_usecase_ProgramRunnerPort__self secondary_port
class T36_usecase_usecase_RefVerifyChainFilter_Chain1 variant_node
class T36_usecase_usecase_RefVerifyChainFilter_Chain2 variant_node
class T36_usecase_usecase_RefVerifyChainFilter_All variant_node
class T36_usecase_usecase_RefVerifyChainFilter__self dto
class R41_usecase_usecase_RefVerifyAggregateService_run method_node
class R41_usecase_usecase_RefVerifyAggregateService_results method_node
class R41_usecase_usecase_RefVerifyAggregateService__self app_service
class R51_usecase_usecase_RefVerifyCheckApprovedDriverService_check_approved method_node
class R51_usecase_usecase_RefVerifyCheckApprovedDriverService__self app_service
class T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new method_node
class T46_usecase_usecase_NonEmptyReviewerFindingsOutput_as_slice method_node
class T46_usecase_usecase_NonEmptyReviewerFindingsOutput_try_new method_node
class T46_usecase_usecase_NonEmptyReviewerFindingsOutput_as_slice method_node
class T46_usecase_usecase_NonEmptyReviewerFindingsOutput__self value_object
class T54_usecase_usecase_ReviewCheckZeroFindingsEvaluationError_EvaluationFailed variant_node
class T54_usecase_usecase_ReviewCheckZeroFindingsEvaluationError__self error_type
class T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new method_node
class T49_usecase_usecase_ReviewCheckZeroFindingsInteractor_new method_node
class T49_usecase_usecase_ReviewCheckZeroFindingsInteractor__self interactor
class T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_CurrentFinalZeroFindings variant_node
class T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_MissingFinalVerdict variant_node
class T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_StaleFinalVerdict variant_node
class T46_usecase_usecase_ReviewCheckZeroFindingsOutcome_FindingsRemain variant_node
class T46_usecase_usecase_ReviewCheckZeroFindingsOutcome__self dto
class T44_usecase_usecase_ReviewCheckZeroFindingsQuery_try_new method_node
class T44_usecase_usecase_ReviewCheckZeroFindingsQuery__self query
class T54_usecase_usecase_ReviewCheckZeroFindingsValidationError_InvalidTrackId variant_node
class T54_usecase_usecase_ReviewCheckZeroFindingsValidationError_InvalidScope variant_node
class T54_usecase_usecase_ReviewCheckZeroFindingsValidationError__self error_type
class T51_usecase_usecase_ReviewFindingsOutputValidationError_Empty variant_node
class T51_usecase_usecase_ReviewFindingsOutputValidationError__self error_type
class T35_usecase_usecase_ReviewFixResolution_new method_node
class T35_usecase_usecase_ReviewFixResolution_track_id method_node
class T35_usecase_usecase_ReviewFixResolution_repository_root method_node
class T35_usecase_usecase_ReviewFixResolution_new method_node
class T35_usecase_usecase_ReviewFixResolution_track_id method_node
class T35_usecase_usecase_ReviewFixResolution_repository_root method_node
class T35_usecase_usecase_ReviewFixResolution__self dto
class T36_usecase_usecase_ReviewFixRunnerError_SmokeTestFailed variant_node
class T36_usecase_usecase_ReviewFixRunnerError_SpawnFailed variant_node
class T36_usecase_usecase_ReviewFixRunnerError_SentinelNotFound variant_node
class T36_usecase_usecase_ReviewFixRunnerError_SubagentDispatchRequired variant_node
class T36_usecase_usecase_ReviewFixRunnerError_Unexpected variant_node
class T36_usecase_usecase_ReviewFixRunnerError__self error_type
class T42_usecase_usecase_ReviewFixTrackResolveError_BranchReadFailed variant_node
class T42_usecase_usecase_ReviewFixTrackResolveError_NonTrackBranch variant_node
class T42_usecase_usecase_ReviewFixTrackResolveError__self error_type
class T39_usecase_usecase_ReviewNotRequiredReason_Empty variant_node
class T39_usecase_usecase_ReviewNotRequiredReason_ZeroFindings variant_node
class T39_usecase_usecase_ReviewNotRequiredReason__self dto
class T36_usecase_usecase_ReviewRequiredReason_NotStarted variant_node
class T36_usecase_usecase_ReviewRequiredReason_FindingsRemain variant_node
class T36_usecase_usecase_ReviewRequiredReason_StaleHash variant_node
class T36_usecase_usecase_ReviewRequiredReason__self dto
class T34_usecase_usecase_ReviewResultsError_Failed variant_node
class T34_usecase_usecase_ReviewResultsError_UnknownScope variant_node
class T34_usecase_usecase_ReviewResultsError_MissingScopeState variant_node
class T34_usecase_usecase_ReviewResultsError__self error_type
class T39_usecase_usecase_ReviewResultsInteractor_new method_node
class T39_usecase_usecase_ReviewResultsInteractor_new method_node
class T39_usecase_usecase_ReviewResultsInteractor__self interactor
class T35_usecase_usecase_ReviewResultsOutput__self dto
class T42_usecase_usecase_ReviewResultsScopeSnapshot__self dto
class T39_usecase_usecase_ReviewRoundResultOutput__self dto
class T40_usecase_usecase_ReviewRoundResultVerdict_ZeroFindings variant_node
class T40_usecase_usecase_ReviewRoundResultVerdict_FindingsRemain variant_node
class T40_usecase_usecase_ReviewRoundResultVerdict__self dto
class T36_usecase_usecase_ReviewRunLocalOutput__self dto
class T31_usecase_usecase_ReviewScopeName_try_new method_node
class T31_usecase_usecase_ReviewScopeName_other method_node
class T31_usecase_usecase_ReviewScopeName_as_str method_node
class T31_usecase_usecase_ReviewScopeName_try_new method_node
class T31_usecase_usecase_ReviewScopeName_other method_node
class T31_usecase_usecase_ReviewScopeName_as_str method_node
class T31_usecase_usecase_ReviewScopeName__self value_object
class T46_usecase_usecase_ReviewScopeNameValidationError_Invalid variant_node
class T46_usecase_usecase_ReviewScopeNameValidationError__self error_type
class T39_usecase_usecase_ReviewScopeResultOutput__self dto
class T38_usecase_usecase_ReviewScopeResultState_RequiredNotStarted variant_node
class T38_usecase_usecase_ReviewScopeResultState_RequiredFindingsRemain variant_node
class T38_usecase_usecase_ReviewScopeResultState_RequiredStaleHash variant_node
class T38_usecase_usecase_ReviewScopeResultState_Empty variant_node
class T38_usecase_usecase_ReviewScopeResultState_Approved variant_node
class T38_usecase_usecase_ReviewScopeResultState__self dto
class T43_usecase_usecase_ReviewScopeSelectionRequest_NamedCandidate variant_node
class T43_usecase_usecase_ReviewScopeSelectionRequest_All variant_node
class T43_usecase_usecase_ReviewScopeSelectionRequest_try_new method_node
class T43_usecase_usecase_ReviewScopeSelectionRequest_try_new method_node
class T43_usecase_usecase_ReviewScopeSelectionRequest__self value_object
class T51_usecase_usecase_ReviewScopeSelectionValidationError_ScopeAndAll variant_node
class T51_usecase_usecase_ReviewScopeSelectionValidationError_InvalidScope variant_node
class T51_usecase_usecase_ReviewScopeSelectionValidationError__self error_type
class T33_usecase_usecase_ReviewStoredRound__self dto
class T40_usecase_usecase_ReviewStoredRoundVerdict__self dto
class T38_usecase_usecase_ReviewStoredScopeState_Required variant_node
class T38_usecase_usecase_ReviewStoredScopeState_NotRequired variant_node
class T38_usecase_usecase_ReviewStoredScopeState__self dto
class T43_usecase_usecase_ReviewStoredScopeStateEntry__self dto
class T29_usecase_usecase_ReviewTrackId_try_new method_node
class T29_usecase_usecase_ReviewTrackId_as_str method_node
class T29_usecase_usecase_ReviewTrackId_try_new method_node
class T29_usecase_usecase_ReviewTrackId_as_str method_node
class T29_usecase_usecase_ReviewTrackId__self value_object
class T44_usecase_usecase_ReviewTrackIdValidationError_Invalid variant_node
class T44_usecase_usecase_ReviewTrackIdValidationError__self error_type
class T37_usecase_usecase_ReviewerFindingOutput__self dto
class T35_usecase_usecase_RunReviewFixCommand_new_resolved method_node
class T35_usecase_usecase_RunReviewFixCommand_scope method_node
class T35_usecase_usecase_RunReviewFixCommand_briefing_file method_node
class T35_usecase_usecase_RunReviewFixCommand_track_id method_node
class T35_usecase_usecase_RunReviewFixCommand_repository_root method_node
class T35_usecase_usecase_RunReviewFixCommand_round_type method_node
class T35_usecase_usecase_RunReviewFixCommand_model method_node
class T35_usecase_usecase_RunReviewFixCommand_new_resolved method_node
class T35_usecase_usecase_RunReviewFixCommand_scope method_node
class T35_usecase_usecase_RunReviewFixCommand_briefing_file method_node
class T35_usecase_usecase_RunReviewFixCommand_track_id method_node
class T35_usecase_usecase_RunReviewFixCommand_repository_root method_node
class T35_usecase_usecase_RunReviewFixCommand_round_type method_node
class T35_usecase_usecase_RunReviewFixCommand_model method_node
class T35_usecase_usecase_RunReviewFixCommand__self command
class T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidScope variant_node
class T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidTrackId variant_node
class T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidRoundType variant_node
class T50_usecase_usecase_RunReviewFixCommandValidationError_InvalidModel variant_node
class T50_usecase_usecase_RunReviewFixCommandValidationError__self error_type
class T33_usecase_usecase_RunReviewFixError_FixRunnerFailed variant_node
class T33_usecase_usecase_RunReviewFixError_TrackResolution variant_node
class T33_usecase_usecase_RunReviewFixError_TrackMismatch variant_node
class T33_usecase_usecase_RunReviewFixError__self error_type
class T38_usecase_usecase_RunReviewFixInteractor_new method_node
class T38_usecase_usecase_RunReviewFixInteractor_new method_node
class T38_usecase_usecase_RunReviewFixInteractor__self interactor
class T34_usecase_usecase_RunReviewFixOutput__self dto
class T35_usecase_usecase_RunReviewFixRequest_try_new method_node
class T35_usecase_usecase_RunReviewFixRequest_try_new method_node
class T35_usecase_usecase_RunReviewFixRequest__self command
class T43_usecase_usecase_SubagentDispatchInstruction__self dto
class T28_usecase_usecase_SubagentName_try_new method_node
class T28_usecase_usecase_SubagentName_as_str method_node
class T28_usecase_usecase_SubagentName_try_new method_node
class T28_usecase_usecase_SubagentName_as_str method_node
class T28_usecase_usecase_SubagentName__self value_object
class T43_usecase_usecase_SubagentNameValidationError_Invalid variant_node
class T43_usecase_usecase_SubagentNameValidationError__self error_type
class R42_usecase_usecase_ReviewCheckApprovedService_check_approved method_node
class R42_usecase_usecase_ReviewCheckApprovedService__self app_service
class R46_usecase_usecase_ReviewCheckZeroFindingsService_check_zero_findings method_node
class R46_usecase_usecase_ReviewCheckZeroFindingsService__self app_service
class R48_usecase_usecase_ReviewCheckZeroFindingsStatePort_state_for method_node
class R48_usecase_usecase_ReviewCheckZeroFindingsStatePort__self secondary_port
class R31_usecase_usecase_ReviewFixRunner_run_fix method_node
class R31_usecase_usecase_ReviewFixRunner__self secondary_port
class R42_usecase_usecase_ReviewFixTrackResolverPort_resolve_current_track method_node
class R42_usecase_usecase_ReviewFixTrackResolverPort__self secondary_port
class R38_usecase_usecase_ReviewResultsRoundPort_load_scope_rounds method_node
class R38_usecase_usecase_ReviewResultsRoundPort__self secondary_port
class R38_usecase_usecase_ReviewResultsScopePort_load_scope_snapshot method_node
class R38_usecase_usecase_ReviewResultsScopePort__self secondary_port
class R36_usecase_usecase_ReviewResultsService_results method_node
class R36_usecase_usecase_ReviewResultsService__self app_service
class R38_usecase_usecase_ReviewResultsStatePort_load_scope_states method_node
class R38_usecase_usecase_ReviewResultsStatePort__self secondary_port
class R37_usecase_usecase_ReviewRunLocalService_run_local method_node
class R37_usecase_usecase_ReviewRunLocalService__self app_service
class R29_usecase_usecase_ReviewService_run_codex method_node
class R29_usecase_usecase_ReviewService_run_claude method_node
class R29_usecase_usecase_ReviewService_run_local method_node
class R29_usecase_usecase_ReviewService_check_approved method_node
class R29_usecase_usecase_ReviewService_classify method_node
class R29_usecase_usecase_ReviewService_files method_node
class R29_usecase_usecase_ReviewService_validate_scope method_node
class R29_usecase_usecase_ReviewService_get_briefing method_node
class R29_usecase_usecase_ReviewService_persist_commit_hash method_node
class R29_usecase_usecase_ReviewService__self app_service
class R35_usecase_usecase_RunReviewFixService_run method_node
class R35_usecase_usecase_RunReviewFixService__self app_service
class T48_infrastructure_infrastructure_CommandArgumentDto__self dto
class T44_infrastructure_infrastructure_CommandArgvDto__self dto
class T59_infrastructure_infrastructure_CommandConfigSchemaVersionDto__self dto
class T53_infrastructure_infrastructure_CommandDeclarationIdDto__self dto
class T54_infrastructure_infrastructure_CommandTimeoutSecondsDto__self dto
class T50_infrastructure_infrastructure_ConfiguredCommandDto__self dto
class T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader_new method_node
class T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader__self secondary_adapter
class T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader_new method_node
class T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader__self secondary_adapter
class T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver_new method_node
class T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver__self secondary_adapter
class T51_infrastructure_infrastructure_PhaseCommandConfigDto__self dto
class T56_infrastructure_infrastructure_PhaseCommandDeclarationDto__self dto
class T55_infrastructure_infrastructure_PreReviewCommandConfigDto__self dto
class T65_infrastructure_infrastructure_PreReviewScopeCommandDeclarationDto__self dto
class T48_infrastructure_infrastructure_ReviewScopeNameDto__self dto
class F98_infrastructure_infrastructure_infrastructure__operator_command_config__decode_phase_command_config free_function
class F98_infrastructure_infrastructure_infrastructure__operator_command_config__decode_phase_command_config function_node
class F103_infrastructure_infrastructure_infrastructure__operator_command_config__decode_pre_review_command_config free_function
class F103_infrastructure_infrastructure_infrastructure__operator_command_config__decode_pre_review_command_config function_node
class T50_infrastructure_infrastructure_ProcessProgramRunner_new method_node
class T50_infrastructure_infrastructure_ProcessProgramRunner__self secondary_adapter
class T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter_new method_node
class T57_infrastructure_infrastructure_FsRefVerifyAggregateAdapter__self secondary_adapter
class T50_infrastructure_infrastructure_CodexReviewFixRunner_new method_node
class T50_infrastructure_infrastructure_CodexReviewFixRunner__self secondary_adapter
class T55_infrastructure_infrastructure_GitReviewFixTrackResolver__self secondary_adapter
class T49_infrastructure_infrastructure_ResultsRoundAdapter__self secondary_adapter
class T49_infrastructure_infrastructure_ResultsScopeAdapter__self secondary_adapter
class T49_infrastructure_infrastructure_ResultsStateAdapter__self secondary_adapter
class T65_infrastructure_infrastructure_ReviewCheckZeroFindingsStateAdapter__self secondary_adapter
class T52_infrastructure_infrastructure_ReviewFixRunnerAdapter__self secondary_adapter
class T51_infrastructure_infrastructure_ContractedEntryRefDto__self dto
class T41_infrastructure_infrastructure_EntryKeyDto__self dto
class T40_infrastructure_infrastructure_LayerIdDto__self dto
class T53_infrastructure_infrastructure_TaskContractDocumentDto__self dto
class T58_infrastructure_infrastructure_TaskContractSchemaVersionDto__self dto
class T39_infrastructure_infrastructure_TaskIdDto__self dto
class T40_infrastructure_infrastructure_TrackIdDto__self dto
class T38_cli_driver_cli_driver_CapabilityDriver_new method_node
class T38_cli_driver_cli_driver_CapabilityDriver_handle method_node
class T38_cli_driver_cli_driver_CapabilityDriver__self primary_adapter
class T47_cli_driver_cli_driver_CapabilityExecDriverInput__self dto
class T40_cli_driver_cli_driver_PhaseCommandDriver_new method_node
class T40_cli_driver_cli_driver_PhaseCommandDriver_handle method_node
class T40_cli_driver_cli_driver_PhaseCommandDriver__self primary_adapter
class T39_cli_driver_cli_driver_PhaseCommandInput_Validate variant_node
class T39_cli_driver_cli_driver_PhaseCommandInput_Explain variant_node
class T39_cli_driver_cli_driver_PhaseCommandInput_Enter variant_node
class T39_cli_driver_cli_driver_PhaseCommandInput__self dto
class T32_cli_driver_cli_driver_PhaseIdArg_new method_node
class T32_cli_driver_cli_driver_PhaseIdArg_as_declaration_id method_node
class T32_cli_driver_cli_driver_PhaseIdArg__self dto
class T42_cli_driver_cli_driver_RefVerifyChainSelect_Chain1 variant_node
class T42_cli_driver_cli_driver_RefVerifyChainSelect_Chain2 variant_node
class T42_cli_driver_cli_driver_RefVerifyChainSelect_All variant_node
class T42_cli_driver_cli_driver_RefVerifyChainSelect__self dto
class T49_cli_driver_cli_driver_RefVerifyCheckApprovedInput__self dto
class T37_cli_driver_cli_driver_RefVerifyDriver_new method_node
class T37_cli_driver_cli_driver_RefVerifyDriver_handle method_node
class T37_cli_driver_cli_driver_RefVerifyDriver__self primary_adapter
class T36_cli_driver_cli_driver_CommandOutcome_success method_node
class T36_cli_driver_cli_driver_CommandOutcome_failure method_node
class T36_cli_driver_cli_driver_CommandOutcome__self dto
class T44_cli_driver_cli_driver_ReviewCheckRoundSelect_Final variant_node
class T44_cli_driver_cli_driver_ReviewCheckRoundSelect__self dto
class T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_try_new method_node
class T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput_into_query method_node
class T50_cli_driver_cli_driver_ReviewCheckZeroFindingsInput__self dto
class T34_cli_driver_cli_driver_ReviewDriver_new method_node
class T34_cli_driver_cli_driver_ReviewDriver_handle method_node
class T34_cli_driver_cli_driver_ReviewDriver_new method_node
class T34_cli_driver_cli_driver_ReviewDriver_handle method_node
class T34_cli_driver_cli_driver_ReviewDriver__self primary_adapter
class T37_cli_driver_cli_driver_ReviewFixDriver_new method_node
class T37_cli_driver_cli_driver_ReviewFixDriver_handle method_node
class T37_cli_driver_cli_driver_ReviewFixDriver_new method_node
class T37_cli_driver_cli_driver_ReviewFixDriver_handle method_node
class T37_cli_driver_cli_driver_ReviewFixDriver__self primary_adapter
class T36_cli_driver_cli_driver_ReviewFixInput_new method_node
class T36_cli_driver_cli_driver_ReviewFixInput_into_parts method_node
class T36_cli_driver_cli_driver_ReviewFixInput_new method_node
class T36_cli_driver_cli_driver_ReviewFixInput_into_parts method_node
class T36_cli_driver_cli_driver_ReviewFixInput__self dto
class T33_cli_driver_cli_driver_ReviewInput_RunCodex variant_node
class T33_cli_driver_cli_driver_ReviewInput_RunClaude variant_node
class T33_cli_driver_cli_driver_ReviewInput_RunLocal variant_node
class T33_cli_driver_cli_driver_ReviewInput_CheckApproved variant_node
class T33_cli_driver_cli_driver_ReviewInput_CheckZeroFindings variant_node
class T33_cli_driver_cli_driver_ReviewInput_Results variant_node
class T33_cli_driver_cli_driver_ReviewInput_Classify variant_node
class T33_cli_driver_cli_driver_ReviewInput_Files variant_node
class T33_cli_driver_cli_driver_ReviewInput_ValidateScope variant_node
class T33_cli_driver_cli_driver_ReviewInput_GetBriefing variant_node
class T33_cli_driver_cli_driver_ReviewInput_PersistCommitHash variant_node
class T33_cli_driver_cli_driver_ReviewInput__self dto
class T40_cli_driver_cli_driver_ReviewResultsInput_try_new method_node
class T40_cli_driver_cli_driver_ReviewResultsInput_into_parts method_node
class T40_cli_driver_cli_driver_ReviewResultsInput_try_new method_node
class T40_cli_driver_cli_driver_ReviewResultsInput_into_parts method_node
class T40_cli_driver_cli_driver_ReviewResultsInput__self dto
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_new method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot__self composition_root
class T52_cli_composition_cli_composition_PhaseCompositionRoot_build method_node
class T52_cli_composition_cli_composition_PhaseCompositionRoot__self composition_root
class T56_cli_composition_cli_composition_RefVerifyCompositionRoot_new method_node
class T56_cli_composition_cli_composition_RefVerifyCompositionRoot_ref_verify_driver method_node
class T56_cli_composition_cli_composition_RefVerifyCompositionRoot__self composition_root
class T53_cli_composition_cli_composition_ReviewCompositionRoot_new method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_new method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot__self composition_root
class T18_cli_cli_CliCommand_Arch variant_node
class T18_cli_cli_CliCommand_AdrBaseline variant_node
class T18_cli_cli_CliCommand_Conventions variant_node
class T18_cli_cli_CliCommand_Domain variant_node
class T18_cli_cli_CliCommand_Guard variant_node
class T18_cli_cli_CliCommand_Hook variant_node
class T18_cli_cli_CliCommand_Maintenance variant_node
class T18_cli_cli_CliCommand_Track variant_node
class T18_cli_cli_CliCommand_Git variant_node
class T18_cli_cli_CliCommand_Pr variant_node
class T18_cli_cli_CliCommand_Capability variant_node
class T18_cli_cli_CliCommand_Phase variant_node
class T18_cli_cli_CliCommand_Review variant_node
class T18_cli_cli_CliCommand_File variant_node
class T18_cli_cli_CliCommand_Verify variant_node
class T18_cli_cli_CliCommand_FindSimilar variant_node
class T18_cli_cli_CliCommand_DupIndex variant_node
class T18_cli_cli_CliCommand_DupCheck variant_node
class T18_cli_cli_CliCommand_Telemetry variant_node
class T18_cli_cli_CliCommand_Dry variant_node
class T18_cli_cli_CliCommand_RefVerify variant_node
class T18_cli_cli_CliCommand_TestObligation variant_node
class T18_cli_cli_CliCommand_Signal variant_node
class T18_cli_cli_CliCommand_TaskContract variant_node
class T18_cli_cli_CliCommand_Catalog variant_node
class T18_cli_cli_CliCommand_CatalogueLint variant_node
class T18_cli_cli_CliCommand_Template variant_node
class T18_cli_cli_CliCommand_CodexRuntime variant_node
class T18_cli_cli_CliCommand_BatchPlan variant_node
class T18_cli_cli_CliCommand_Demo variant_node
class T18_cli_cli_CliCommand__self dto
class T26_cli_cli_CapabilityExecArgs__self dto
class T25_cli_cli_CheckApprovedArgs__self dto
class T29_cli_cli_CheckZeroFindingsArgs__self dto
class T20_cli_cli_PhaseCommand_Validate variant_node
class T20_cli_cli_PhaseCommand_Explain variant_node
class T20_cli_cli_PhaseCommand_Enter variant_node
class T20_cli_cli_PhaseCommand__self dto
class T22_cli_cli_PhaseEnterArgs__self dto
class T19_cli_cli_PhaseIdArgs__self dto
class T25_cli_cli_PhaseValidateArgs__self dto
class T30_cli_cli_RefVerifyCheckChainArg_Chain1 variant_node
class T30_cli_cli_RefVerifyCheckChainArg_Chain2 variant_node
class T30_cli_cli_RefVerifyCheckChainArg__self dto
class T31_cli_cli_ReviewCheckApprovedArgs__self dto
class T27_cli_cli_ReviewCheckRoundArg_Final variant_node
class T27_cli_cli_ReviewCheckRoundArg__self dto
class T21_cli_cli_ReviewCommand_Local variant_node
class T21_cli_cli_ReviewCommand_FixLocal variant_node
class T21_cli_cli_ReviewCommand_CheckApproved variant_node
class T21_cli_cli_ReviewCommand_CheckZeroFindings variant_node
class T21_cli_cli_ReviewCommand_Results variant_node
class T21_cli_cli_ReviewCommand_Classify variant_node
class T21_cli_cli_ReviewCommand_Files variant_node
class T21_cli_cli_ReviewCommand__self dto
class F52_cli_cli_cli__commands__capability__into_driver_input free_function
class F52_cli_cli_cli__commands__capability__into_driver_input function_node
class F37_cli_cli_cli__commands__phase__execute free_function
class F37_cli_cli_cli__commands__phase__execute function_node
class F49_cli_cli_cli__commands__phase__execute_with_driver free_function
class F49_cli_cli_cli__commands__phase__execute_with_driver function_node
class F48_cli_cli_cli__commands__phase__input_from_command free_function
class F48_cli_cli_cli__commands__phase__input_from_command function_node
class F69_cli_cli_cli__commands__ref_verify__execute_check_approved_with_driver free_function
class F69_cli_cli_cli__commands__ref_verify__execute_check_approved_with_driver function_node
class F53_cli_cli_cli__commands__review__execute_check_approved free_function
class F53_cli_cli_cli__commands__review__execute_check_approved function_node
class F58_cli_cli_cli__commands__review__execute_check_zero_findings free_function
class F58_cli_cli_cli__commands__review__execute_check_zero_findings function_node
```
