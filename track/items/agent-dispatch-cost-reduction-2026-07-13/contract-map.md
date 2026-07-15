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
  subgraph domain_domain_module_ids["domain::ids"]
    direction TB
  subgraph T25_domain_domain_TrackBranch["ids::TrackBranch"]
    direction TB
    T25_domain_domain_TrackBranch__self[TrackBranch]
    T25_domain_domain_TrackBranch_try_new([try_new])
  end
  subgraph T21_domain_domain_TrackId["ids::TrackId"]
    direction TB
    T21_domain_domain_TrackId__self[TrackId]
    T21_domain_domain_TrackId_try_new([try_new])
  end
  end
  subgraph domain_domain_module_review_v2["domain::review_v2"]
    direction TB
  subgraph T23_domain_domain_RoundType["review_v2::types::RoundType"]
    direction TB
    T23_domain_domain_RoundType__self[RoundType]
    T23_domain_domain_RoundType_Fast[Fast]
    T23_domain_domain_RoundType_Final[Final]
  end
  subgraph T23_domain_domain_ScopeName["review_v2::types::ScopeName"]
    direction TB
    T23_domain_domain_ScopeName__self[ScopeName]
    T23_domain_domain_ScopeName_Main[Main]
    T23_domain_domain_ScopeName_Other[Other]
  end
  end
  subgraph domain_domain_module_schema["domain::schema"]
    direction TB
  subgraph R28_domain_domain_SchemaExporter["schema::SchemaExporter"]
    direction TB
    R28_domain_domain_SchemaExporter__self[SchemaExporter]
    R28_domain_domain_SchemaExporter_export([export])
  end
  end
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T38_domain_domain_CatalogueDeclarationHash["tddd::type_signals_doc::CatalogueDeclarationHash"]
    direction TB
    T38_domain_domain_CatalogueDeclarationHash__self[CatalogueDeclarationHash]
    T38_domain_domain_CatalogueDeclarationHash_new([new])
    T38_domain_domain_CatalogueDeclarationHash_as_digest([as_digest])
  end
  subgraph T27_domain_domain_EdgeOwnership["tddd::test_obligation::obligations::EdgeOwnership"]
    direction TB
    T27_domain_domain_EdgeOwnership__self[EdgeOwnership]
    T27_domain_domain_EdgeOwnership_None[None]
    T27_domain_domain_EdgeOwnership_Unique[Unique]
    T27_domain_domain_EdgeOwnership_Multiple[Multiple]
  end
  subgraph T37_domain_domain_ImplementationInputHash["tddd::type_signals_doc::ImplementationInputHash"]
    direction TB
    T37_domain_domain_ImplementationInputHash__self[ImplementationInputHash]
    T37_domain_domain_ImplementationInputHash_new([new])
    T37_domain_domain_ImplementationInputHash_as_digest([as_digest])
  end
  subgraph T21_domain_domain_LayerId["tddd::layer_id::LayerId"]
    direction TB
    T21_domain_domain_LayerId__self[LayerId]
    T21_domain_domain_LayerId_try_new([try_new])
  end
  subgraph T33_domain_domain_ObligationsDocument["tddd::test_obligation::obligations::ObligationsDocument"]
    direction TB
    T33_domain_domain_ObligationsDocument__self[ObligationsDocument]
    T33_domain_domain_ObligationsDocument_new([new])
    T33_domain_domain_ObligationsDocument_track_id([track_id])
    T33_domain_domain_ObligationsDocument_obligations([obligations])
    T33_domain_domain_ObligationsDocument_edge_ownership([edge_ownership])
    T33_domain_domain_ObligationsDocument_owners_of_edge([owners_of_edge])
    T33_domain_domain_ObligationsDocument_staleness_against([staleness_against])
  end
  subgraph T26_domain_domain_Sha256Digest["tddd::type_signals_doc::Sha256Digest"]
    direction TB
    T26_domain_domain_Sha256Digest__self[Sha256Digest]
    T26_domain_domain_Sha256Digest_try_new([try_new])
    T26_domain_domain_Sha256Digest_from_content_hash([from_content_hash])
    T26_domain_domain_Sha256Digest_as_str([as_str])
  end
  subgraph T31_domain_domain_Sha256DigestError["tddd::type_signals_doc::Sha256DigestError"]
    direction TB
    T31_domain_domain_Sha256DigestError__self[Sha256DigestError]
    T31_domain_domain_Sha256DigestError_InvalidLength[InvalidLength]
    T31_domain_domain_Sha256DigestError_InvalidHex[InvalidHex]
  end
  subgraph T34_domain_domain_TestBindingsDocument["tddd::test_obligation::binding::TestBindingsDocument"]
    direction TB
    T34_domain_domain_TestBindingsDocument__self[TestBindingsDocument]
    T34_domain_domain_TestBindingsDocument_new([new])
    T34_domain_domain_TestBindingsDocument_track_id([track_id])
    T34_domain_domain_TestBindingsDocument_records([records])
    T34_domain_domain_TestBindingsDocument_waived_edge_ids([waived_edge_ids])
    T34_domain_domain_TestBindingsDocument_is_edge_waived([is_edge_waived])
  end
  subgraph T28_domain_domain_TestObligation["tddd::test_obligation::obligations::TestObligation"]
    direction TB
    T28_domain_domain_TestObligation__self[TestObligation]
    T28_domain_domain_TestObligation_new([new])
    T28_domain_domain_TestObligation_id([id])
    T28_domain_domain_TestObligation_target_entry([target_entry])
    T28_domain_domain_TestObligation_target_role([target_role])
    T28_domain_domain_TestObligation_brief([brief])
    T28_domain_domain_TestObligation_declaration_hash([declaration_hash])
    T28_domain_domain_TestObligation_spec_refs([spec_refs])
    T28_domain_domain_TestObligation_owns_edge([owns_edge])
  end
  subgraph T33_domain_domain_TypeSignalsDocument["tddd::type_signals_doc::TypeSignalsDocument"]
    direction TB
    T33_domain_domain_TypeSignalsDocument__self[TypeSignalsDocument]
    T33_domain_domain_TypeSignalsDocument_new([new])
    T33_domain_domain_TypeSignalsDocument_with_schema_version([with_schema_version])
    T33_domain_domain_TypeSignalsDocument_schema_version([schema_version])
    T33_domain_domain_TypeSignalsDocument_generated_at([generated_at])
    T33_domain_domain_TypeSignalsDocument_declaration_hash([declaration_hash])
    T33_domain_domain_TypeSignalsDocument_implementation_input_hash([implementation_input_hash])
    T33_domain_domain_TypeSignalsDocument_signals([signals])
  end
  subgraph T35_domain_domain_TypeSignalsLoadResult["tddd::type_signals_doc::TypeSignalsLoadResult"]
    direction TB
    T35_domain_domain_TypeSignalsLoadResult__self[TypeSignalsLoadResult]
    T35_domain_domain_TypeSignalsLoadResult_Current[Current]
    T35_domain_domain_TypeSignalsLoadResult_Stale[Stale]
    T35_domain_domain_TypeSignalsLoadResult_Missing[Missing]
    T35_domain_domain_TypeSignalsLoadResult_as_current([as_current])
    T35_domain_domain_TypeSignalsLoadResult_is_current([is_current])
    T35_domain_domain_TypeSignalsLoadResult_is_stale([is_stale])
    T35_domain_domain_TypeSignalsLoadResult_is_missing([is_missing])
  end
  subgraph T38_domain_domain_TypeSignalsReuseDecision["tddd::type_signals_doc::TypeSignalsReuseDecision"]
    direction TB
    T38_domain_domain_TypeSignalsReuseDecision__self[TypeSignalsReuseDecision]
    T38_domain_domain_TypeSignalsReuseDecision_SkipEvaluation[SkipEvaluation]
    T38_domain_domain_TypeSignalsReuseDecision_ReevaluateWithoutExtraction[ReevaluateWithoutExtraction]
    T38_domain_domain_TypeSignalsReuseDecision_ReextractAndEvaluate[ReextractAndEvaluate]
  end
  subgraph T38_domain_domain_TypeSignalsSchemaVersion["tddd::type_signals_doc::TypeSignalsSchemaVersion"]
    direction TB
    T38_domain_domain_TypeSignalsSchemaVersion__self[TypeSignalsSchemaVersion]
    T38_domain_domain_TypeSignalsSchemaVersion_try_new([try_new])
    T38_domain_domain_TypeSignalsSchemaVersion_value([value])
  end
  subgraph T43_domain_domain_TypeSignalsSchemaVersionError["tddd::type_signals_doc::TypeSignalsSchemaVersionError"]
    direction TB
    T43_domain_domain_TypeSignalsSchemaVersionError__self[TypeSignalsSchemaVersionError]
    T43_domain_domain_TypeSignalsSchemaVersionError_Zero[Zero]
  end
  F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse[[decide_type_signals_reuse]]
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_capability_exec["usecase::capability_exec"]
    direction TB
  subgraph T41_usecase_usecase_CapabilityDispatchOutcome["capability_exec::CapabilityDispatchOutcome"]
    direction TB
    T41_usecase_usecase_CapabilityDispatchOutcome__self[CapabilityDispatchOutcome]
    T41_usecase_usecase_CapabilityDispatchOutcome_Executed[Executed]
    T41_usecase_usecase_CapabilityDispatchOutcome_DelegateInHost[DelegateInHost]
  end
  subgraph T41_usecase_usecase_CapabilityDispatchRequest["capability_exec::CapabilityDispatchRequest"]
    direction TB
    T41_usecase_usecase_CapabilityDispatchRequest__self[CapabilityDispatchRequest]
  end
  subgraph T35_usecase_usecase_CapabilityExecError["capability_exec::CapabilityExecError"]
    direction TB
    T35_usecase_usecase_CapabilityExecError__self[CapabilityExecError]
    T35_usecase_usecase_CapabilityExecError_ProfileResolution[ProfileResolution]
    T35_usecase_usecase_CapabilityExecError_ExecutionModeRejected[ExecutionModeRejected]
    T35_usecase_usecase_CapabilityExecError_ModelMissing[ModelMissing]
    T35_usecase_usecase_CapabilityExecError_EffortMissing[EffortMissing]
    T35_usecase_usecase_CapabilityExecError_UnsupportedProvider[UnsupportedProvider]
    T35_usecase_usecase_CapabilityExecError_SourceValidation[SourceValidation]
    T35_usecase_usecase_CapabilityExecError_AdapterPreflight[AdapterPreflight]
    T35_usecase_usecase_CapabilityExecError_DispatchFailed[DispatchFailed]
  end
  subgraph T37_usecase_usecase_CapabilityExecRequest["capability_exec::CapabilityExecRequest"]
    direction TB
    T37_usecase_usecase_CapabilityExecRequest__self[CapabilityExecRequest]
  end
  subgraph T39_usecase_usecase_CapabilityFailureDetail["capability_exec::CapabilityFailureDetail"]
    direction TB
    T39_usecase_usecase_CapabilityFailureDetail__self[CapabilityFailureDetail]
    T39_usecase_usecase_CapabilityFailureDetail_new([new])
    T39_usecase_usecase_CapabilityFailureDetail_as_str([as_str])
  end
  subgraph T34_usecase_usecase_CapabilityFilePath["capability_exec::CapabilityFilePath"]
    direction TB
    T34_usecase_usecase_CapabilityFilePath__self[CapabilityFilePath]
    T34_usecase_usecase_CapabilityFilePath_try_new([try_new])
    T34_usecase_usecase_CapabilityFilePath_as_path([as_path])
  end
  subgraph T46_usecase_usecase_CapabilityInputValidationError["capability_exec::CapabilityInputValidationError"]
    direction TB
    T46_usecase_usecase_CapabilityInputValidationError__self[CapabilityInputValidationError]
    T46_usecase_usecase_CapabilityInputValidationError_EmptyProviderName[EmptyProviderName]
    T46_usecase_usecase_CapabilityInputValidationError_EmptyModelName[EmptyModelName]
    T46_usecase_usecase_CapabilityInputValidationError_EmptyFilePath[EmptyFilePath]
    T46_usecase_usecase_CapabilityInputValidationError_InvalidFilePath[InvalidFilePath]
    T46_usecase_usecase_CapabilityInputValidationError_EmptyContent[EmptyContent]
    T46_usecase_usecase_CapabilityInputValidationError_ZeroTimeoutSeconds[ZeroTimeoutSeconds]
    T46_usecase_usecase_CapabilityInputValidationError_EmptyTargetArtifactSet[EmptyTargetArtifactSet]
  end
  subgraph T33_usecase_usecase_CapabilityProfile["capability_exec::CapabilityProfile"]
    direction TB
    T33_usecase_usecase_CapabilityProfile__self[CapabilityProfile]
  end
  subgraph T39_usecase_usecase_CapabilityResumeRequest["capability_exec::CapabilityResumeRequest"]
    direction TB
    T39_usecase_usecase_CapabilityResumeRequest__self[CapabilityResumeRequest]
    T39_usecase_usecase_CapabilityResumeRequest_Fresh[Fresh]
    T39_usecase_usecase_CapabilityResumeRequest_ResumeWithoutTarget[ResumeWithoutTarget]
    T39_usecase_usecase_CapabilityResumeRequest_Resume[Resume]
  end
  subgraph T25_usecase_usecase_ModelName["capability_exec::ModelName"]
    direction TB
    T25_usecase_usecase_ModelName__self[ModelName]
    T25_usecase_usecase_ModelName_try_new([try_new])
    T25_usecase_usecase_ModelName_as_str([as_str])
  end
  subgraph T28_usecase_usecase_ProviderName["capability_exec::ProviderName"]
    direction TB
    T28_usecase_usecase_ProviderName__self[ProviderName]
    T28_usecase_usecase_ProviderName_try_new([try_new])
    T28_usecase_usecase_ProviderName_as_str([as_str])
  end
  subgraph T31_usecase_usecase_ReasoningEffort["capability_exec::ReasoningEffort"]
    direction TB
    T31_usecase_usecase_ReasoningEffort__self[ReasoningEffort]
    T31_usecase_usecase_ReasoningEffort_Low[Low]
    T31_usecase_usecase_ReasoningEffort_Medium[Medium]
    T31_usecase_usecase_ReasoningEffort_High[High]
    T31_usecase_usecase_ReasoningEffort_XHigh[XHigh]
    T31_usecase_usecase_ReasoningEffort_Max[Max]
  end
  subgraph T34_usecase_usecase_TargetArtifactPath["capability_exec::TargetArtifactPath"]
    direction TB
    T34_usecase_usecase_TargetArtifactPath__self[TargetArtifactPath]
    T34_usecase_usecase_TargetArtifactPath_try_new([try_new])
    T34_usecase_usecase_TargetArtifactPath_as_path([as_path])
  end
  subgraph T33_usecase_usecase_TargetArtifactSet["capability_exec::TargetArtifactSet"]
    direction TB
    T33_usecase_usecase_TargetArtifactSet__self[TargetArtifactSet]
    T33_usecase_usecase_TargetArtifactSet_try_new([try_new])
    T33_usecase_usecase_TargetArtifactSet_as_slice([as_slice])
  end
  subgraph T30_usecase_usecase_TimeoutSeconds["capability_exec::TimeoutSeconds"]
    direction TB
    T30_usecase_usecase_TimeoutSeconds__self[TimeoutSeconds]
    T30_usecase_usecase_TimeoutSeconds_try_new([try_new])
    T30_usecase_usecase_TimeoutSeconds_as_secs([as_secs])
  end
  subgraph R37_usecase_usecase_CapabilityProfilePort["capability_exec::CapabilityProfilePort"]
    direction TB
    R37_usecase_usecase_CapabilityProfilePort__self[CapabilityProfilePort]
    R37_usecase_usecase_CapabilityProfilePort_resolve([resolve])
  end
  subgraph R38_usecase_usecase_CapabilityProviderPort["capability_exec::CapabilityProviderPort"]
    direction TB
    R38_usecase_usecase_CapabilityProviderPort__self[CapabilityProviderPort]
    R38_usecase_usecase_CapabilityProviderPort_provider([provider])
    R38_usecase_usecase_CapabilityProviderPort_dispatch([dispatch])
  end
  end
  subgraph usecase_usecase_module_dry_write_driver["usecase::dry_write_driver"]
    direction TB
  subgraph T30_usecase_usecase_CapabilityName["dry_write_driver::failure_details::CapabilityName"]
    direction TB
    T30_usecase_usecase_CapabilityName__self[CapabilityName]
    T30_usecase_usecase_CapabilityName_try_new([try_new])
    T30_usecase_usecase_CapabilityName_as_str([as_str])
  end
  end
  subgraph usecase_usecase_module_export_schema["usecase::export_schema"]
    direction TB
  subgraph R34_usecase_usecase_SchemaExporterPort["export_schema::SchemaExporterPort"]
    direction TB
    R34_usecase_usecase_SchemaExporterPort__self[SchemaExporterPort]
    R34_usecase_usecase_SchemaExporterPort_export_as_json([export_as_json])
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
  subgraph usecase_usecase_module_provider_session["usecase::provider_session"]
    direction TB
  subgraph T41_usecase_usecase_ProviderSessionCacheEntry["provider_session::ProviderSessionCacheEntry"]
    direction TB
    T41_usecase_usecase_ProviderSessionCacheEntry__self[ProviderSessionCacheEntry]
    T41_usecase_usecase_ProviderSessionCacheEntry_new([new])
    T41_usecase_usecase_ProviderSessionCacheEntry_session_id([session_id])
    T41_usecase_usecase_ProviderSessionCacheEntry_provider([provider])
    T41_usecase_usecase_ProviderSessionCacheEntry_model([model])
    T41_usecase_usecase_ProviderSessionCacheEntry_effort([effort])
  end
  subgraph T41_usecase_usecase_ProviderSessionCacheError["provider_session::ProviderSessionCacheError"]
    direction TB
    T41_usecase_usecase_ProviderSessionCacheError__self[ProviderSessionCacheError]
    T41_usecase_usecase_ProviderSessionCacheError_StorageUnavailable[StorageUnavailable]
    T41_usecase_usecase_ProviderSessionCacheError_EntryInvalid[EntryInvalid]
    T41_usecase_usecase_ProviderSessionCacheError_IdentityBoundaryViolation[IdentityBoundaryViolation]
  end
  subgraph T39_usecase_usecase_ProviderSessionCacheKey["provider_session::ProviderSessionCacheKey"]
    direction TB
    T39_usecase_usecase_ProviderSessionCacheKey__self[ProviderSessionCacheKey]
    T39_usecase_usecase_ProviderSessionCacheKey_Review[Review]
    T39_usecase_usecase_ProviderSessionCacheKey_TrackCapability[TrackCapability]
    T39_usecase_usecase_ProviderSessionCacheKey_WorkspaceCapability[WorkspaceCapability]
  end
  subgraph T33_usecase_usecase_ProviderSessionId["provider_session::ProviderSessionId"]
    direction TB
    T33_usecase_usecase_ProviderSessionId__self[ProviderSessionId]
    T33_usecase_usecase_ProviderSessionId_try_new([try_new])
    T33_usecase_usecase_ProviderSessionId_as_str([as_str])
  end
  subgraph T30_usecase_usecase_ReviewerPrompt["provider_session::ReviewerPrompt"]
    direction TB
    T30_usecase_usecase_ReviewerPrompt__self[ReviewerPrompt]
    T30_usecase_usecase_ReviewerPrompt_try_new([try_new])
    T30_usecase_usecase_ReviewerPrompt_as_str([as_str])
  end
  subgraph R40_usecase_usecase_ProviderSessionCachePort["provider_session::ProviderSessionCachePort"]
    direction TB
    R40_usecase_usecase_ProviderSessionCachePort__self[ProviderSessionCachePort]
    R40_usecase_usecase_ProviderSessionCachePort_load([load])
    R40_usecase_usecase_ProviderSessionCachePort_save([save])
    R40_usecase_usecase_ProviderSessionCachePort_remove([remove])
  end
  end
  subgraph usecase_usecase_module_review_v2["usecase::review_v2"]
    direction TB
  subgraph R24_usecase_usecase_Reviewer["review_v2::ports::Reviewer"]
    direction TB
    R24_usecase_usecase_Reviewer__self[Reviewer]
    R24_usecase_usecase_Reviewer_review([review])
    R24_usecase_usecase_Reviewer_fast_review([fast_review])
  end
  end
  subgraph usecase_usecase_module_type_signals["usecase::type_signals"]
    direction TB
  subgraph T32_usecase_usecase_TypeSignalsError["type_signals::service::TypeSignalsError"]
    direction TB
    T32_usecase_usecase_TypeSignalsError__self[TypeSignalsError]
    T32_usecase_usecase_TypeSignalsError_BranchTrackMismatch[BranchTrackMismatch]
    T32_usecase_usecase_TypeSignalsError_LayerBindingsLoad[LayerBindingsLoad]
    T32_usecase_usecase_TypeSignalsError_NoLayers[NoLayers]
    T32_usecase_usecase_TypeSignalsError_EvaluationFailed[EvaluationFailed]
    T32_usecase_usecase_TypeSignalsError_InconsistentRequest[InconsistentRequest]
  end
  subgraph T41_usecase_usecase_TypeSignalsExecutionError["type_signals::ports::TypeSignalsExecutionError"]
    direction TB
    T41_usecase_usecase_TypeSignalsExecutionError__self[TypeSignalsExecutionError]
  end
  subgraph T37_usecase_usecase_TypeSignalsInteractor["type_signals::interactor::TypeSignalsInteractor"]
    direction TB
    T37_usecase_usecase_TypeSignalsInteractor__self[TypeSignalsInteractor]
    T37_usecase_usecase_TypeSignalsInteractor_new([new])
  end
  subgraph T34_usecase_usecase_TypeSignalsRequest["type_signals::service::TypeSignalsRequest"]
    direction TB
    T34_usecase_usecase_TypeSignalsRequest__self[TypeSignalsRequest]
  end
  subgraph R39_usecase_usecase_TypeSignalsExecutorPort["type_signals::ports::TypeSignalsExecutorPort"]
    direction TB
    R39_usecase_usecase_TypeSignalsExecutorPort__self[TypeSignalsExecutorPort]
    R39_usecase_usecase_TypeSignalsExecutorPort_evaluate_layer([evaluate_layer])
  end
  subgraph R34_usecase_usecase_TypeSignalsService["type_signals::service::TypeSignalsService"]
    direction TB
    R34_usecase_usecase_TypeSignalsService__self[TypeSignalsService]
    R34_usecase_usecase_TypeSignalsService_run([run])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_agent_profiles["infrastructure::agent_profiles"]
    direction TB
  subgraph T43_infrastructure_infrastructure_AgentProfiles["agent_profiles::AgentProfiles"]
    direction TB
    T43_infrastructure_infrastructure_AgentProfiles__self[AgentProfiles]
    T43_infrastructure_infrastructure_AgentProfiles_load([load])
    T43_infrastructure_infrastructure_AgentProfiles_resolve_capability([resolve_capability])
    T43_infrastructure_infrastructure_AgentProfiles_resolve_execution([resolve_execution])
    T43_infrastructure_infrastructure_AgentProfiles_provider_label([provider_label])
    T43_infrastructure_infrastructure_AgentProfiles_resolve_prompt_template_path([resolve_prompt_template_path])
  end
  subgraph T48_infrastructure_infrastructure_AgentProfilesError["agent_profiles::AgentProfilesError"]
    direction TB
    T48_infrastructure_infrastructure_AgentProfilesError__self[AgentProfilesError]
    T48_infrastructure_infrastructure_AgentProfilesError_Io[Io]
    T48_infrastructure_infrastructure_AgentProfilesError_Symlink[Symlink]
    T48_infrastructure_infrastructure_AgentProfilesError_PathOutsideTrustedRoot[PathOutsideTrustedRoot]
    T48_infrastructure_infrastructure_AgentProfilesError_Parse[Parse]
    T48_infrastructure_infrastructure_AgentProfilesError_UnsupportedSchemaVersion[UnsupportedSchemaVersion]
    T48_infrastructure_infrastructure_AgentProfilesError_InvalidCapability[InvalidCapability]
    T48_infrastructure_infrastructure_AgentProfilesError_CapabilityNotFound[CapabilityNotFound]
    T48_infrastructure_infrastructure_AgentProfilesError_ModelMissing[ModelMissing]
    T48_infrastructure_infrastructure_AgentProfilesError_EffortMissing[EffortMissing]
    T48_infrastructure_infrastructure_AgentProfilesError_UnsupportedEffort[UnsupportedEffort]
  end
  subgraph T49_infrastructure_infrastructure_CapabilityConfigDto["agent_profiles::CapabilityConfigDto"]
    direction TB
    T49_infrastructure_infrastructure_CapabilityConfigDto__self[CapabilityConfigDto]
    T49_infrastructure_infrastructure_CapabilityConfigDto_provider([provider])
    T49_infrastructure_infrastructure_CapabilityConfigDto_model([model])
    T49_infrastructure_infrastructure_CapabilityConfigDto_fast_provider([fast_provider])
    T49_infrastructure_infrastructure_CapabilityConfigDto_fast_model([fast_model])
    T49_infrastructure_infrastructure_CapabilityConfigDto_prompt_template_path([prompt_template_path])
    T49_infrastructure_infrastructure_CapabilityConfigDto_effort([effort])
    T49_infrastructure_infrastructure_CapabilityConfigDto_fast_effort([fast_effort])
    T49_infrastructure_infrastructure_CapabilityConfigDto_execution_mode([execution_mode])
  end
  subgraph T46_infrastructure_infrastructure_ExecutionModeDto["agent_profiles::ExecutionModeDto"]
    direction TB
    T46_infrastructure_infrastructure_ExecutionModeDto__self[ExecutionModeDto]
    T46_infrastructure_infrastructure_ExecutionModeDto_OrchestratorOutput[OrchestratorOutput]
    T46_infrastructure_infrastructure_ExecutionModeDto_TypedPipeline[TypedPipeline]
    T46_infrastructure_infrastructure_ExecutionModeDto_into_domain([into_domain])
  end
  subgraph T42_infrastructure_infrastructure_ModelNameDto["agent_profiles::ModelNameDto"]
    direction TB
    T42_infrastructure_infrastructure_ModelNameDto__self[ModelNameDto]
    T42_infrastructure_infrastructure_ModelNameDto_try_new([try_new])
    T42_infrastructure_infrastructure_ModelNameDto_into_domain([into_domain])
  end
  subgraph T45_infrastructure_infrastructure_ProviderNameDto["agent_profiles::ProviderNameDto"]
    direction TB
    T45_infrastructure_infrastructure_ProviderNameDto__self[ProviderNameDto]
    T45_infrastructure_infrastructure_ProviderNameDto_try_new([try_new])
    T45_infrastructure_infrastructure_ProviderNameDto_into_domain([into_domain])
  end
  subgraph T48_infrastructure_infrastructure_ReasoningEffortDto["agent_profiles::ReasoningEffortDto"]
    direction TB
    T48_infrastructure_infrastructure_ReasoningEffortDto__self[ReasoningEffortDto]
    T48_infrastructure_infrastructure_ReasoningEffortDto_Low[Low]
    T48_infrastructure_infrastructure_ReasoningEffortDto_Medium[Medium]
    T48_infrastructure_infrastructure_ReasoningEffortDto_High[High]
    T48_infrastructure_infrastructure_ReasoningEffortDto_XHigh[XHigh]
    T48_infrastructure_infrastructure_ReasoningEffortDto_Max[Max]
    T48_infrastructure_infrastructure_ReasoningEffortDto_into_domain([into_domain])
  end
  subgraph T47_infrastructure_infrastructure_ResolvedExecution["agent_profiles::ResolvedExecution"]
    direction TB
    T47_infrastructure_infrastructure_ResolvedExecution__self[ResolvedExecution]
    T47_infrastructure_infrastructure_ResolvedExecution_ProviderCli[ProviderCli]
    T47_infrastructure_infrastructure_ResolvedExecution_HostedService[HostedService]
  end
  subgraph T39_infrastructure_infrastructure_RoundType["agent_profiles::RoundType"]
    direction TB
    T39_infrastructure_infrastructure_RoundType__self[RoundType]
    T39_infrastructure_infrastructure_RoundType_Final[Final]
    T39_infrastructure_infrastructure_RoundType_Fast[Fast]
  end
  end
  subgraph infrastructure_infrastructure_module_capability_exec["infrastructure::capability_exec"]
    direction TB
  subgraph T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter["capability_exec::agent_profiles::AgentProfilesCapabilityAdapter"]
    direction TB
    T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter__self[AgentProfilesCapabilityAdapter]
    T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter_new([new])
  end
  subgraph T53_infrastructure_infrastructure_ClaudeCapabilityAdapter["capability_exec::claude::ClaudeCapabilityAdapter"]
    direction TB
    T53_infrastructure_infrastructure_ClaudeCapabilityAdapter__self[ClaudeCapabilityAdapter]
    T53_infrastructure_infrastructure_ClaudeCapabilityAdapter_new([new])
  end
  subgraph T52_infrastructure_infrastructure_CodexCapabilityAdapter["capability_exec::codex::CodexCapabilityAdapter"]
    direction TB
    T52_infrastructure_infrastructure_CodexCapabilityAdapter__self[CodexCapabilityAdapter]
    T52_infrastructure_infrastructure_CodexCapabilityAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_provider_session["infrastructure::provider_session"]
    direction TB
  subgraph T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter["provider_session::FsProviderSessionCacheAdapter"]
    direction TB
    T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter__self[FsProviderSessionCacheAdapter]
    T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_ref_verify["infrastructure::ref_verify"]
    direction TB
  F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_claude_ref_verifier_args[[build_claude_ref_verifier_args]]
  F103_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_codex_ref_verifier_args[[build_codex_ref_verifier_args]]
  F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_gemini_ref_verifier_args[[build_gemini_ref_verifier_args]]
  end
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  subgraph T44_infrastructure_infrastructure_ClaudeReviewer["review_v2::claude_reviewer::ClaudeReviewer"]
    direction TB
    T44_infrastructure_infrastructure_ClaudeReviewer__self[ClaudeReviewer]
    T44_infrastructure_infrastructure_ClaudeReviewer_new([new])
  end
  subgraph T43_infrastructure_infrastructure_CodexReviewer["review_v2::codex_reviewer::CodexReviewer"]
    direction TB
    T43_infrastructure_infrastructure_CodexReviewer__self[CodexReviewer]
    T43_infrastructure_infrastructure_CodexReviewer_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_schema_export["infrastructure::schema_export"]
    direction TB
  subgraph T51_infrastructure_infrastructure_RustdocSchemaExporter["schema_export::RustdocSchemaExporter"]
    direction TB
    T51_infrastructure_infrastructure_RustdocSchemaExporter__self[RustdocSchemaExporter]
    T51_infrastructure_infrastructure_RustdocSchemaExporter_new([new])
    T51_infrastructure_infrastructure_RustdocSchemaExporter_export_rustdoc_json_path([export_rustdoc_json_path])
    T51_infrastructure_infrastructure_RustdocSchemaExporter_existing_rustdoc_json_path([existing_rustdoc_json_path])
  end
  end
  subgraph infrastructure_infrastructure_module_tddd["infrastructure::tddd"]
    direction TB
  subgraph T50_infrastructure_infrastructure_EvaluateSignalsError["tddd::type_signals_evaluator::EvaluateSignalsError"]
    direction TB
    T50_infrastructure_infrastructure_EvaluateSignalsError__self[EvaluateSignalsError]
  end
  subgraph T51_infrastructure_infrastructure_TypeSignalsCodecError["tddd::type_signals_codec::TypeSignalsCodecError"]
    direction TB
    T51_infrastructure_infrastructure_TypeSignalsCodecError__self[TypeSignalsCodecError]
    T51_infrastructure_infrastructure_TypeSignalsCodecError_Json[Json]
    T51_infrastructure_infrastructure_TypeSignalsCodecError_UnsupportedSchemaVersion[UnsupportedSchemaVersion]
    T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidSchemaVersion[InvalidSchemaVersion]
    T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidTimestamp[InvalidTimestamp]
    T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidDigest[InvalidDigest]
  end
  subgraph T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter["tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter"]
    direction TB
    T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter__self[TypeSignalsExecutorAdapter]
    T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter_new([new])
  end
  F88_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__declaration_hash[[declaration_hash]]
  F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__decode[[decode]]
  F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__encode[[encode]]
  F106_infrastructure_infrastructure_infrastructure__tddd__type_signals_evaluator__execute_type_signals_for_layer[[execute_type_signals_for_layer]]
  end
  subgraph infrastructure_infrastructure_module_type_catalogue_render["infrastructure::type_catalogue_render"]
    direction TB
  subgraph T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError["type_catalogue_render::LoadCatalogueSpecSignalsForViewError"]
    direction TB
    T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError__self[LoadCatalogueSpecSignalsForViewError]
    T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_NotFound[NotFound]
    T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_NotRegularFile[NotRegularFile]
    T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_Io[Io]
    T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_Decode[Decode]
    T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_StaleHash[StaleHash]
  end
  end
  subgraph infrastructure_infrastructure_module_verify["infrastructure::verify"]
    direction TB
  F112_infrastructure_infrastructure_infrastructure__verify__catalogue_spec_signals__compute_catalogue_declaration_hash[[compute_catalogue_declaration_hash]]
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
  subgraph T41_cli_driver_cli_driver_CapabilityResumeArg["capability::CapabilityResumeArg"]
    direction TB
    T41_cli_driver_cli_driver_CapabilityResumeArg__self[CapabilityResumeArg]
    T41_cli_driver_cli_driver_CapabilityResumeArg_Fresh[Fresh]
    T41_cli_driver_cli_driver_CapabilityResumeArg_ResumeWithoutTarget[ResumeWithoutTarget]
    T41_cli_driver_cli_driver_CapabilityResumeArg_Resume[Resume]
    T41_cli_driver_cli_driver_CapabilityResumeArg_into_domain([into_domain])
  end
  subgraph T43_cli_driver_cli_driver_TargetArtifactPathArg["capability::TargetArtifactPathArg"]
    direction TB
    T43_cli_driver_cli_driver_TargetArtifactPathArg__self[TargetArtifactPathArg]
    T43_cli_driver_cli_driver_TargetArtifactPathArg_into_domain([into_domain])
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
  subgraph cli_composition_cli_composition_module_review_v2["cli_composition::review_v2"]
    direction TB
  subgraph T53_cli_composition_cli_composition_ReviewCompositionRoot["review_v2::shim::ReviewCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_ReviewCompositionRoot__self[ReviewCompositionRoot]
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_codex([review_run_codex])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_claude([review_run_claude])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_local([review_run_local])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local([review_run_fix_local])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local_resolve([review_run_fix_local_resolve])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_check_approved([review_check_approved])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_results([review_results])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_classify([review_classify])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_files([review_files])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_validate_scope([review_validate_scope])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_get_briefing([review_get_briefing])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_persist_commit_hash([review_persist_commit_hash])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_new([new])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver([review_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_signal["cli_composition::signal"]
    direction TB
  subgraph T53_cli_composition_cli_composition_SignalCompositionRoot["signal::SignalCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_SignalCompositionRoot__self[SignalCompositionRoot]
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_gate([signal_check_gate])
    T53_cli_composition_cli_composition_SignalCompositionRoot_new([new])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_adr_user([signal_calc_adr_user])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_adr_user([signal_check_adr_user])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_spec_adr([signal_calc_spec_adr])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_spec_adr([signal_check_spec_adr])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_catalog_spec([signal_calc_catalog_spec])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_catalog_spec([signal_check_catalog_spec])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_impl_catalog([signal_calc_impl_catalog])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_impl_catalog([signal_check_impl_catalog])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver([signal_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T25_cli_cli_CapabilityCommand["commands::capability::CapabilityCommand"]
    direction TB
    T25_cli_cli_CapabilityCommand__self[CapabilityCommand]
    T25_cli_cli_CapabilityCommand_Exec[Exec]
  end
  subgraph T26_cli_cli_CapabilityExecArgs["commands::capability::CapabilityExecArgs"]
    direction TB
    T26_cli_cli_CapabilityExecArgs__self[CapabilityExecArgs]
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
  F42_cli_cli_cli__commands__capability__execute[[execute]]
  F38_cli_cli_cli__commands__review__execute[[execute]]
  end
end
T25_domain_domain_TrackBranch_try_new --> T25_domain_domain_TrackBranch__self
T21_domain_domain_TrackId_try_new --> T21_domain_domain_TrackId__self
T38_domain_domain_CatalogueDeclarationHash_new --o T26_domain_domain_Sha256Digest__self
T38_domain_domain_CatalogueDeclarationHash_new --> T38_domain_domain_CatalogueDeclarationHash__self
T38_domain_domain_CatalogueDeclarationHash_as_digest --> T26_domain_domain_Sha256Digest__self
T27_domain_domain_EdgeOwnership_Unique --o T28_domain_domain_TestObligation__self
T27_domain_domain_EdgeOwnership_Multiple --o T28_domain_domain_TestObligation__self
T37_domain_domain_ImplementationInputHash_new --o T26_domain_domain_Sha256Digest__self
T37_domain_domain_ImplementationInputHash_new --> T37_domain_domain_ImplementationInputHash__self
T37_domain_domain_ImplementationInputHash_as_digest --> T26_domain_domain_Sha256Digest__self
T21_domain_domain_LayerId_try_new --> T21_domain_domain_LayerId__self
T33_domain_domain_ObligationsDocument_new --o T21_domain_domain_TrackId__self
T33_domain_domain_ObligationsDocument_new --o T28_domain_domain_TestObligation__self
T33_domain_domain_ObligationsDocument_new --> T33_domain_domain_ObligationsDocument__self
T33_domain_domain_ObligationsDocument_track_id --> T21_domain_domain_TrackId__self
T33_domain_domain_ObligationsDocument_obligations --> T28_domain_domain_TestObligation__self
T33_domain_domain_ObligationsDocument_edge_ownership --> T27_domain_domain_EdgeOwnership__self
T33_domain_domain_ObligationsDocument_owners_of_edge --> T28_domain_domain_TestObligation__self
T33_domain_domain_ObligationsDocument_staleness_against --o T33_domain_domain_ObligationsDocument__self
T26_domain_domain_Sha256Digest_try_new --> T26_domain_domain_Sha256Digest__self
T26_domain_domain_Sha256Digest_try_new --> T31_domain_domain_Sha256DigestError__self
T26_domain_domain_Sha256Digest_from_content_hash --> T26_domain_domain_Sha256Digest__self
T34_domain_domain_TestBindingsDocument_new --o T21_domain_domain_TrackId__self
T34_domain_domain_TestBindingsDocument_new --> T34_domain_domain_TestBindingsDocument__self
T34_domain_domain_TestBindingsDocument_track_id --> T21_domain_domain_TrackId__self
T28_domain_domain_TestObligation_new --> T28_domain_domain_TestObligation__self
T33_domain_domain_TypeSignalsDocument_new --o T38_domain_domain_CatalogueDeclarationHash__self
T33_domain_domain_TypeSignalsDocument_new --o T37_domain_domain_ImplementationInputHash__self
T33_domain_domain_TypeSignalsDocument_new --> T33_domain_domain_TypeSignalsDocument__self
T33_domain_domain_TypeSignalsDocument_with_schema_version --o T38_domain_domain_TypeSignalsSchemaVersion__self
T33_domain_domain_TypeSignalsDocument_with_schema_version --o T38_domain_domain_CatalogueDeclarationHash__self
T33_domain_domain_TypeSignalsDocument_with_schema_version --o T37_domain_domain_ImplementationInputHash__self
T33_domain_domain_TypeSignalsDocument_with_schema_version --> T33_domain_domain_TypeSignalsDocument__self
T33_domain_domain_TypeSignalsDocument_schema_version --> T38_domain_domain_TypeSignalsSchemaVersion__self
T33_domain_domain_TypeSignalsDocument_declaration_hash --> T38_domain_domain_CatalogueDeclarationHash__self
T33_domain_domain_TypeSignalsDocument_implementation_input_hash --> T37_domain_domain_ImplementationInputHash__self
T35_domain_domain_TypeSignalsLoadResult_Current --o T33_domain_domain_TypeSignalsDocument__self
T35_domain_domain_TypeSignalsLoadResult_Stale --o T33_domain_domain_TypeSignalsDocument__self
T35_domain_domain_TypeSignalsLoadResult_Stale --o T38_domain_domain_CatalogueDeclarationHash__self
T35_domain_domain_TypeSignalsLoadResult_as_current --> T33_domain_domain_TypeSignalsDocument__self
T38_domain_domain_TypeSignalsSchemaVersion_try_new --> T38_domain_domain_TypeSignalsSchemaVersion__self
T38_domain_domain_TypeSignalsSchemaVersion_try_new --> T43_domain_domain_TypeSignalsSchemaVersionError__self
F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse --o T38_domain_domain_CatalogueDeclarationHash__self
F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse --o T37_domain_domain_ImplementationInputHash__self
F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse --o T38_domain_domain_CatalogueDeclarationHash__self
F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse --o T37_domain_domain_ImplementationInputHash__self
F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse --> T38_domain_domain_TypeSignalsReuseDecision__self
T41_usecase_usecase_CapabilityDispatchOutcome_Executed --o T28_usecase_usecase_ProviderName__self
T41_usecase_usecase_CapabilityDispatchOutcome_DelegateInHost --o T30_usecase_usecase_CapabilityName__self
T41_usecase_usecase_CapabilityDispatchOutcome_DelegateInHost --o T34_usecase_usecase_CapabilityFilePath__self
T41_usecase_usecase_CapabilityDispatchRequest__self --o|request| T37_usecase_usecase_CapabilityExecRequest__self
T41_usecase_usecase_CapabilityDispatchRequest__self --o|profile| T33_usecase_usecase_CapabilityProfile__self
T35_usecase_usecase_CapabilityExecError_ProfileResolution --o|capability| T30_usecase_usecase_CapabilityName__self
T35_usecase_usecase_CapabilityExecError_ProfileResolution --o|detail| T39_usecase_usecase_CapabilityFailureDetail__self
T35_usecase_usecase_CapabilityExecError_ExecutionModeRejected --o|capability| T30_usecase_usecase_CapabilityName__self
T35_usecase_usecase_CapabilityExecError_ModelMissing --o|capability| T30_usecase_usecase_CapabilityName__self
T35_usecase_usecase_CapabilityExecError_EffortMissing --o T30_usecase_usecase_CapabilityName__self
T35_usecase_usecase_CapabilityExecError_UnsupportedProvider --o|provider| T28_usecase_usecase_ProviderName__self
T35_usecase_usecase_CapabilityExecError_SourceValidation --o|path| T34_usecase_usecase_CapabilityFilePath__self
T35_usecase_usecase_CapabilityExecError_SourceValidation --o|detail| T39_usecase_usecase_CapabilityFailureDetail__self
T35_usecase_usecase_CapabilityExecError_AdapterPreflight --o|capability| T30_usecase_usecase_CapabilityName__self
T35_usecase_usecase_CapabilityExecError_AdapterPreflight --o|provider| T28_usecase_usecase_ProviderName__self
T35_usecase_usecase_CapabilityExecError_AdapterPreflight --o|detail| T39_usecase_usecase_CapabilityFailureDetail__self
T35_usecase_usecase_CapabilityExecError_DispatchFailed --o|provider| T28_usecase_usecase_ProviderName__self
T35_usecase_usecase_CapabilityExecError_DispatchFailed --o|detail| T39_usecase_usecase_CapabilityFailureDetail__self
T37_usecase_usecase_CapabilityExecRequest__self --o|capability| T30_usecase_usecase_CapabilityName__self
T37_usecase_usecase_CapabilityExecRequest__self --o|host| T28_usecase_usecase_ProviderName__self
T37_usecase_usecase_CapabilityExecRequest__self --o|briefing_file| T34_usecase_usecase_CapabilityFilePath__self
T37_usecase_usecase_CapabilityExecRequest__self --o|timeout| T30_usecase_usecase_TimeoutSeconds__self
T37_usecase_usecase_CapabilityExecRequest__self --o|resume| T39_usecase_usecase_CapabilityResumeRequest__self
T39_usecase_usecase_CapabilityFailureDetail_new --> T39_usecase_usecase_CapabilityFailureDetail__self
T34_usecase_usecase_CapabilityFilePath_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T34_usecase_usecase_CapabilityFilePath_try_new --> T34_usecase_usecase_CapabilityFilePath__self
T33_usecase_usecase_CapabilityProfile__self --o|provider| T28_usecase_usecase_ProviderName__self
T33_usecase_usecase_CapabilityProfile__self --o|model| T25_usecase_usecase_ModelName__self
T33_usecase_usecase_CapabilityProfile__self --o|effort| T31_usecase_usecase_ReasoningEffort__self
T39_usecase_usecase_CapabilityResumeRequest_Resume --o T33_usecase_usecase_TargetArtifactSet__self
T25_usecase_usecase_ModelName_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T25_usecase_usecase_ModelName_try_new --> T25_usecase_usecase_ModelName__self
T28_usecase_usecase_ProviderName_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T28_usecase_usecase_ProviderName_try_new --> T28_usecase_usecase_ProviderName__self
T34_usecase_usecase_TargetArtifactPath_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T34_usecase_usecase_TargetArtifactPath_try_new --> T34_usecase_usecase_TargetArtifactPath__self
T33_usecase_usecase_TargetArtifactSet_try_new --o T34_usecase_usecase_TargetArtifactPath__self
T33_usecase_usecase_TargetArtifactSet_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T33_usecase_usecase_TargetArtifactSet_try_new --> T33_usecase_usecase_TargetArtifactSet__self
T33_usecase_usecase_TargetArtifactSet_as_slice --> T34_usecase_usecase_TargetArtifactPath__self
T30_usecase_usecase_TimeoutSeconds_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T30_usecase_usecase_TimeoutSeconds_try_new --> T30_usecase_usecase_TimeoutSeconds__self
R37_usecase_usecase_CapabilityProfilePort_resolve --o T30_usecase_usecase_CapabilityName__self
R37_usecase_usecase_CapabilityProfilePort_resolve --> T35_usecase_usecase_CapabilityExecError__self
R37_usecase_usecase_CapabilityProfilePort_resolve --> T33_usecase_usecase_CapabilityProfile__self
R38_usecase_usecase_CapabilityProviderPort_provider --> T28_usecase_usecase_ProviderName__self
R38_usecase_usecase_CapabilityProviderPort_dispatch --o T41_usecase_usecase_CapabilityDispatchRequest__self
R38_usecase_usecase_CapabilityProviderPort_dispatch --> T41_usecase_usecase_CapabilityDispatchOutcome__self
R38_usecase_usecase_CapabilityProviderPort_dispatch --> T35_usecase_usecase_CapabilityExecError__self
T30_usecase_usecase_CapabilityName_try_new --> T30_usecase_usecase_CapabilityName__self
T30_usecase_usecase_DiagnosticText_new --> T30_usecase_usecase_DiagnosticText__self
T41_usecase_usecase_ProviderSessionCacheEntry_new --o T33_usecase_usecase_ProviderSessionId__self
T41_usecase_usecase_ProviderSessionCacheEntry_new --o T28_usecase_usecase_ProviderName__self
T41_usecase_usecase_ProviderSessionCacheEntry_new --o T25_usecase_usecase_ModelName__self
T41_usecase_usecase_ProviderSessionCacheEntry_new --o T31_usecase_usecase_ReasoningEffort__self
T41_usecase_usecase_ProviderSessionCacheEntry_new --> T41_usecase_usecase_ProviderSessionCacheEntry__self
T41_usecase_usecase_ProviderSessionCacheEntry_session_id --> T33_usecase_usecase_ProviderSessionId__self
T41_usecase_usecase_ProviderSessionCacheEntry_provider --> T28_usecase_usecase_ProviderName__self
T41_usecase_usecase_ProviderSessionCacheEntry_model --> T25_usecase_usecase_ModelName__self
T41_usecase_usecase_ProviderSessionCacheEntry_effort --> T31_usecase_usecase_ReasoningEffort__self
T41_usecase_usecase_ProviderSessionCacheError_StorageUnavailable --o T30_usecase_usecase_DiagnosticText__self
T41_usecase_usecase_ProviderSessionCacheError_EntryInvalid --o T30_usecase_usecase_DiagnosticText__self
T41_usecase_usecase_ProviderSessionCacheError_IdentityBoundaryViolation --o T30_usecase_usecase_DiagnosticText__self
T39_usecase_usecase_ProviderSessionCacheKey_Review --o|track_id| T21_domain_domain_TrackId__self
T39_usecase_usecase_ProviderSessionCacheKey_Review --o|scope| T23_domain_domain_ScopeName__self
T39_usecase_usecase_ProviderSessionCacheKey_Review --o|round_type| T23_domain_domain_RoundType__self
T39_usecase_usecase_ProviderSessionCacheKey_TrackCapability --o|track_id| T21_domain_domain_TrackId__self
T39_usecase_usecase_ProviderSessionCacheKey_TrackCapability --o|capability| T30_usecase_usecase_CapabilityName__self
T39_usecase_usecase_ProviderSessionCacheKey_WorkspaceCapability --o|capability| T30_usecase_usecase_CapabilityName__self
T39_usecase_usecase_ProviderSessionCacheKey_WorkspaceCapability --o|target_artifacts| T33_usecase_usecase_TargetArtifactSet__self
T33_usecase_usecase_ProviderSessionId_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T33_usecase_usecase_ProviderSessionId_try_new --> T33_usecase_usecase_ProviderSessionId__self
T30_usecase_usecase_ReviewerPrompt_try_new --> T46_usecase_usecase_CapabilityInputValidationError__self
T30_usecase_usecase_ReviewerPrompt_try_new --> T30_usecase_usecase_ReviewerPrompt__self
R40_usecase_usecase_ProviderSessionCachePort_load --o T39_usecase_usecase_ProviderSessionCacheKey__self
R40_usecase_usecase_ProviderSessionCachePort_load --> T41_usecase_usecase_ProviderSessionCacheEntry__self
R40_usecase_usecase_ProviderSessionCachePort_load --> T41_usecase_usecase_ProviderSessionCacheError__self
R40_usecase_usecase_ProviderSessionCachePort_save --o T39_usecase_usecase_ProviderSessionCacheKey__self
R40_usecase_usecase_ProviderSessionCachePort_save --o T41_usecase_usecase_ProviderSessionCacheEntry__self
R40_usecase_usecase_ProviderSessionCachePort_save --> T41_usecase_usecase_ProviderSessionCacheError__self
R40_usecase_usecase_ProviderSessionCachePort_remove --o T39_usecase_usecase_ProviderSessionCacheKey__self
R40_usecase_usecase_ProviderSessionCachePort_remove --> T41_usecase_usecase_ProviderSessionCacheError__self
T32_usecase_usecase_TypeSignalsError_BranchTrackMismatch --o|branch| T25_domain_domain_TrackBranch__self
T32_usecase_usecase_TypeSignalsError_BranchTrackMismatch --o|track_id| T21_domain_domain_TrackId__self
T32_usecase_usecase_TypeSignalsError_LayerBindingsLoad --o|reason| T30_usecase_usecase_DiagnosticText__self
T32_usecase_usecase_TypeSignalsError_EvaluationFailed --o|layer_id| T21_domain_domain_LayerId__self
T32_usecase_usecase_TypeSignalsError_EvaluationFailed --o|reason| T30_usecase_usecase_DiagnosticText__self
T32_usecase_usecase_TypeSignalsError_InconsistentRequest --o|reason| T30_usecase_usecase_DiagnosticText__self
T37_usecase_usecase_TypeSignalsInteractor_new --> T37_usecase_usecase_TypeSignalsInteractor__self
T34_usecase_usecase_TypeSignalsRequest__self --o|track_id| T21_domain_domain_TrackId__self
T34_usecase_usecase_TypeSignalsRequest__self --o|branch| T25_domain_domain_TrackBranch__self
T34_usecase_usecase_TypeSignalsRequest__self --o|layer| T21_domain_domain_LayerId__self
R39_usecase_usecase_TypeSignalsExecutorPort_evaluate_layer --o T21_domain_domain_TrackId__self
R39_usecase_usecase_TypeSignalsExecutorPort_evaluate_layer --> T41_usecase_usecase_TypeSignalsExecutionError__self
R34_usecase_usecase_TypeSignalsService_run --o T34_usecase_usecase_TypeSignalsRequest__self
R34_usecase_usecase_TypeSignalsService_run --> T32_usecase_usecase_TypeSignalsError__self
T37_usecase_usecase_TypeSignalsInteractor__self -.impl.-> R34_usecase_usecase_TypeSignalsService__self
T43_infrastructure_infrastructure_AgentProfiles_load --> T48_infrastructure_infrastructure_AgentProfilesError__self
T43_infrastructure_infrastructure_AgentProfiles_load --> T43_infrastructure_infrastructure_AgentProfiles__self
T43_infrastructure_infrastructure_AgentProfiles_resolve_capability --o T30_usecase_usecase_CapabilityName__self
T43_infrastructure_infrastructure_AgentProfiles_resolve_capability --> T49_infrastructure_infrastructure_CapabilityConfigDto__self
T43_infrastructure_infrastructure_AgentProfiles_resolve_execution --o T30_usecase_usecase_CapabilityName__self
T43_infrastructure_infrastructure_AgentProfiles_resolve_execution --o T39_infrastructure_infrastructure_RoundType__self
T43_infrastructure_infrastructure_AgentProfiles_resolve_execution --> T48_infrastructure_infrastructure_AgentProfilesError__self
T43_infrastructure_infrastructure_AgentProfiles_resolve_execution --> T47_infrastructure_infrastructure_ResolvedExecution__self
T43_infrastructure_infrastructure_AgentProfiles_provider_label --o T28_usecase_usecase_ProviderName__self
T43_infrastructure_infrastructure_AgentProfiles_resolve_prompt_template_path --o T30_usecase_usecase_CapabilityName__self
T48_infrastructure_infrastructure_AgentProfilesError_CapabilityNotFound --o T30_usecase_usecase_CapabilityName__self
T48_infrastructure_infrastructure_AgentProfilesError_ModelMissing --o T30_usecase_usecase_CapabilityName__self
T48_infrastructure_infrastructure_AgentProfilesError_EffortMissing --o T30_usecase_usecase_CapabilityName__self
T48_infrastructure_infrastructure_AgentProfilesError_EffortMissing --o T39_infrastructure_infrastructure_RoundType__self
T48_infrastructure_infrastructure_AgentProfilesError_UnsupportedEffort --o T28_usecase_usecase_ProviderName__self
T48_infrastructure_infrastructure_AgentProfilesError_UnsupportedEffort --o T31_usecase_usecase_ReasoningEffort__self
T49_infrastructure_infrastructure_CapabilityConfigDto_provider --> T45_infrastructure_infrastructure_ProviderNameDto__self
T49_infrastructure_infrastructure_CapabilityConfigDto_model --> T42_infrastructure_infrastructure_ModelNameDto__self
T49_infrastructure_infrastructure_CapabilityConfigDto_fast_provider --> T45_infrastructure_infrastructure_ProviderNameDto__self
T49_infrastructure_infrastructure_CapabilityConfigDto_fast_model --> T42_infrastructure_infrastructure_ModelNameDto__self
T49_infrastructure_infrastructure_CapabilityConfigDto_effort --> T48_infrastructure_infrastructure_ReasoningEffortDto__self
T49_infrastructure_infrastructure_CapabilityConfigDto_fast_effort --> T48_infrastructure_infrastructure_ReasoningEffortDto__self
T49_infrastructure_infrastructure_CapabilityConfigDto_execution_mode --> T46_infrastructure_infrastructure_ExecutionModeDto__self
T42_infrastructure_infrastructure_ModelNameDto_try_new --> T42_infrastructure_infrastructure_ModelNameDto__self
T45_infrastructure_infrastructure_ProviderNameDto_try_new --> T45_infrastructure_infrastructure_ProviderNameDto__self
T48_infrastructure_infrastructure_ReasoningEffortDto_into_domain --> T31_usecase_usecase_ReasoningEffort__self
T47_infrastructure_infrastructure_ResolvedExecution_ProviderCli --o|provider| T28_usecase_usecase_ProviderName__self
T47_infrastructure_infrastructure_ResolvedExecution_ProviderCli --o|model| T25_usecase_usecase_ModelName__self
T47_infrastructure_infrastructure_ResolvedExecution_ProviderCli --o|effort| T31_usecase_usecase_ReasoningEffort__self
T47_infrastructure_infrastructure_ResolvedExecution_HostedService --o|provider| T28_usecase_usecase_ProviderName__self
T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter_new --> T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter__self
T53_infrastructure_infrastructure_ClaudeCapabilityAdapter_new --> T53_infrastructure_infrastructure_ClaudeCapabilityAdapter__self
T52_infrastructure_infrastructure_CodexCapabilityAdapter_new --> T52_infrastructure_infrastructure_CodexCapabilityAdapter__self
T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter_new --> T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter__self
F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_claude_ref_verifier_args --o T31_usecase_usecase_ReasoningEffort__self
F103_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_codex_ref_verifier_args --o T31_usecase_usecase_ReasoningEffort__self
F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_gemini_ref_verifier_args --o T31_usecase_usecase_ReasoningEffort__self
T44_infrastructure_infrastructure_ClaudeReviewer_new --o T21_domain_domain_TrackId__self
T44_infrastructure_infrastructure_ClaudeReviewer_new --o T23_domain_domain_ScopeName__self
T44_infrastructure_infrastructure_ClaudeReviewer_new --o T23_domain_domain_RoundType__self
T44_infrastructure_infrastructure_ClaudeReviewer_new --o T25_usecase_usecase_ModelName__self
T44_infrastructure_infrastructure_ClaudeReviewer_new --o T31_usecase_usecase_ReasoningEffort__self
T44_infrastructure_infrastructure_ClaudeReviewer_new --o T30_usecase_usecase_ReviewerPrompt__self
T44_infrastructure_infrastructure_ClaudeReviewer_new --> T44_infrastructure_infrastructure_ClaudeReviewer__self
T43_infrastructure_infrastructure_CodexReviewer_new --o T21_domain_domain_TrackId__self
T43_infrastructure_infrastructure_CodexReviewer_new --o T23_domain_domain_ScopeName__self
T43_infrastructure_infrastructure_CodexReviewer_new --o T23_domain_domain_RoundType__self
T43_infrastructure_infrastructure_CodexReviewer_new --o T25_usecase_usecase_ModelName__self
T43_infrastructure_infrastructure_CodexReviewer_new --o T31_usecase_usecase_ReasoningEffort__self
T43_infrastructure_infrastructure_CodexReviewer_new --o T30_usecase_usecase_ReviewerPrompt__self
T43_infrastructure_infrastructure_CodexReviewer_new --> T43_infrastructure_infrastructure_CodexReviewer__self
T51_infrastructure_infrastructure_RustdocSchemaExporter_new --> T51_infrastructure_infrastructure_RustdocSchemaExporter__self
T51_infrastructure_infrastructure_TypeSignalsCodecError_UnsupportedSchemaVersion --o T38_domain_domain_TypeSignalsSchemaVersion__self
T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidSchemaVersion --o T43_domain_domain_TypeSignalsSchemaVersionError__self
T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidDigest --o|source| T31_domain_domain_Sha256DigestError__self
T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter_new --> T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter__self
F88_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__declaration_hash --> T38_domain_domain_CatalogueDeclarationHash__self
F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__decode --> T51_infrastructure_infrastructure_TypeSignalsCodecError__self
F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__decode --> T33_domain_domain_TypeSignalsDocument__self
F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__encode --o T33_domain_domain_TypeSignalsDocument__self
F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__encode --> T51_infrastructure_infrastructure_TypeSignalsCodecError__self
F106_infrastructure_infrastructure_infrastructure__tddd__type_signals_evaluator__execute_type_signals_for_layer --o T21_domain_domain_TrackId__self
F106_infrastructure_infrastructure_infrastructure__tddd__type_signals_evaluator__execute_type_signals_for_layer --> T50_infrastructure_infrastructure_EvaluateSignalsError__self
T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_StaleHash --o|declared| T38_domain_domain_CatalogueDeclarationHash__self
T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_StaleHash --o|actual| T38_domain_domain_CatalogueDeclarationHash__self
F112_infrastructure_infrastructure_infrastructure__verify__catalogue_spec_signals__compute_catalogue_declaration_hash --> T38_domain_domain_CatalogueDeclarationHash__self
T51_infrastructure_infrastructure_RustdocSchemaExporter__self -.impl.-> R28_domain_domain_SchemaExporter__self
T51_infrastructure_infrastructure_RustdocSchemaExporter__self -.impl.-> R34_usecase_usecase_SchemaExporterPort__self
T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter__self -.impl.-> R39_usecase_usecase_TypeSignalsExecutorPort__self
T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter__self -.impl.-> R37_usecase_usecase_CapabilityProfilePort__self
T52_infrastructure_infrastructure_CodexCapabilityAdapter__self -.impl.-> R38_usecase_usecase_CapabilityProviderPort__self
T53_infrastructure_infrastructure_ClaudeCapabilityAdapter__self -.impl.-> R38_usecase_usecase_CapabilityProviderPort__self
T43_infrastructure_infrastructure_CodexReviewer__self -.impl.-> R24_usecase_usecase_Reviewer__self
T44_infrastructure_infrastructure_ClaudeReviewer__self -.impl.-> R24_usecase_usecase_Reviewer__self
T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter__self -.impl.-> R40_usecase_usecase_ProviderSessionCachePort__self
T38_cli_driver_cli_driver_CapabilityDriver_new --> T38_cli_driver_cli_driver_CapabilityDriver__self
T38_cli_driver_cli_driver_CapabilityDriver_handle --o T47_cli_driver_cli_driver_CapabilityExecDriverInput__self
T47_cli_driver_cli_driver_CapabilityExecDriverInput__self --o|resume| T41_cli_driver_cli_driver_CapabilityResumeArg__self
T41_cli_driver_cli_driver_CapabilityResumeArg_Resume --o T43_cli_driver_cli_driver_TargetArtifactPathArg__self
T41_cli_driver_cli_driver_CapabilityResumeArg_into_domain --> T46_usecase_usecase_CapabilityInputValidationError__self
T41_cli_driver_cli_driver_CapabilityResumeArg_into_domain --> T39_usecase_usecase_CapabilityResumeRequest__self
T43_cli_driver_cli_driver_TargetArtifactPathArg_into_domain --> T34_usecase_usecase_TargetArtifactPath__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_new --> T57_cli_composition_cli_composition_CapabilityCompositionRoot__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover --> T57_cli_composition_cli_composition_CapabilityCompositionRoot__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver --> T38_cli_driver_cli_driver_CapabilityDriver__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local_resolve --o T21_domain_domain_TrackId__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local_resolve --o T23_domain_domain_ScopeName__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local_resolve --o T23_domain_domain_RoundType__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local_resolve --o T25_usecase_usecase_ModelName__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_check_approved --o T21_domain_domain_TrackId__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_classify --o T21_domain_domain_TrackId__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_files --o T23_domain_domain_ScopeName__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_files --o T21_domain_domain_TrackId__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_validate_scope --o T23_domain_domain_ScopeName__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_validate_scope --o T21_domain_domain_TrackId__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_get_briefing --o T23_domain_domain_ScopeName__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_get_briefing --o T21_domain_domain_TrackId__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_persist_commit_hash --o T21_domain_domain_TrackId__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_new --> T53_cli_composition_cli_composition_ReviewCompositionRoot__self
T53_cli_composition_cli_composition_SignalCompositionRoot_new --> T53_cli_composition_cli_composition_SignalCompositionRoot__self
T25_cli_cli_CapabilityCommand_Exec --o T26_cli_cli_CapabilityExecArgs__self
T26_cli_cli_CapabilityExecArgs__self --o|resume| T41_cli_driver_cli_driver_CapabilityResumeArg__self
F42_cli_cli_cli__commands__capability__execute --o T25_cli_cli_CapabilityCommand__self
F38_cli_cli_cli__commands__review__execute --o T21_cli_cli_ReviewCommand__self
class T25_domain_domain_TrackBranch_try_new method_node
class T25_domain_domain_TrackBranch__self value_object
class T21_domain_domain_TrackId_try_new method_node
class T21_domain_domain_TrackId__self value_object
class T23_domain_domain_RoundType_Fast variant_node
class T23_domain_domain_RoundType_Final variant_node
class T23_domain_domain_RoundType__self value_object
class T23_domain_domain_ScopeName_Main variant_node
class T23_domain_domain_ScopeName_Other variant_node
class T23_domain_domain_ScopeName__self value_object
class R28_domain_domain_SchemaExporter_export method_node
class R28_domain_domain_SchemaExporter__self secondary_port
class T38_domain_domain_CatalogueDeclarationHash_new method_node
class T38_domain_domain_CatalogueDeclarationHash_as_digest method_node
class T38_domain_domain_CatalogueDeclarationHash__self value_object
class T27_domain_domain_EdgeOwnership_None variant_node
class T27_domain_domain_EdgeOwnership_Unique variant_node
class T27_domain_domain_EdgeOwnership_Multiple variant_node
class T27_domain_domain_EdgeOwnership__self value_object
class T37_domain_domain_ImplementationInputHash_new method_node
class T37_domain_domain_ImplementationInputHash_as_digest method_node
class T37_domain_domain_ImplementationInputHash__self value_object
class T21_domain_domain_LayerId_try_new method_node
class T21_domain_domain_LayerId__self value_object
class T33_domain_domain_ObligationsDocument_new method_node
class T33_domain_domain_ObligationsDocument_track_id method_node
class T33_domain_domain_ObligationsDocument_obligations method_node
class T33_domain_domain_ObligationsDocument_edge_ownership method_node
class T33_domain_domain_ObligationsDocument_owners_of_edge method_node
class T33_domain_domain_ObligationsDocument_staleness_against method_node
class T33_domain_domain_ObligationsDocument__self domain_service
class T26_domain_domain_Sha256Digest_try_new method_node
class T26_domain_domain_Sha256Digest_from_content_hash method_node
class T26_domain_domain_Sha256Digest_as_str method_node
class T26_domain_domain_Sha256Digest__self value_object
class T31_domain_domain_Sha256DigestError_InvalidLength variant_node
class T31_domain_domain_Sha256DigestError_InvalidHex variant_node
class T31_domain_domain_Sha256DigestError__self error_type
class T34_domain_domain_TestBindingsDocument_new method_node
class T34_domain_domain_TestBindingsDocument_track_id method_node
class T34_domain_domain_TestBindingsDocument_records method_node
class T34_domain_domain_TestBindingsDocument_waived_edge_ids method_node
class T34_domain_domain_TestBindingsDocument_is_edge_waived method_node
class T34_domain_domain_TestBindingsDocument__self domain_service
class T28_domain_domain_TestObligation_new method_node
class T28_domain_domain_TestObligation_id method_node
class T28_domain_domain_TestObligation_target_entry method_node
class T28_domain_domain_TestObligation_target_role method_node
class T28_domain_domain_TestObligation_brief method_node
class T28_domain_domain_TestObligation_declaration_hash method_node
class T28_domain_domain_TestObligation_spec_refs method_node
class T28_domain_domain_TestObligation_owns_edge method_node
class T28_domain_domain_TestObligation__self domain_service
class T33_domain_domain_TypeSignalsDocument_new method_node
class T33_domain_domain_TypeSignalsDocument_with_schema_version method_node
class T33_domain_domain_TypeSignalsDocument_schema_version method_node
class T33_domain_domain_TypeSignalsDocument_generated_at method_node
class T33_domain_domain_TypeSignalsDocument_declaration_hash method_node
class T33_domain_domain_TypeSignalsDocument_implementation_input_hash method_node
class T33_domain_domain_TypeSignalsDocument_signals method_node
class T33_domain_domain_TypeSignalsDocument__self value_object
class T35_domain_domain_TypeSignalsLoadResult_Current variant_node
class T35_domain_domain_TypeSignalsLoadResult_Stale variant_node
class T35_domain_domain_TypeSignalsLoadResult_Missing variant_node
class T35_domain_domain_TypeSignalsLoadResult_as_current method_node
class T35_domain_domain_TypeSignalsLoadResult_is_current method_node
class T35_domain_domain_TypeSignalsLoadResult_is_stale method_node
class T35_domain_domain_TypeSignalsLoadResult_is_missing method_node
class T35_domain_domain_TypeSignalsLoadResult__self value_object
class T38_domain_domain_TypeSignalsReuseDecision_SkipEvaluation variant_node
class T38_domain_domain_TypeSignalsReuseDecision_ReevaluateWithoutExtraction variant_node
class T38_domain_domain_TypeSignalsReuseDecision_ReextractAndEvaluate variant_node
class T38_domain_domain_TypeSignalsReuseDecision__self value_object
class T38_domain_domain_TypeSignalsSchemaVersion_try_new method_node
class T38_domain_domain_TypeSignalsSchemaVersion_value method_node
class T38_domain_domain_TypeSignalsSchemaVersion__self value_object
class T43_domain_domain_TypeSignalsSchemaVersionError_Zero variant_node
class T43_domain_domain_TypeSignalsSchemaVersionError__self error_type
class F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse free_function
class F71_domain_domain_domain__tddd__type_signals_doc__decide_type_signals_reuse function_node
class T41_usecase_usecase_CapabilityDispatchOutcome_Executed variant_node
class T41_usecase_usecase_CapabilityDispatchOutcome_DelegateInHost variant_node
class T41_usecase_usecase_CapabilityDispatchOutcome__self dto
class T41_usecase_usecase_CapabilityDispatchRequest__self dto
class T35_usecase_usecase_CapabilityExecError_ProfileResolution variant_node
class T35_usecase_usecase_CapabilityExecError_ExecutionModeRejected variant_node
class T35_usecase_usecase_CapabilityExecError_ModelMissing variant_node
class T35_usecase_usecase_CapabilityExecError_EffortMissing variant_node
class T35_usecase_usecase_CapabilityExecError_UnsupportedProvider variant_node
class T35_usecase_usecase_CapabilityExecError_SourceValidation variant_node
class T35_usecase_usecase_CapabilityExecError_AdapterPreflight variant_node
class T35_usecase_usecase_CapabilityExecError_DispatchFailed variant_node
class T35_usecase_usecase_CapabilityExecError__self error_type
class T37_usecase_usecase_CapabilityExecRequest__self command
class T39_usecase_usecase_CapabilityFailureDetail_new method_node
class T39_usecase_usecase_CapabilityFailureDetail_as_str method_node
class T39_usecase_usecase_CapabilityFailureDetail__self value_object
class T34_usecase_usecase_CapabilityFilePath_try_new method_node
class T34_usecase_usecase_CapabilityFilePath_as_path method_node
class T34_usecase_usecase_CapabilityFilePath__self value_object
class T46_usecase_usecase_CapabilityInputValidationError_EmptyProviderName variant_node
class T46_usecase_usecase_CapabilityInputValidationError_EmptyModelName variant_node
class T46_usecase_usecase_CapabilityInputValidationError_EmptyFilePath variant_node
class T46_usecase_usecase_CapabilityInputValidationError_InvalidFilePath variant_node
class T46_usecase_usecase_CapabilityInputValidationError_EmptyContent variant_node
class T46_usecase_usecase_CapabilityInputValidationError_ZeroTimeoutSeconds variant_node
class T46_usecase_usecase_CapabilityInputValidationError_EmptyTargetArtifactSet variant_node
class T46_usecase_usecase_CapabilityInputValidationError__self error_type
class T33_usecase_usecase_CapabilityProfile__self dto
class T39_usecase_usecase_CapabilityResumeRequest_Fresh variant_node
class T39_usecase_usecase_CapabilityResumeRequest_ResumeWithoutTarget variant_node
class T39_usecase_usecase_CapabilityResumeRequest_Resume variant_node
class T39_usecase_usecase_CapabilityResumeRequest__self value_object
class T25_usecase_usecase_ModelName_try_new method_node
class T25_usecase_usecase_ModelName_as_str method_node
class T25_usecase_usecase_ModelName__self value_object
class T28_usecase_usecase_ProviderName_try_new method_node
class T28_usecase_usecase_ProviderName_as_str method_node
class T28_usecase_usecase_ProviderName__self value_object
class T31_usecase_usecase_ReasoningEffort_Low variant_node
class T31_usecase_usecase_ReasoningEffort_Medium variant_node
class T31_usecase_usecase_ReasoningEffort_High variant_node
class T31_usecase_usecase_ReasoningEffort_XHigh variant_node
class T31_usecase_usecase_ReasoningEffort_Max variant_node
class T31_usecase_usecase_ReasoningEffort__self value_object
class T34_usecase_usecase_TargetArtifactPath_try_new method_node
class T34_usecase_usecase_TargetArtifactPath_as_path method_node
class T34_usecase_usecase_TargetArtifactPath__self value_object
class T33_usecase_usecase_TargetArtifactSet_try_new method_node
class T33_usecase_usecase_TargetArtifactSet_as_slice method_node
class T33_usecase_usecase_TargetArtifactSet__self value_object
class T30_usecase_usecase_TimeoutSeconds_try_new method_node
class T30_usecase_usecase_TimeoutSeconds_as_secs method_node
class T30_usecase_usecase_TimeoutSeconds__self value_object
class R37_usecase_usecase_CapabilityProfilePort_resolve method_node
class R37_usecase_usecase_CapabilityProfilePort__self secondary_port
class R38_usecase_usecase_CapabilityProviderPort_provider method_node
class R38_usecase_usecase_CapabilityProviderPort_dispatch method_node
class R38_usecase_usecase_CapabilityProviderPort__self secondary_port
class T30_usecase_usecase_CapabilityName_try_new method_node
class T30_usecase_usecase_CapabilityName_as_str method_node
class T30_usecase_usecase_CapabilityName__self value_object
class R34_usecase_usecase_SchemaExporterPort_export_as_json method_node
class R34_usecase_usecase_SchemaExporterPort__self secondary_port
class T30_usecase_usecase_DiagnosticText_new method_node
class T30_usecase_usecase_DiagnosticText_as_str method_node
class T30_usecase_usecase_DiagnosticText__self value_object
class T41_usecase_usecase_ProviderSessionCacheEntry_new method_node
class T41_usecase_usecase_ProviderSessionCacheEntry_session_id method_node
class T41_usecase_usecase_ProviderSessionCacheEntry_provider method_node
class T41_usecase_usecase_ProviderSessionCacheEntry_model method_node
class T41_usecase_usecase_ProviderSessionCacheEntry_effort method_node
class T41_usecase_usecase_ProviderSessionCacheEntry__self value_object
class T41_usecase_usecase_ProviderSessionCacheError_StorageUnavailable variant_node
class T41_usecase_usecase_ProviderSessionCacheError_EntryInvalid variant_node
class T41_usecase_usecase_ProviderSessionCacheError_IdentityBoundaryViolation variant_node
class T41_usecase_usecase_ProviderSessionCacheError__self error_type
class T39_usecase_usecase_ProviderSessionCacheKey_Review variant_node
class T39_usecase_usecase_ProviderSessionCacheKey_TrackCapability variant_node
class T39_usecase_usecase_ProviderSessionCacheKey_WorkspaceCapability variant_node
class T39_usecase_usecase_ProviderSessionCacheKey__self value_object
class T33_usecase_usecase_ProviderSessionId_try_new method_node
class T33_usecase_usecase_ProviderSessionId_as_str method_node
class T33_usecase_usecase_ProviderSessionId__self value_object
class T30_usecase_usecase_ReviewerPrompt_try_new method_node
class T30_usecase_usecase_ReviewerPrompt_as_str method_node
class T30_usecase_usecase_ReviewerPrompt__self value_object
class R40_usecase_usecase_ProviderSessionCachePort_load method_node
class R40_usecase_usecase_ProviderSessionCachePort_save method_node
class R40_usecase_usecase_ProviderSessionCachePort_remove method_node
class R40_usecase_usecase_ProviderSessionCachePort__self secondary_port
class R24_usecase_usecase_Reviewer_review method_node
class R24_usecase_usecase_Reviewer_fast_review method_node
class R24_usecase_usecase_Reviewer__self secondary_port
class T32_usecase_usecase_TypeSignalsError_BranchTrackMismatch variant_node
class T32_usecase_usecase_TypeSignalsError_LayerBindingsLoad variant_node
class T32_usecase_usecase_TypeSignalsError_NoLayers variant_node
class T32_usecase_usecase_TypeSignalsError_EvaluationFailed variant_node
class T32_usecase_usecase_TypeSignalsError_InconsistentRequest variant_node
class T32_usecase_usecase_TypeSignalsError__self error_type
class T41_usecase_usecase_TypeSignalsExecutionError__self error_type
class T37_usecase_usecase_TypeSignalsInteractor_new method_node
class T37_usecase_usecase_TypeSignalsInteractor__self interactor
class T34_usecase_usecase_TypeSignalsRequest__self command
class R39_usecase_usecase_TypeSignalsExecutorPort_evaluate_layer method_node
class R39_usecase_usecase_TypeSignalsExecutorPort__self secondary_port
class R34_usecase_usecase_TypeSignalsService_run method_node
class R34_usecase_usecase_TypeSignalsService__self app_service
class T43_infrastructure_infrastructure_AgentProfiles_load method_node
class T43_infrastructure_infrastructure_AgentProfiles_resolve_capability method_node
class T43_infrastructure_infrastructure_AgentProfiles_resolve_execution method_node
class T43_infrastructure_infrastructure_AgentProfiles_provider_label method_node
class T43_infrastructure_infrastructure_AgentProfiles_resolve_prompt_template_path method_node
class T43_infrastructure_infrastructure_AgentProfiles__self secondary_adapter
class T48_infrastructure_infrastructure_AgentProfilesError_Io variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_Symlink variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_PathOutsideTrustedRoot variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_Parse variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_UnsupportedSchemaVersion variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_InvalidCapability variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_CapabilityNotFound variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_ModelMissing variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_EffortMissing variant_node
class T48_infrastructure_infrastructure_AgentProfilesError_UnsupportedEffort variant_node
class T48_infrastructure_infrastructure_AgentProfilesError__self error_type
class T49_infrastructure_infrastructure_CapabilityConfigDto_provider method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto_model method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto_fast_provider method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto_fast_model method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto_prompt_template_path method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto_effort method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto_fast_effort method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto_execution_mode method_node
class T49_infrastructure_infrastructure_CapabilityConfigDto__self dto
class T46_infrastructure_infrastructure_ExecutionModeDto_OrchestratorOutput variant_node
class T46_infrastructure_infrastructure_ExecutionModeDto_TypedPipeline variant_node
class T46_infrastructure_infrastructure_ExecutionModeDto_into_domain method_node
class T46_infrastructure_infrastructure_ExecutionModeDto__self dto
class T42_infrastructure_infrastructure_ModelNameDto_try_new method_node
class T42_infrastructure_infrastructure_ModelNameDto_into_domain method_node
class T42_infrastructure_infrastructure_ModelNameDto__self dto
class T45_infrastructure_infrastructure_ProviderNameDto_try_new method_node
class T45_infrastructure_infrastructure_ProviderNameDto_into_domain method_node
class T45_infrastructure_infrastructure_ProviderNameDto__self dto
class T48_infrastructure_infrastructure_ReasoningEffortDto_Low variant_node
class T48_infrastructure_infrastructure_ReasoningEffortDto_Medium variant_node
class T48_infrastructure_infrastructure_ReasoningEffortDto_High variant_node
class T48_infrastructure_infrastructure_ReasoningEffortDto_XHigh variant_node
class T48_infrastructure_infrastructure_ReasoningEffortDto_Max variant_node
class T48_infrastructure_infrastructure_ReasoningEffortDto_into_domain method_node
class T48_infrastructure_infrastructure_ReasoningEffortDto__self dto
class T47_infrastructure_infrastructure_ResolvedExecution_ProviderCli variant_node
class T47_infrastructure_infrastructure_ResolvedExecution_HostedService variant_node
class T47_infrastructure_infrastructure_ResolvedExecution__self dto
class T39_infrastructure_infrastructure_RoundType_Final variant_node
class T39_infrastructure_infrastructure_RoundType_Fast variant_node
class T39_infrastructure_infrastructure_RoundType__self value_object
class T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter_new method_node
class T60_infrastructure_infrastructure_AgentProfilesCapabilityAdapter__self secondary_adapter
class T53_infrastructure_infrastructure_ClaudeCapabilityAdapter_new method_node
class T53_infrastructure_infrastructure_ClaudeCapabilityAdapter__self secondary_adapter
class T52_infrastructure_infrastructure_CodexCapabilityAdapter_new method_node
class T52_infrastructure_infrastructure_CodexCapabilityAdapter__self secondary_adapter
class T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter_new method_node
class T59_infrastructure_infrastructure_FsProviderSessionCacheAdapter__self secondary_adapter
class F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_claude_ref_verifier_args free_function
class F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_claude_ref_verifier_args function_node
class F103_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_codex_ref_verifier_args free_function
class F103_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_codex_ref_verifier_args function_node
class F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_gemini_ref_verifier_args free_function
class F104_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_gemini_ref_verifier_args function_node
class T44_infrastructure_infrastructure_ClaudeReviewer_new method_node
class T44_infrastructure_infrastructure_ClaudeReviewer__self secondary_adapter
class T43_infrastructure_infrastructure_CodexReviewer_new method_node
class T43_infrastructure_infrastructure_CodexReviewer__self secondary_adapter
class T51_infrastructure_infrastructure_RustdocSchemaExporter_new method_node
class T51_infrastructure_infrastructure_RustdocSchemaExporter_export_rustdoc_json_path method_node
class T51_infrastructure_infrastructure_RustdocSchemaExporter_existing_rustdoc_json_path method_node
class T51_infrastructure_infrastructure_RustdocSchemaExporter__self secondary_adapter
class T50_infrastructure_infrastructure_EvaluateSignalsError__self error_type
class T51_infrastructure_infrastructure_TypeSignalsCodecError_Json variant_node
class T51_infrastructure_infrastructure_TypeSignalsCodecError_UnsupportedSchemaVersion variant_node
class T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidSchemaVersion variant_node
class T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidTimestamp variant_node
class T51_infrastructure_infrastructure_TypeSignalsCodecError_InvalidDigest variant_node
class T51_infrastructure_infrastructure_TypeSignalsCodecError__self error_type
class T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter_new method_node
class T56_infrastructure_infrastructure_TypeSignalsExecutorAdapter__self secondary_adapter
class F88_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__declaration_hash free_function
class F88_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__declaration_hash function_node
class F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__decode free_function
class F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__decode function_node
class F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__encode free_function
class F78_infrastructure_infrastructure_infrastructure__tddd__type_signals_codec__encode function_node
class F106_infrastructure_infrastructure_infrastructure__tddd__type_signals_evaluator__execute_type_signals_for_layer free_function
class F106_infrastructure_infrastructure_infrastructure__tddd__type_signals_evaluator__execute_type_signals_for_layer function_node
class T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_NotFound variant_node
class T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_NotRegularFile variant_node
class T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_Io variant_node
class T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_Decode variant_node
class T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError_StaleHash variant_node
class T66_infrastructure_infrastructure_LoadCatalogueSpecSignalsForViewError__self error_type
class F112_infrastructure_infrastructure_infrastructure__verify__catalogue_spec_signals__compute_catalogue_declaration_hash free_function
class F112_infrastructure_infrastructure_infrastructure__verify__catalogue_spec_signals__compute_catalogue_declaration_hash function_node
class T38_cli_driver_cli_driver_CapabilityDriver_new method_node
class T38_cli_driver_cli_driver_CapabilityDriver_handle method_node
class T47_cli_driver_cli_driver_CapabilityExecDriverInput__self dto
class T41_cli_driver_cli_driver_CapabilityResumeArg_Fresh variant_node
class T41_cli_driver_cli_driver_CapabilityResumeArg_ResumeWithoutTarget variant_node
class T41_cli_driver_cli_driver_CapabilityResumeArg_Resume variant_node
class T41_cli_driver_cli_driver_CapabilityResumeArg_into_domain method_node
class T41_cli_driver_cli_driver_CapabilityResumeArg__self dto
class T43_cli_driver_cli_driver_TargetArtifactPathArg_into_domain method_node
class T43_cli_driver_cli_driver_TargetArtifactPathArg__self dto
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_new method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_codex method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_claude method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_local method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_run_fix_local_resolve method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_check_approved method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_results method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_classify method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_files method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_validate_scope method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_get_briefing method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_persist_commit_hash method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_new method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_gate method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_new method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_adr_user method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_adr_user method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_spec_adr method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_spec_adr method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_catalog_spec method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_catalog_spec method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_calc_impl_catalog method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_check_impl_catalog method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver method_node
class T25_cli_cli_CapabilityCommand_Exec variant_node
class T25_cli_cli_CapabilityCommand__self dto
class T26_cli_cli_CapabilityExecArgs__self dto
class T21_cli_cli_ReviewCommand_CodexLocal variant_node
class T21_cli_cli_ReviewCommand_ClaudeLocal variant_node
class T21_cli_cli_ReviewCommand_Local variant_node
class T21_cli_cli_ReviewCommand_FixLocal variant_node
class T21_cli_cli_ReviewCommand_CheckApproved variant_node
class T21_cli_cli_ReviewCommand_Results variant_node
class T21_cli_cli_ReviewCommand_Classify variant_node
class T21_cli_cli_ReviewCommand_Files variant_node
class T21_cli_cli_ReviewCommand__self dto
class F42_cli_cli_cli__commands__capability__execute free_function
class F42_cli_cli_cli__commands__capability__execute function_node
class F38_cli_cli_cli__commands__review__execute free_function
class F38_cli_cli_cli__commands__review__execute function_node
```
