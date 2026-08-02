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
  end
  subgraph T36_usecase_usecase_CommandDeclarationId["operator_command::CommandDeclarationId"]
    direction TB
    T36_usecase_usecase_CommandDeclarationId__self[CommandDeclarationId]
    T36_usecase_usecase_CommandDeclarationId_try_new([try_new])
    T36_usecase_usecase_CommandDeclarationId_as_str([as_str])
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
  subgraph T33_usecase_usecase_ConfiguredCommand["operator_command::ConfiguredCommand"]
    direction TB
    T33_usecase_usecase_ConfiguredCommand__self[ConfiguredCommand]
    T33_usecase_usecase_ConfiguredCommand_try_new([try_new])
    T33_usecase_usecase_ConfiguredCommand_argv([argv])
    T33_usecase_usecase_ConfiguredCommand_timeout([timeout])
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
  subgraph T32_usecase_usecase_PhaseCommandStep["phase_command::PhaseCommandStep"]
    direction TB
    T32_usecase_usecase_PhaseCommandStep__self[PhaseCommandStep]
    T32_usecase_usecase_PhaseCommandStep_PreEntry[PreEntry]
    T32_usecase_usecase_PhaseCommandStep_Writer[Writer]
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
  subgraph T38_usecase_usecase_ProgramExecutionRecord["program_runner::ProgramExecutionRecord"]
    direction TB
    T38_usecase_usecase_ProgramExecutionRecord__self[ProgramExecutionRecord]
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
  subgraph R33_usecase_usecase_ProgramRunnerPort["program_runner::ProgramRunnerPort"]
    direction TB
    R33_usecase_usecase_ProgramRunnerPort__self[ProgramRunnerPort]
    R33_usecase_usecase_ProgramRunnerPort_run([run])
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
  subgraph infrastructure_infrastructure_module_task_contract_codec["infrastructure::task_contract_codec"]
    direction TB
  subgraph T51_infrastructure_infrastructure_ContractedEntryRefDto["task_contract_codec::ContractedEntryRefDto"]
    direction TB
    T51_infrastructure_infrastructure_ContractedEntryRefDto__self[ContractedEntryRefDto]
  end
  subgraph T53_infrastructure_infrastructure_TaskContractDocumentDto["task_contract_codec::TaskContractDocumentDto"]
    direction TB
    T53_infrastructure_infrastructure_TaskContractDocumentDto__self[TaskContractDocumentDto]
  end
  subgraph T58_infrastructure_infrastructure_TaskContractSchemaVersionDto["task_contract_codec::TaskContractSchemaVersionDto"]
    direction TB
    T58_infrastructure_infrastructure_TaskContractSchemaVersionDto__self[TaskContractSchemaVersionDto]
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
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
  subgraph cli_driver_cli_driver_module_render["cli_driver::render"]
    direction TB
  subgraph T36_cli_driver_cli_driver_CommandOutcome["render::CommandOutcome"]
    direction TB
    T36_cli_driver_cli_driver_CommandOutcome__self[CommandOutcome]
    T36_cli_driver_cli_driver_CommandOutcome_success([success])
    T36_cli_driver_cli_driver_CommandOutcome_failure([failure])
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_phase_command["cli_composition::phase_command"]
    direction TB
  subgraph T52_cli_composition_cli_composition_PhaseCompositionRoot["phase_command::PhaseCompositionRoot"]
    direction TB
    T52_cli_composition_cli_composition_PhaseCompositionRoot__self[PhaseCompositionRoot]
    T52_cli_composition_cli_composition_PhaseCompositionRoot_build([build])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
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
  subgraph T24_cli_cli_PhaseExplainArgs["commands::phase::PhaseExplainArgs"]
    direction TB
    T24_cli_cli_PhaseExplainArgs__self[PhaseExplainArgs]
  end
  subgraph T25_cli_cli_PhaseValidateArgs["commands::phase::PhaseValidateArgs"]
    direction TB
    T25_cli_cli_PhaseValidateArgs__self[PhaseValidateArgs]
  end
  end
end
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
T31_usecase_usecase_CommandArgument_try_new --> T31_usecase_usecase_CommandArgument__self
T27_usecase_usecase_CommandArgv_try_new --o T31_usecase_usecase_CommandArgument__self
T27_usecase_usecase_CommandArgv_try_new --> T44_usecase_usecase_CommandConfigValidationError__self
T27_usecase_usecase_CommandArgv_try_new --> T27_usecase_usecase_CommandArgv__self
T27_usecase_usecase_CommandArgv_arguments --> T31_usecase_usecase_CommandArgument__self
T38_usecase_usecase_CommandConfigLoadError_Invalid --o T44_usecase_usecase_CommandConfigValidationError__self
T42_usecase_usecase_CommandConfigSchemaVersion_new --> T42_usecase_usecase_CommandConfigSchemaVersion__self
T44_usecase_usecase_CommandConfigValidationError_InvalidSchemaVersion --o|actual| T42_usecase_usecase_CommandConfigSchemaVersion__self
T44_usecase_usecase_CommandConfigValidationError_DuplicateDeclaration --o T36_usecase_usecase_CommandDeclarationId__self
T44_usecase_usecase_CommandConfigValidationError_TimeoutOutOfRange --o|seconds| T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T44_usecase_usecase_CommandConfigValidationError_RecursiveInvocation --o|prefix| T31_usecase_usecase_CommandArgument__self
T36_usecase_usecase_CommandDeclarationId_try_new --> T44_usecase_usecase_CommandConfigValidationError__self
T36_usecase_usecase_CommandDeclarationId_try_new --> T36_usecase_usecase_CommandDeclarationId__self
T36_usecase_usecase_CommandSequenceIndex_new --> T36_usecase_usecase_CommandSequenceIndex__self
T37_usecase_usecase_CommandTimeoutSeconds_try_new --o T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T37_usecase_usecase_CommandTimeoutSeconds_try_new --> T44_usecase_usecase_CommandConfigValidationError__self
T37_usecase_usecase_CommandTimeoutSeconds_try_new --> T37_usecase_usecase_CommandTimeoutSeconds__self
T37_usecase_usecase_CommandTimeoutSeconds_default_max --> T37_usecase_usecase_CommandTimeoutSeconds__self
T33_usecase_usecase_ConfiguredCommand_try_new --o T31_usecase_usecase_CommandArgument__self
T33_usecase_usecase_ConfiguredCommand_try_new --o T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T33_usecase_usecase_ConfiguredCommand_try_new --> T44_usecase_usecase_CommandConfigValidationError__self
T33_usecase_usecase_ConfiguredCommand_try_new --> T33_usecase_usecase_ConfiguredCommand__self
T33_usecase_usecase_ConfiguredCommand_argv --> T27_usecase_usecase_CommandArgv__self
T33_usecase_usecase_ConfiguredCommand_timeout --> T37_usecase_usecase_CommandTimeoutSeconds__self
T39_usecase_usecase_OutputCaptureLimitBytes_one_mebibyte --> T39_usecase_usecase_OutputCaptureLimitBytes__self
T41_usecase_usecase_UnvalidatedTimeoutSeconds_new --> T41_usecase_usecase_UnvalidatedTimeoutSeconds__self
T34_usecase_usecase_PhaseCommandConfig_try_new --o T42_usecase_usecase_CommandConfigSchemaVersion__self
T34_usecase_usecase_PhaseCommandConfig_try_new --o T39_usecase_usecase_PhaseCommandDeclaration__self
T34_usecase_usecase_PhaseCommandConfig_try_new --> T44_usecase_usecase_CommandConfigValidationError__self
T34_usecase_usecase_PhaseCommandConfig_try_new --> T34_usecase_usecase_PhaseCommandConfig__self
T34_usecase_usecase_PhaseCommandConfig_declaration --o T36_usecase_usecase_CommandDeclarationId__self
T34_usecase_usecase_PhaseCommandConfig_declaration --> T39_usecase_usecase_PhaseCommandDeclaration__self
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
T40_usecase_usecase_PhaseCommandEnterOutcome_Completed --o|records| T38_usecase_usecase_ProgramExecutionRecord__self
T40_usecase_usecase_PhaseCommandEnterOutcome_Blocked --o|completed| T38_usecase_usecase_ProgramExecutionRecord__self
T40_usecase_usecase_PhaseCommandEnterOutcome_Blocked --o|failed| T38_usecase_usecase_ProgramExecutionRecord__self
T40_usecase_usecase_PhaseCommandExplainError_Config --o T38_usecase_usecase_CommandConfigLoadError__self
T40_usecase_usecase_PhaseCommandExplainError_UnknownPhase --o T36_usecase_usecase_CommandDeclarationId__self
T39_usecase_usecase_PhaseCommandExplanation__self --o|phase_id| T36_usecase_usecase_CommandDeclarationId__self
T39_usecase_usecase_PhaseCommandExplanation__self --o|steps| T32_usecase_usecase_PhaseCommandStep__self
T39_usecase_usecase_PhaseCommandExplanation__self --o|output_limit| T39_usecase_usecase_OutputCaptureLimitBytes__self
T38_usecase_usecase_PhaseCommandInteractor_new --o R44_usecase_usecase_PhaseCommandConfigLoaderPort__self
T38_usecase_usecase_PhaseCommandInteractor_new --o R33_usecase_usecase_ProgramRunnerPort__self
T38_usecase_usecase_PhaseCommandInteractor_new --> T38_usecase_usecase_PhaseCommandInteractor__self
T38_usecase_usecase_PhaseCommandInteractor__self --o|config_loader| R44_usecase_usecase_PhaseCommandConfigLoaderPort__self
T38_usecase_usecase_PhaseCommandInteractor__self --o|runner| R33_usecase_usecase_ProgramRunnerPort__self
T32_usecase_usecase_PhaseCommandStep_PreEntry --o|position| T36_usecase_usecase_CommandSequenceIndex__self
T32_usecase_usecase_PhaseCommandStep_PreEntry --o|command| T33_usecase_usecase_ConfiguredCommand__self
T32_usecase_usecase_PhaseCommandStep_Writer --o|command| T33_usecase_usecase_ConfiguredCommand__self
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
T38_usecase_usecase_PreReviewCommandConfig_try_new --> T44_usecase_usecase_CommandConfigValidationError__self
T38_usecase_usecase_PreReviewCommandConfig_try_new --> T38_usecase_usecase_PreReviewCommandConfig__self
T38_usecase_usecase_PreReviewCommandConfig_commands_for --> T33_usecase_usecase_ConfiguredCommand__self
T47_usecase_usecase_PreReviewCommandDispatchCommand__self --o|track| T35_usecase_usecase_ReviewTrackSelector__self
T47_usecase_usecase_PreReviewCommandDispatchCommand__self --o|scope| T35_usecase_usecase_ReviewScopeSelector__self
T45_usecase_usecase_PreReviewCommandDispatchError_Config --o T38_usecase_usecase_CommandConfigLoadError__self
T45_usecase_usecase_PreReviewCommandDispatchError_TrackResolution --o T46_usecase_usecase_CurrentReviewTrackResolveError__self
T45_usecase_usecase_PreReviewCommandDispatchError_Runner --o T34_usecase_usecase_ProgramRunnerError__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --o R48_usecase_usecase_PreReviewCommandConfigLoaderPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --o R46_usecase_usecase_CurrentReviewTrackResolverPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --o R33_usecase_usecase_ProgramRunnerPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor_new --> T50_usecase_usecase_PreReviewCommandDispatchInteractor__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor__self --o|config_loader| R48_usecase_usecase_PreReviewCommandConfigLoaderPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor__self --o|track_resolver| R46_usecase_usecase_CurrentReviewTrackResolverPort__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor__self --o|runner| R33_usecase_usecase_ProgramRunnerPort__self
T47_usecase_usecase_PreReviewCommandDispatchOutcome_ReadyForReview --o|records| T38_usecase_usecase_ProgramExecutionRecord__self
T47_usecase_usecase_PreReviewCommandDispatchOutcome_Blocked --o|completed| T38_usecase_usecase_ProgramExecutionRecord__self
T47_usecase_usecase_PreReviewCommandDispatchOutcome_Blocked --o|failed| T38_usecase_usecase_ProgramExecutionRecord__self
T53_usecase_usecase_PreReviewCommandGatedReviewInteractor_new --o R47_usecase_usecase_PreReviewCommandDispatchService__self
T53_usecase_usecase_PreReviewCommandGatedReviewInteractor_new --> T53_usecase_usecase_PreReviewCommandGatedReviewInteractor__self
T53_usecase_usecase_PreReviewCommandGatedReviewInteractor__self --o|dispatcher| R47_usecase_usecase_PreReviewCommandDispatchService__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_new --o T33_usecase_usecase_ConfiguredCommand__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_new --> T48_usecase_usecase_PreReviewScopeCommandDeclaration__self
T48_usecase_usecase_PreReviewScopeCommandDeclaration_commands --> T33_usecase_usecase_ConfiguredCommand__self
R46_usecase_usecase_CurrentReviewTrackResolverPort_resolve --> T46_usecase_usecase_CurrentReviewTrackResolveError__self
R48_usecase_usecase_PreReviewCommandConfigLoaderPort_load --> T38_usecase_usecase_CommandConfigLoadError__self
R48_usecase_usecase_PreReviewCommandConfigLoaderPort_load --> T38_usecase_usecase_PreReviewCommandConfig__self
R47_usecase_usecase_PreReviewCommandDispatchService_dispatch --o T47_usecase_usecase_PreReviewCommandDispatchCommand__self
R47_usecase_usecase_PreReviewCommandDispatchService_dispatch --> T45_usecase_usecase_PreReviewCommandDispatchError__self
R47_usecase_usecase_PreReviewCommandDispatchService_dispatch --> T47_usecase_usecase_PreReviewCommandDispatchOutcome__self
T38_usecase_usecase_ProgramExecutionRecord__self --o|sequence_index| T36_usecase_usecase_CommandSequenceIndex__self
T38_usecase_usecase_ProgramExecutionRecord__self --o|command| T33_usecase_usecase_ConfiguredCommand__self
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
T38_usecase_usecase_PhaseCommandInteractor__self -.impl.-> R35_usecase_usecase_PhaseCommandService__self
T50_usecase_usecase_PreReviewCommandDispatchInteractor__self -.impl.-> R47_usecase_usecase_PreReviewCommandDispatchService__self
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
T53_infrastructure_infrastructure_TaskContractDocumentDto__self --o|schema_version| T58_infrastructure_infrastructure_TaskContractSchemaVersionDto__self
T53_infrastructure_infrastructure_TaskContractDocumentDto__self --o|entries| T51_infrastructure_infrastructure_ContractedEntryRefDto__self
T56_infrastructure_infrastructure_FsPhaseCommandConfigLoader__self -.impl.-> R44_usecase_usecase_PhaseCommandConfigLoaderPort__self
T60_infrastructure_infrastructure_FsPreReviewCommandConfigLoader__self -.impl.-> R48_usecase_usecase_PreReviewCommandConfigLoaderPort__self
T50_infrastructure_infrastructure_ProcessProgramRunner__self -.impl.-> R33_usecase_usecase_ProgramRunnerPort__self
T59_infrastructure_infrastructure_GitCurrentReviewTrackResolver__self -.impl.-> R46_usecase_usecase_CurrentReviewTrackResolverPort__self
T40_cli_driver_cli_driver_PhaseCommandDriver_new --o R35_usecase_usecase_PhaseCommandService__self
T40_cli_driver_cli_driver_PhaseCommandDriver_new --> T40_cli_driver_cli_driver_PhaseCommandDriver__self
T40_cli_driver_cli_driver_PhaseCommandDriver_handle --o T39_cli_driver_cli_driver_PhaseCommandInput__self
T40_cli_driver_cli_driver_PhaseCommandDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T40_cli_driver_cli_driver_PhaseCommandDriver__self --o|service| R35_usecase_usecase_PhaseCommandService__self
T39_cli_driver_cli_driver_PhaseCommandInput_Explain --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
T39_cli_driver_cli_driver_PhaseCommandInput_Enter --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
T32_cli_driver_cli_driver_PhaseIdArg_new --o T36_usecase_usecase_CommandDeclarationId__self
T32_cli_driver_cli_driver_PhaseIdArg_new --> T32_cli_driver_cli_driver_PhaseIdArg__self
T32_cli_driver_cli_driver_PhaseIdArg_as_declaration_id --> T36_usecase_usecase_CommandDeclarationId__self
T32_cli_driver_cli_driver_PhaseIdArg__self --o|value| T36_usecase_usecase_CommandDeclarationId__self
T36_cli_driver_cli_driver_CommandOutcome_success --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_CommandOutcome_failure --> T36_cli_driver_cli_driver_CommandOutcome__self
T52_cli_composition_cli_composition_PhaseCompositionRoot_build --> T40_cli_driver_cli_driver_PhaseCommandDriver__self
T20_cli_cli_PhaseCommand_Validate --o T25_cli_cli_PhaseValidateArgs__self
T20_cli_cli_PhaseCommand_Explain --o T24_cli_cli_PhaseExplainArgs__self
T20_cli_cli_PhaseCommand_Enter --o T22_cli_cli_PhaseEnterArgs__self
T22_cli_cli_PhaseEnterArgs__self --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
T24_cli_cli_PhaseExplainArgs__self --o|phase_id| T32_cli_driver_cli_driver_PhaseIdArg__self
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
class T31_usecase_usecase_CommandArgument_try_new method_node
class T31_usecase_usecase_CommandArgument_as_str method_node
class T31_usecase_usecase_CommandArgument__self value_object
class T27_usecase_usecase_CommandArgv_try_new method_node
class T27_usecase_usecase_CommandArgv_arguments method_node
class T27_usecase_usecase_CommandArgv__self value_object
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
class T44_usecase_usecase_CommandConfigValidationError__self error_type
class T36_usecase_usecase_CommandDeclarationId_try_new method_node
class T36_usecase_usecase_CommandDeclarationId_as_str method_node
class T36_usecase_usecase_CommandDeclarationId__self value_object
class T36_usecase_usecase_CommandSequenceIndex_new method_node
class T36_usecase_usecase_CommandSequenceIndex_as_usize method_node
class T36_usecase_usecase_CommandSequenceIndex__self value_object
class T37_usecase_usecase_CommandTimeoutSeconds_try_new method_node
class T37_usecase_usecase_CommandTimeoutSeconds_default_max method_node
class T37_usecase_usecase_CommandTimeoutSeconds_as_secs method_node
class T37_usecase_usecase_CommandTimeoutSeconds__self value_object
class T33_usecase_usecase_ConfiguredCommand_try_new method_node
class T33_usecase_usecase_ConfiguredCommand_argv method_node
class T33_usecase_usecase_ConfiguredCommand_timeout method_node
class T33_usecase_usecase_ConfiguredCommand__self value_object
class T39_usecase_usecase_OutputCaptureLimitBytes_one_mebibyte method_node
class T39_usecase_usecase_OutputCaptureLimitBytes_as_usize method_node
class T39_usecase_usecase_OutputCaptureLimitBytes__self value_object
class T41_usecase_usecase_UnvalidatedTimeoutSeconds_new method_node
class T41_usecase_usecase_UnvalidatedTimeoutSeconds_as_u64 method_node
class T41_usecase_usecase_UnvalidatedTimeoutSeconds__self value_object
class T34_usecase_usecase_PhaseCommandConfig_try_new method_node
class T34_usecase_usecase_PhaseCommandConfig_declaration method_node
class T34_usecase_usecase_PhaseCommandConfig__self value_object
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
class T32_usecase_usecase_PhaseCommandStep_PreEntry variant_node
class T32_usecase_usecase_PhaseCommandStep_Writer variant_node
class T32_usecase_usecase_PhaseCommandStep__self value_object
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
class T47_usecase_usecase_PreReviewCommandDispatchCommand__self command
class T45_usecase_usecase_PreReviewCommandDispatchError_Config variant_node
class T45_usecase_usecase_PreReviewCommandDispatchError_UnknownScope variant_node
class T45_usecase_usecase_PreReviewCommandDispatchError_TrackResolution variant_node
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
class R33_usecase_usecase_ProgramRunnerPort_run method_node
class R33_usecase_usecase_ProgramRunnerPort__self secondary_port
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
class T51_infrastructure_infrastructure_ContractedEntryRefDto__self dto
class T53_infrastructure_infrastructure_TaskContractDocumentDto__self dto
class T58_infrastructure_infrastructure_TaskContractSchemaVersionDto__self dto
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
class T36_cli_driver_cli_driver_CommandOutcome_success method_node
class T36_cli_driver_cli_driver_CommandOutcome_failure method_node
class T36_cli_driver_cli_driver_CommandOutcome__self dto
class T52_cli_composition_cli_composition_PhaseCompositionRoot_build method_node
class T52_cli_composition_cli_composition_PhaseCompositionRoot__self composition_root
class T20_cli_cli_PhaseCommand_Validate variant_node
class T20_cli_cli_PhaseCommand_Explain variant_node
class T20_cli_cli_PhaseCommand_Enter variant_node
class T20_cli_cli_PhaseCommand__self dto
class T22_cli_cli_PhaseEnterArgs__self dto
class T24_cli_cli_PhaseExplainArgs__self dto
class T25_cli_cli_PhaseValidateArgs__self dto
```
