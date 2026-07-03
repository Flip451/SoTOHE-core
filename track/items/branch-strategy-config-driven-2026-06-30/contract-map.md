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
  subgraph domain_domain_module_branch_strategy["domain::branch_strategy"]
    direction TB
  subgraph T36_domain_domain_BranchStrategySnapshot["branch_strategy::BranchStrategySnapshot"]
    direction TB
    T36_domain_domain_BranchStrategySnapshot__self[BranchStrategySnapshot]
    T36_domain_domain_BranchStrategySnapshot_new([new])
    T36_domain_domain_BranchStrategySnapshot_base_branch([base_branch])
    T36_domain_domain_BranchStrategySnapshot_merge_target([merge_target])
    T36_domain_domain_BranchStrategySnapshot_merge_method([merge_method])
  end
  subgraph T25_domain_domain_MergeMethod["branch_strategy::MergeMethod"]
    direction TB
    T25_domain_domain_MergeMethod__self[MergeMethod]
    T25_domain_domain_MergeMethod_Squash[Squash]
    T25_domain_domain_MergeMethod_Merge[Merge]
    T25_domain_domain_MergeMethod_Rebase[Rebase]
  end
  end
  subgraph domain_domain_module_track["domain::track"]
    direction TB
  subgraph T27_domain_domain_TrackMetadata["track::TrackMetadata"]
    direction TB
    T27_domain_domain_TrackMetadata__self[TrackMetadata]
    T27_domain_domain_TrackMetadata_new([new])
    T27_domain_domain_TrackMetadata_with_branch([with_branch])
    T27_domain_domain_TrackMetadata_id([id])
    T27_domain_domain_TrackMetadata_branch([branch])
    T27_domain_domain_TrackMetadata_is_activated([is_activated])
    T27_domain_domain_TrackMetadata_set_branch([set_branch])
    T27_domain_domain_TrackMetadata_title([title])
    T27_domain_domain_TrackMetadata_status_override([status_override])
    T27_domain_domain_TrackMetadata_set_status_override([set_status_override])
    T27_domain_domain_TrackMetadata_branch_strategy_snapshot([branch_strategy_snapshot])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_branch_strategy["usecase::branch_strategy"]
    direction TB
  subgraph R34_usecase_usecase_BranchStrategyPort["branch_strategy::BranchStrategyPort"]
    direction TB
    R34_usecase_usecase_BranchStrategyPort__self[BranchStrategyPort]
    R34_usecase_usecase_BranchStrategyPort_base_branch([base_branch])
    R34_usecase_usecase_BranchStrategyPort_merge_target([merge_target])
    R34_usecase_usecase_BranchStrategyPort_merge_method([merge_method])
    R34_usecase_usecase_BranchStrategyPort_track_prefix([track_prefix])
  end
  end
  subgraph usecase_usecase_module_dry_check_approved_driver["usecase::dry_check_approved_driver"]
    direction TB
  subgraph T48_usecase_usecase_DryCheckApprovedDriverInteractor["dry_check_approved_driver::DryCheckApprovedDriverInteractor"]
    direction TB
    T48_usecase_usecase_DryCheckApprovedDriverInteractor__self[DryCheckApprovedDriverInteractor]
    T48_usecase_usecase_DryCheckApprovedDriverInteractor_new([new])
  end
  subgraph R45_usecase_usecase_DryCheckApprovedDriverService["dry_check_approved_driver::DryCheckApprovedDriverService"]
    direction TB
    R45_usecase_usecase_DryCheckApprovedDriverService__self[DryCheckApprovedDriverService]
    R45_usecase_usecase_DryCheckApprovedDriverService_dry_check_approved([dry_check_approved])
  end
  end
  subgraph usecase_usecase_module_dry_driver["usecase::dry_driver"]
    direction TB
  subgraph T43_usecase_usecase_DryCheckApprovedDriverInput["dry_driver::DryCheckApprovedDriverInput"]
    direction TB
    T43_usecase_usecase_DryCheckApprovedDriverInput__self[DryCheckApprovedDriverInput]
  end
  subgraph T39_usecase_usecase_DryCheckApprovedOutcome["dry_driver::DryCheckApprovedOutcome"]
    direction TB
    T39_usecase_usecase_DryCheckApprovedOutcome__self[DryCheckApprovedOutcome]
    T39_usecase_usecase_DryCheckApprovedOutcome_Approved[Approved]
    T39_usecase_usecase_DryCheckApprovedOutcome_Blocked[Blocked]
    T39_usecase_usecase_DryCheckApprovedOutcome_Failure[Failure]
  end
  subgraph T35_usecase_usecase_DryDriverInteractor["dry_driver::DryDriverInteractor"]
    direction TB
    T35_usecase_usecase_DryDriverInteractor__self[DryDriverInteractor]
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
  subgraph T38_usecase_usecase_DryWriteFindingSummary["dry_driver::DryWriteFindingSummary"]
    direction TB
    T38_usecase_usecase_DryWriteFindingSummary__self[DryWriteFindingSummary]
  end
  subgraph T31_usecase_usecase_DryWriteOutcome["dry_driver::DryWriteOutcome"]
    direction TB
    T31_usecase_usecase_DryWriteOutcome__self[DryWriteOutcome]
    T31_usecase_usecase_DryWriteOutcome_Success[Success]
    T31_usecase_usecase_DryWriteOutcome_Failure[Failure]
  end
  subgraph R29_usecase_usecase_DryDriverPort["dry_driver::DryDriverPort"]
    direction TB
    R29_usecase_usecase_DryDriverPort__self[DryDriverPort]
    R29_usecase_usecase_DryDriverPort_dry_fix_local([dry_fix_local])
  end
  subgraph R32_usecase_usecase_DryDriverService["dry_driver::DryDriverService"]
    direction TB
    R32_usecase_usecase_DryDriverService__self[DryDriverService]
    R32_usecase_usecase_DryDriverService_dry_fix_local([dry_fix_local])
  end
  end
  subgraph usecase_usecase_module_dry_driver_shared["usecase::dry_driver_shared"]
    direction TB
  subgraph T34_usecase_usecase_DryBaseBranchError["dry_driver_shared::DryBaseBranchError"]
    direction TB
    T34_usecase_usecase_DryBaseBranchError__self[DryBaseBranchError]
    T34_usecase_usecase_DryBaseBranchError_MetadataPathOutsideRepo[MetadataPathOutsideRepo]
    T34_usecase_usecase_DryBaseBranchError_MetadataSymlinkRejected[MetadataSymlinkRejected]
    T34_usecase_usecase_DryBaseBranchError_MetadataNotFound[MetadataNotFound]
    T34_usecase_usecase_DryBaseBranchError_MetadataReadFailed[MetadataReadFailed]
    T34_usecase_usecase_DryBaseBranchError_MetadataDecodeFailed[MetadataDecodeFailed]
  end
  subgraph T37_usecase_usecase_DryCheckStorageHandle["dry_driver_shared::DryCheckStorageHandle"]
    direction TB
    T37_usecase_usecase_DryCheckStorageHandle__self[DryCheckStorageHandle]
  end
  subgraph T32_usecase_usecase_DryRepoWorkspace["dry_driver_shared::DryRepoWorkspace"]
    direction TB
    T32_usecase_usecase_DryRepoWorkspace__self[DryRepoWorkspace]
  end
  subgraph T37_usecase_usecase_DryRepoWorkspaceError["dry_driver_shared::DryRepoWorkspaceError"]
    direction TB
    T37_usecase_usecase_DryRepoWorkspaceError__self[DryRepoWorkspaceError]
    T37_usecase_usecase_DryRepoWorkspaceError_GitDiscoveryFailed[GitDiscoveryFailed]
    T37_usecase_usecase_DryRepoWorkspaceError_RepoRootCanonicalizeFailed[RepoRootCanonicalizeFailed]
    T37_usecase_usecase_DryRepoWorkspaceError_ItemsDirSymlinkRejected[ItemsDirSymlinkRejected]
    T37_usecase_usecase_DryRepoWorkspaceError_ItemsDirInvalid[ItemsDirInvalid]
  end
  subgraph T41_usecase_usecase_GitDiscoveryFailureDetail["dry_driver_shared::GitDiscoveryFailureDetail"]
    direction TB
    T41_usecase_usecase_GitDiscoveryFailureDetail__self[GitDiscoveryFailureDetail]
    T41_usecase_usecase_GitDiscoveryFailureDetail_new([new])
    T41_usecase_usecase_GitDiscoveryFailureDetail_as_str([as_str])
  end
  subgraph T31_usecase_usecase_IoFailureDetail["dry_driver_shared::IoFailureDetail"]
    direction TB
    T31_usecase_usecase_IoFailureDetail__self[IoFailureDetail]
    T31_usecase_usecase_IoFailureDetail_new([new])
    T31_usecase_usecase_IoFailureDetail_as_str([as_str])
  end
  subgraph T43_usecase_usecase_MetadataDecodeFailureDetail["dry_driver_shared::MetadataDecodeFailureDetail"]
    direction TB
    T43_usecase_usecase_MetadataDecodeFailureDetail__self[MetadataDecodeFailureDetail]
    T43_usecase_usecase_MetadataDecodeFailureDetail_new([new])
    T43_usecase_usecase_MetadataDecodeFailureDetail_as_str([as_str])
  end
  subgraph R33_usecase_usecase_DryBaseBranchPort["dry_driver_shared::DryBaseBranchPort"]
    direction TB
    R33_usecase_usecase_DryBaseBranchPort__self[DryBaseBranchPort]
    R33_usecase_usecase_DryBaseBranchPort_resolve_base_branch([resolve_base_branch])
  end
  subgraph R42_usecase_usecase_DryCheckStorageFactoryPort["dry_driver_shared::DryCheckStorageFactoryPort"]
    direction TB
    R42_usecase_usecase_DryCheckStorageFactoryPort__self[DryCheckStorageFactoryPort]
    R42_usecase_usecase_DryCheckStorageFactoryPort_build([build])
  end
  subgraph R38_usecase_usecase_DryDiffBaseFactoryPort["dry_driver_shared::DryDiffBaseFactoryPort"]
    direction TB
    R38_usecase_usecase_DryDiffBaseFactoryPort__self[DryDiffBaseFactoryPort]
    R38_usecase_usecase_DryDiffBaseFactoryPort_build([build])
  end
  subgraph R31_usecase_usecase_DryRepoRootPort["dry_driver_shared::DryRepoRootPort"]
    direction TB
    R31_usecase_usecase_DryRepoRootPort__self[DryRepoRootPort]
    R31_usecase_usecase_DryRepoRootPort_resolve([resolve])
  end
  end
  subgraph usecase_usecase_module_dry_results_driver["usecase::dry_results_driver"]
    direction TB
  subgraph T42_usecase_usecase_DryResultsDriverInteractor["dry_results_driver::DryResultsDriverInteractor"]
    direction TB
    T42_usecase_usecase_DryResultsDriverInteractor__self[DryResultsDriverInteractor]
    T42_usecase_usecase_DryResultsDriverInteractor_new([new])
  end
  subgraph T33_usecase_usecase_DryResultsOutcome["dry_results_driver::DryResultsOutcome"]
    direction TB
    T33_usecase_usecase_DryResultsOutcome__self[DryResultsOutcome]
    T33_usecase_usecase_DryResultsOutcome_Success[Success]
    T33_usecase_usecase_DryResultsOutcome_Failure[Failure]
  end
  subgraph T39_usecase_usecase_DryResultsRecordSummary["dry_results_driver::DryResultsRecordSummary"]
    direction TB
    T39_usecase_usecase_DryResultsRecordSummary__self[DryResultsRecordSummary]
  end
  subgraph T40_usecase_usecase_DryResultsVerdictSummary["dry_results_driver::DryResultsVerdictSummary"]
    direction TB
    T40_usecase_usecase_DryResultsVerdictSummary__self[DryResultsVerdictSummary]
    T40_usecase_usecase_DryResultsVerdictSummary_NotAViolation[NotAViolation]
    T40_usecase_usecase_DryResultsVerdictSummary_Accepted[Accepted]
    T40_usecase_usecase_DryResultsVerdictSummary_Violation[Violation]
  end
  subgraph R39_usecase_usecase_DryResultsDriverService["dry_results_driver::DryResultsDriverService"]
    direction TB
    R39_usecase_usecase_DryResultsDriverService__self[DryResultsDriverService]
    R39_usecase_usecase_DryResultsDriverService_dry_results([dry_results])
  end
  end
  subgraph usecase_usecase_module_dry_write_driver["usecase::dry_write_driver"]
    direction TB
  subgraph T50_usecase_usecase_AgentConfigResolutionFailureDetail["dry_write_driver::AgentConfigResolutionFailureDetail"]
    direction TB
    T50_usecase_usecase_AgentConfigResolutionFailureDetail__self[AgentConfigResolutionFailureDetail]
    T50_usecase_usecase_AgentConfigResolutionFailureDetail_new([new])
    T50_usecase_usecase_AgentConfigResolutionFailureDetail_as_str([as_str])
  end
  subgraph T30_usecase_usecase_CapabilityName["dry_write_driver::CapabilityName"]
    direction TB
    T30_usecase_usecase_CapabilityName__self[CapabilityName]
    T30_usecase_usecase_CapabilityName_try_new([try_new])
    T30_usecase_usecase_CapabilityName_as_str([as_str])
  end
  subgraph T44_usecase_usecase_DiffHunkListingFailureDetail["dry_write_driver::DiffHunkListingFailureDetail"]
    direction TB
    T44_usecase_usecase_DiffHunkListingFailureDetail__self[DiffHunkListingFailureDetail]
    T44_usecase_usecase_DiffHunkListingFailureDetail_new([new])
    T44_usecase_usecase_DiffHunkListingFailureDetail_as_str([as_str])
  end
  subgraph T45_usecase_usecase_DryCheckServiceFactoryCommand["dry_write_driver::DryCheckServiceFactoryCommand"]
    direction TB
    T45_usecase_usecase_DryCheckServiceFactoryCommand__self[DryCheckServiceFactoryCommand]
  end
  subgraph T43_usecase_usecase_DryCheckServiceFactoryError["dry_write_driver::DryCheckServiceFactoryError"]
    direction TB
    T43_usecase_usecase_DryCheckServiceFactoryError__self[DryCheckServiceFactoryError]
    T43_usecase_usecase_DryCheckServiceFactoryError_EmbeddingModelLoadFailed[EmbeddingModelLoadFailed]
    T43_usecase_usecase_DryCheckServiceFactoryError_SemanticIndexOpenFailed[SemanticIndexOpenFailed]
    T43_usecase_usecase_DryCheckServiceFactoryError_AgentConfigResolutionFailed[AgentConfigResolutionFailed]
  end
  subgraph T44_usecase_usecase_DryCheckServiceFactoryOutput["dry_write_driver::DryCheckServiceFactoryOutput"]
    direction TB
    T44_usecase_usecase_DryCheckServiceFactoryOutput__self[DryCheckServiceFactoryOutput]
  end
  subgraph T39_usecase_usecase_DryCorpusFragmentsError["dry_write_driver::DryCorpusFragmentsError"]
    direction TB
    T39_usecase_usecase_DryCorpusFragmentsError__self[DryCorpusFragmentsError]
    T39_usecase_usecase_DryCorpusFragmentsError_WorkspaceRootSymlinkRejected[WorkspaceRootSymlinkRejected]
    T39_usecase_usecase_DryCorpusFragmentsError_WorkspaceRootInvalid[WorkspaceRootInvalid]
    T39_usecase_usecase_DryCorpusFragmentsError_DiffHunkListingFailed[DiffHunkListingFailed]
    T39_usecase_usecase_DryCorpusFragmentsError_FragmentExtractionFailed[FragmentExtractionFailed]
    T39_usecase_usecase_DryCorpusFragmentsError_FragmentPathNormalizationFailed[FragmentPathNormalizationFailed]
  end
  subgraph T40_usecase_usecase_DryCorpusFragmentsOutput["dry_write_driver::DryCorpusFragmentsOutput"]
    direction TB
    T40_usecase_usecase_DryCorpusFragmentsOutput__self[DryCorpusFragmentsOutput]
  end
  subgraph T42_usecase_usecase_DryCorpusRootManifestError["dry_write_driver::DryCorpusRootManifestError"]
    direction TB
    T42_usecase_usecase_DryCorpusRootManifestError__self[DryCorpusRootManifestError]
    T42_usecase_usecase_DryCorpusRootManifestError_RepoRootCanonicalizeFailed[RepoRootCanonicalizeFailed]
    T42_usecase_usecase_DryCorpusRootManifestError_ManifestPathOutsideRepo[ManifestPathOutsideRepo]
    T42_usecase_usecase_DryCorpusRootManifestError_ManifestSerializeFailed[ManifestSerializeFailed]
    T42_usecase_usecase_DryCorpusRootManifestError_ManifestSymlinkRejected[ManifestSymlinkRejected]
    T42_usecase_usecase_DryCorpusRootManifestError_ManifestParentCreateFailed[ManifestParentCreateFailed]
    T42_usecase_usecase_DryCorpusRootManifestError_ManifestWriteFailed[ManifestWriteFailed]
  end
  subgraph T40_usecase_usecase_DryWriteConfigResolution["dry_write_driver::DryWriteConfigResolution"]
    direction TB
    T40_usecase_usecase_DryWriteConfigResolution__self[DryWriteConfigResolution]
  end
  subgraph T40_usecase_usecase_DryWriteDriverInteractor["dry_write_driver::DryWriteDriverInteractor"]
    direction TB
    T40_usecase_usecase_DryWriteDriverInteractor__self[DryWriteDriverInteractor]
    T40_usecase_usecase_DryWriteDriverInteractor_new([new])
  end
  subgraph T47_usecase_usecase_EmbeddingModelLoadFailureDetail["dry_write_driver::EmbeddingModelLoadFailureDetail"]
    direction TB
    T47_usecase_usecase_EmbeddingModelLoadFailureDetail__self[EmbeddingModelLoadFailureDetail]
    T47_usecase_usecase_EmbeddingModelLoadFailureDetail_new([new])
    T47_usecase_usecase_EmbeddingModelLoadFailureDetail_as_str([as_str])
  end
  subgraph T45_usecase_usecase_FragmentPipelineFailureDetail["dry_write_driver::FragmentPipelineFailureDetail"]
    direction TB
    T45_usecase_usecase_FragmentPipelineFailureDetail__self[FragmentPipelineFailureDetail]
    T45_usecase_usecase_FragmentPipelineFailureDetail_new([new])
    T45_usecase_usecase_FragmentPipelineFailureDetail_as_str([as_str])
  end
  subgraph T46_usecase_usecase_SemanticIndexOpenFailureDetail["dry_write_driver::SemanticIndexOpenFailureDetail"]
    direction TB
    T46_usecase_usecase_SemanticIndexOpenFailureDetail__self[SemanticIndexOpenFailureDetail]
    T46_usecase_usecase_SemanticIndexOpenFailureDetail_new([new])
    T46_usecase_usecase_SemanticIndexOpenFailureDetail_as_str([as_str])
  end
  subgraph T42_usecase_usecase_SerializationFailureDetail["dry_write_driver::SerializationFailureDetail"]
    direction TB
    T42_usecase_usecase_SerializationFailureDetail__self[SerializationFailureDetail]
    T42_usecase_usecase_SerializationFailureDetail_new([new])
    T42_usecase_usecase_SerializationFailureDetail_as_str([as_str])
  end
  subgraph R42_usecase_usecase_DryCheckServiceFactoryPort["dry_write_driver::DryCheckServiceFactoryPort"]
    direction TB
    R42_usecase_usecase_DryCheckServiceFactoryPort__self[DryCheckServiceFactoryPort]
    R42_usecase_usecase_DryCheckServiceFactoryPort_build([build])
  end
  subgraph R38_usecase_usecase_DryCorpusFragmentsPort["dry_write_driver::DryCorpusFragmentsPort"]
    direction TB
    R38_usecase_usecase_DryCorpusFragmentsPort__self[DryCorpusFragmentsPort]
    R38_usecase_usecase_DryCorpusFragmentsPort_build([build])
  end
  subgraph R47_usecase_usecase_DryCorpusRootManifestWriterPort["dry_write_driver::DryCorpusRootManifestWriterPort"]
    direction TB
    R47_usecase_usecase_DryCorpusRootManifestWriterPort__self[DryCorpusRootManifestWriterPort]
    R47_usecase_usecase_DryCorpusRootManifestWriterPort_write([write])
  end
  subgraph R36_usecase_usecase_DryTierTelemetryPort["dry_write_driver::DryTierTelemetryPort"]
    direction TB
    R36_usecase_usecase_DryTierTelemetryPort__self[DryTierTelemetryPort]
    R36_usecase_usecase_DryTierTelemetryPort_record([record])
  end
  subgraph R40_usecase_usecase_DryWriteConfigLoaderPort["dry_write_driver::DryWriteConfigLoaderPort"]
    direction TB
    R40_usecase_usecase_DryWriteConfigLoaderPort__self[DryWriteConfigLoaderPort]
    R40_usecase_usecase_DryWriteConfigLoaderPort_load([load])
  end
  subgraph R37_usecase_usecase_DryWriteDriverService["dry_write_driver::DryWriteDriverService"]
    direction TB
    R37_usecase_usecase_DryWriteDriverService__self[DryWriteDriverService]
    R37_usecase_usecase_DryWriteDriverService_dry_write([dry_write])
  end
  end
  subgraph usecase_usecase_module_fixpoint_resolve["usecase::fixpoint_resolve"]
    direction TB
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
  subgraph usecase_usecase_module_fixpoint_resolve_driver["usecase::fixpoint_resolve_driver"]
    direction TB
  subgraph T47_usecase_usecase_DryCheckConfigLoadFailureDetail["fixpoint_resolve_driver::DryCheckConfigLoadFailureDetail"]
    direction TB
    T47_usecase_usecase_DryCheckConfigLoadFailureDetail__self[DryCheckConfigLoadFailureDetail]
    T47_usecase_usecase_DryCheckConfigLoadFailureDetail_new([new])
    T47_usecase_usecase_DryCheckConfigLoadFailureDetail_as_str([as_str])
  end
  subgraph T41_usecase_usecase_DryCheckConfigLoaderError["fixpoint_resolve_driver::DryCheckConfigLoaderError"]
    direction TB
    T41_usecase_usecase_DryCheckConfigLoaderError__self[DryCheckConfigLoaderError]
    T41_usecase_usecase_DryCheckConfigLoaderError_RepoRootCanonicalizeFailed[RepoRootCanonicalizeFailed]
    T41_usecase_usecase_DryCheckConfigLoaderError_ConfigPathCanonicalizeFailed[ConfigPathCanonicalizeFailed]
    T41_usecase_usecase_DryCheckConfigLoaderError_ConfigPathOutsideRepo[ConfigPathOutsideRepo]
    T41_usecase_usecase_DryCheckConfigLoaderError_ConfigSymlinkRejected[ConfigSymlinkRejected]
    T41_usecase_usecase_DryCheckConfigLoaderError_ConfigLoadFailed[ConfigLoadFailed]
    T41_usecase_usecase_DryCheckConfigLoaderError_InvalidKnownBadPercent[InvalidKnownBadPercent]
    T41_usecase_usecase_DryCheckConfigLoaderError_InvalidMaxParallelism[InvalidMaxParallelism]
  end
  subgraph T42_usecase_usecase_FixpointResolveDriverInput["fixpoint_resolve_driver::FixpointResolveDriverInput"]
    direction TB
    T42_usecase_usecase_FixpointResolveDriverInput__self[FixpointResolveDriverInput]
  end
  subgraph T47_usecase_usecase_FixpointResolveDriverInteractor["fixpoint_resolve_driver::FixpointResolveDriverInteractor"]
    direction TB
    T47_usecase_usecase_FixpointResolveDriverInteractor__self[FixpointResolveDriverInteractor]
    T47_usecase_usecase_FixpointResolveDriverInteractor_new([new])
  end
  subgraph T44_usecase_usecase_FixpointResolveDriverOutcome["fixpoint_resolve_driver::FixpointResolveDriverOutcome"]
    direction TB
    T44_usecase_usecase_FixpointResolveDriverOutcome__self[FixpointResolveDriverOutcome]
    T44_usecase_usecase_FixpointResolveDriverOutcome_RunDfp[RunDfp]
    T44_usecase_usecase_FixpointResolveDriverOutcome_RunRfp[RunRfp]
    T44_usecase_usecase_FixpointResolveDriverOutcome_RunRefVerify[RunRefVerify]
    T44_usecase_usecase_FixpointResolveDriverOutcome_Commit[Commit]
    T44_usecase_usecase_FixpointResolveDriverOutcome_Failure[Failure]
  end
  subgraph T40_usecase_usecase_FixpointWorkspaceContext["fixpoint_resolve_driver::FixpointWorkspaceContext"]
    direction TB
    T40_usecase_usecase_FixpointWorkspaceContext__self[FixpointWorkspaceContext]
  end
  subgraph T45_usecase_usecase_FixpointWorkspaceContextError["fixpoint_resolve_driver::FixpointWorkspaceContextError"]
    direction TB
    T45_usecase_usecase_FixpointWorkspaceContextError__self[FixpointWorkspaceContextError]
    T45_usecase_usecase_FixpointWorkspaceContextError_GitDiscoveryFailed[GitDiscoveryFailed]
    T45_usecase_usecase_FixpointWorkspaceContextError_RepoRootCanonicalizeFailed[RepoRootCanonicalizeFailed]
    T45_usecase_usecase_FixpointWorkspaceContextError_ItemsDirSymlinkCheckFailed[ItemsDirSymlinkCheckFailed]
    T45_usecase_usecase_FixpointWorkspaceContextError_ItemsDirIsSymlink[ItemsDirIsSymlink]
    T45_usecase_usecase_FixpointWorkspaceContextError_ItemsDirInvalid[ItemsDirInvalid]
    T45_usecase_usecase_FixpointWorkspaceContextError_ProjectRootPatternInvalid[ProjectRootPatternInvalid]
    T45_usecase_usecase_FixpointWorkspaceContextError_ProjectRootCanonicalizeFailed[ProjectRootCanonicalizeFailed]
    T45_usecase_usecase_FixpointWorkspaceContextError_MetadataNotFound[MetadataNotFound]
    T45_usecase_usecase_FixpointWorkspaceContextError_MetadataSymlinkRejected[MetadataSymlinkRejected]
    T45_usecase_usecase_FixpointWorkspaceContextError_MetadataReadFailed[MetadataReadFailed]
    T45_usecase_usecase_FixpointWorkspaceContextError_MetadataDecodeFailed[MetadataDecodeFailed]
  end
  subgraph R40_usecase_usecase_DryCheckConfigLoaderPort["fixpoint_resolve_driver::DryCheckConfigLoaderPort"]
    direction TB
    R40_usecase_usecase_DryCheckConfigLoaderPort__self[DryCheckConfigLoaderPort]
    R40_usecase_usecase_DryCheckConfigLoaderPort_load([load])
  end
  subgraph R42_usecase_usecase_FixpointDryGateFactoryPort["fixpoint_resolve_driver::FixpointDryGateFactoryPort"]
    direction TB
    R42_usecase_usecase_FixpointDryGateFactoryPort__self[FixpointDryGateFactoryPort]
    R42_usecase_usecase_FixpointDryGateFactoryPort_build([build])
  end
  subgraph R44_usecase_usecase_FixpointGateStateFactoryPort["fixpoint_resolve_driver::FixpointGateStateFactoryPort"]
    direction TB
    R44_usecase_usecase_FixpointGateStateFactoryPort__self[FixpointGateStateFactoryPort]
    R44_usecase_usecase_FixpointGateStateFactoryPort_build_review_gate([build_review_gate])
    R44_usecase_usecase_FixpointGateStateFactoryPort_build_ref_verify_gate([build_ref_verify_gate])
  end
  subgraph R44_usecase_usecase_FixpointResolveDriverService["fixpoint_resolve_driver::FixpointResolveDriverService"]
    direction TB
    R44_usecase_usecase_FixpointResolveDriverService__self[FixpointResolveDriverService]
    R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve([fixpoint_resolve])
  end
  subgraph R44_usecase_usecase_FixpointWorkspaceContextPort["fixpoint_resolve_driver::FixpointWorkspaceContextPort"]
    direction TB
    R44_usecase_usecase_FixpointWorkspaceContextPort__self[FixpointWorkspaceContextPort]
    R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context([resolve_context])
  end
  end
  subgraph usecase_usecase_module_track_service["usecase::track_service"]
    direction TB
  subgraph R28_usecase_usecase_TrackService["track_service::TrackService"]
    direction TB
    R28_usecase_usecase_TrackService__self[TrackService]
    R28_usecase_usecase_TrackService_init([init])
    R28_usecase_usecase_TrackService_transition([transition])
    R28_usecase_usecase_TrackService_resolve([resolve])
    R28_usecase_usecase_TrackService_branch_create([branch_create])
    R28_usecase_usecase_TrackService_branch_switch([branch_switch])
    R28_usecase_usecase_TrackService_views_validate([views_validate])
    R28_usecase_usecase_TrackService_views_sync([views_sync])
    R28_usecase_usecase_TrackService_add_task([add_task])
    R28_usecase_usecase_TrackService_set_override([set_override])
    R28_usecase_usecase_TrackService_clear_override([clear_override])
    R28_usecase_usecase_TrackService_next_task([next_task])
    R28_usecase_usecase_TrackService_task_counts([task_counts])
    R28_usecase_usecase_TrackService_archive([archive])
    R28_usecase_usecase_TrackService_detect_active([detect_active])
    R28_usecase_usecase_TrackService_switch_base([switch_base])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_branch_strategy["infrastructure::branch_strategy"]
    direction TB
  subgraph T55_infrastructure_infrastructure_BranchStrategyConfigError["branch_strategy::BranchStrategyConfigError"]
    direction TB
    T55_infrastructure_infrastructure_BranchStrategyConfigError__self[BranchStrategyConfigError]
    T55_infrastructure_infrastructure_BranchStrategyConfigError_Io[Io]
    T55_infrastructure_infrastructure_BranchStrategyConfigError_Parse[Parse]
  end
  subgraph T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter["branch_strategy::JsonConfigBranchStrategyAdapter"]
    direction TB
    T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self[JsonConfigBranchStrategyAdapter]
    T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new([new])
  end
  subgraph T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter["branch_strategy::SnapshotBranchStrategyAdapter"]
    direction TB
    T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self[SnapshotBranchStrategyAdapter]
    T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_dry_check["infrastructure::dry_check"]
    direction TB
  subgraph T52_infrastructure_infrastructure_CodexDryFixLocalRunner["dry_check::dry_fix_local::CodexDryFixLocalRunner"]
    direction TB
    T52_infrastructure_infrastructure_CodexDryFixLocalRunner__self[CodexDryFixLocalRunner]
    T52_infrastructure_infrastructure_CodexDryFixLocalRunner_new([new])
    T52_infrastructure_infrastructure_CodexDryFixLocalRunner_dry_run_fix_local([dry_run_fix_local])
  end
  subgraph T59_infrastructure_infrastructure_DryCheckServiceFactoryAdapter["dry_check::dry_write_driver::DryCheckServiceFactoryAdapter"]
    direction TB
    T59_infrastructure_infrastructure_DryCheckServiceFactoryAdapter__self[DryCheckServiceFactoryAdapter]
  end
  subgraph T46_infrastructure_infrastructure_DryDriverAdapter["dry_check::dry_fix_local::DryDriverAdapter"]
    direction TB
    T46_infrastructure_infrastructure_DryDriverAdapter__self[DryDriverAdapter]
    T46_infrastructure_infrastructure_DryDriverAdapter_new([new])
  end
  subgraph T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter["dry_check::diff_base_resolver::FsDiffBaseResolverAdapter"]
    direction TB
    T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self[FsDiffBaseResolverAdapter]
    T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter_new([new])
  end
  subgraph T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter["dry_check::approval_factory::FsDryApprovalFactoryAdapter"]
    direction TB
    T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter__self[FsDryApprovalFactoryAdapter]
  end
  subgraph T52_infrastructure_infrastructure_FsDryBaseBranchAdapter["dry_check::dry_driver_shared::FsDryBaseBranchAdapter"]
    direction TB
    T52_infrastructure_infrastructure_FsDryBaseBranchAdapter__self[FsDryBaseBranchAdapter]
  end
  subgraph T61_infrastructure_infrastructure_FsDryCheckStorageFactoryAdapter["dry_check::dry_driver_shared::FsDryCheckStorageFactoryAdapter"]
    direction TB
    T61_infrastructure_infrastructure_FsDryCheckStorageFactoryAdapter__self[FsDryCheckStorageFactoryAdapter]
  end
  subgraph T57_infrastructure_infrastructure_FsDryCorpusFragmentsAdapter["dry_check::dry_write_driver::FsDryCorpusFragmentsAdapter"]
    direction TB
    T57_infrastructure_infrastructure_FsDryCorpusFragmentsAdapter__self[FsDryCorpusFragmentsAdapter]
  end
  subgraph T60_infrastructure_infrastructure_FsDryCorpusRootManifestAdapter["dry_check::dry_write_driver::FsDryCorpusRootManifestAdapter"]
    direction TB
    T60_infrastructure_infrastructure_FsDryCorpusRootManifestAdapter__self[FsDryCorpusRootManifestAdapter]
  end
  subgraph T57_infrastructure_infrastructure_FsDryDiffBaseFactoryAdapter["dry_check::dry_driver_shared::FsDryDiffBaseFactoryAdapter"]
    direction TB
    T57_infrastructure_infrastructure_FsDryDiffBaseFactoryAdapter__self[FsDryDiffBaseFactoryAdapter]
  end
  subgraph T50_infrastructure_infrastructure_FsDryRepoRootAdapter["dry_check::dry_driver_shared::FsDryRepoRootAdapter"]
    direction TB
    T50_infrastructure_infrastructure_FsDryRepoRootAdapter__self[FsDryRepoRootAdapter]
  end
  subgraph T59_infrastructure_infrastructure_FsDryWriteConfigLoaderAdapter["dry_check::dry_write_driver::FsDryWriteConfigLoaderAdapter"]
    direction TB
    T59_infrastructure_infrastructure_FsDryWriteConfigLoaderAdapter__self[FsDryWriteConfigLoaderAdapter]
  end
  subgraph T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter["dry_check::dry_write_driver::RecordingDryTierTelemetryAdapter"]
    direction TB
    T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter__self[RecordingDryTierTelemetryAdapter]
    T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_track["infrastructure::track"]
    direction TB
  subgraph T60_infrastructure_infrastructure_BranchStrategySnapshotDocument["track::codec::BranchStrategySnapshotDocument"]
    direction TB
    T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self[BranchStrategySnapshotDocument]
  end
  subgraph T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter["track::fixpoint_resolve_driver::FsDryCheckConfigLoaderAdapter"]
    direction TB
    T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter__self[FsDryCheckConfigLoaderAdapter]
  end
  subgraph T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter["track::fixpoint_resolve_driver::FsFixpointDryGateFactoryAdapter"]
    direction TB
    T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter__self[FsFixpointDryGateFactoryAdapter]
  end
  subgraph T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter["track::fixpoint_resolve_driver::FsFixpointGateStateFactoryAdapter"]
    direction TB
    T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter__self[FsFixpointGateStateFactoryAdapter]
  end
  subgraph T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter["track::fixpoint_resolve_driver::FsFixpointWorkspaceContextAdapter"]
    direction TB
    T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter__self[FsFixpointWorkspaceContextAdapter]
  end
  subgraph T54_infrastructure_infrastructure_FsReviewGateStateAdapter["track::gate_state::FsReviewGateStateAdapter"]
    direction TB
    T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self[FsReviewGateStateAdapter]
    T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new([new])
  end
  subgraph T49_infrastructure_infrastructure_MergeMethodDocument["track::codec::MergeMethodDocument"]
    direction TB
    T49_infrastructure_infrastructure_MergeMethodDocument__self[MergeMethodDocument]
    T49_infrastructure_infrastructure_MergeMethodDocument_Squash[Squash]
    T49_infrastructure_infrastructure_MergeMethodDocument_Merge[Merge]
    T49_infrastructure_infrastructure_MergeMethodDocument_Rebase[Rebase]
  end
  subgraph T45_infrastructure_infrastructure_TrackDocumentV2["track::codec::TrackDocumentV2"]
    direction TB
    T45_infrastructure_infrastructure_TrackDocumentV2__self[TrackDocumentV2]
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_dry["cli_driver::dry"]
    direction TB
  subgraph T31_cli_driver_cli_driver_DryDriver["dry::DryDriver"]
    direction TB
    T31_cli_driver_cli_driver_DryDriver__self[DryDriver]
    T31_cli_driver_cli_driver_DryDriver_new([new])
    T31_cli_driver_cli_driver_DryDriver_handle([handle])
  end
  end
  subgraph cli_driver_cli_driver_module_pr["cli_driver::pr"]
    direction TB
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
  subgraph cli_driver_cli_driver_module_track["cli_driver::track"]
    direction TB
  subgraph T33_cli_driver_cli_driver_TrackDriver["track::TrackDriver"]
    direction TB
    T33_cli_driver_cli_driver_TrackDriver__self[TrackDriver]
    T33_cli_driver_cli_driver_TrackDriver_new([new])
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
    T32_cli_driver_cli_driver_TrackInput_SwitchBase[SwitchBase]
    T32_cli_driver_cli_driver_TrackInput_FixpointResolve[FixpointResolve]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_dry["cli_composition::dry"]
    direction TB
  subgraph T50_cli_composition_cli_composition_DryCompositionRoot["dry::shim::DryCompositionRoot"]
    direction TB
    T50_cli_composition_cli_composition_DryCompositionRoot__self[DryCompositionRoot]
    T50_cli_composition_cli_composition_DryCompositionRoot_new([new])
    T50_cli_composition_cli_composition_DryCompositionRoot_dry_driver([dry_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_git["cli_composition::git"]
    direction TB
  subgraph T50_cli_composition_cli_composition_GitCompositionRoot["git::GitCompositionRoot"]
    direction TB
    T50_cli_composition_cli_composition_GitCompositionRoot__self[GitCompositionRoot]
    T50_cli_composition_cli_composition_GitCompositionRoot_new([new])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_add_all([git_add_all])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_add_from_file([git_add_from_file])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_commit_from_file([git_commit_from_file])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_note_from_file([git_note_from_file])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull([git_switch_and_pull])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull_in([git_switch_and_pull_in])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_unstage([git_unstage])
    T50_cli_composition_cli_composition_GitCompositionRoot_current_branch_track_id_strict([current_branch_track_id_strict])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_driver([git_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_track["cli_composition::track"]
    direction TB
  subgraph T52_cli_composition_cli_composition_TrackCompositionRoot["track::composition_root::TrackCompositionRoot"]
    direction TB
    T52_cli_composition_cli_composition_TrackCompositionRoot__self[TrackCompositionRoot]
    T52_cli_composition_cli_composition_TrackCompositionRoot_new([new])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_driver([track_driver])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task_resolved([track_add_task_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override_resolved([track_set_override_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override_resolved([track_clear_override_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task_resolved([track_next_task_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts_resolved([track_task_counts_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_detect_active_track_from_branch([detect_active_track_from_branch])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_commit_hash([track_set_commit_hash])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_signals([track_type_signals])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_graph([track_type_graph])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_graph([track_baseline_graph])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_contract_map([track_contract_map])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_spec_signals([track_catalogue_spec_signals])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_spec_element_hash([track_spec_element_hash])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_capture([track_baseline_capture])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_lint([track_lint])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_impl_signals([track_catalogue_impl_signals])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id([track_resolve_id])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root([track_resolve_id_from_root])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_for_write([track_resolve_id_for_write])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root_for_write([track_resolve_id_from_root_for_write])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_validate_id([track_validate_id])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_project_root([track_resolve_project_root])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_init([track_init])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_transition([track_transition])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_create([track_branch_create])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_switch([track_branch_switch])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve([track_resolve])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_validate([track_views_validate])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_sync([track_views_sync])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task([track_add_task])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override([track_set_override])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override([track_clear_override])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task([track_next_task])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts([track_task_counts])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_archive([track_archive])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_switch_base([track_switch_base])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T20_cli_cli_EnsurePrArgs["commands::pr::EnsurePrArgs"]
    direction TB
    T20_cli_cli_EnsurePrArgs__self[EnsurePrArgs]
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
    T20_cli_cli_TrackCommand_Lint[Lint]
    T20_cli_cli_TrackCommand_CatalogueImplSignals[CatalogueImplSignals]
    T20_cli_cli_TrackCommand_FixpointResolve[FixpointResolve]
    T20_cli_cli_TrackCommand_SetCommitHash[SetCommitHash]
    T20_cli_cli_TrackCommand_SwitchBase[SwitchBase]
    T20_cli_cli_TrackCommand_items_dir([items_dir])
  end
  subgraph T24_cli_cli_WaitAndMergeArgs["commands::pr::WaitAndMergeArgs"]
    direction TB
    T24_cli_cli_WaitAndMergeArgs__self[WaitAndMergeArgs]
  end
  F54_cli_cli_cli__commands__dry__execute_dry_check_approved[[execute_dry_check_approved]]
  end
end
T36_domain_domain_BranchStrategySnapshot_new --o T25_domain_domain_MergeMethod__self
T36_domain_domain_BranchStrategySnapshot_new --> T36_domain_domain_BranchStrategySnapshot__self
T36_domain_domain_BranchStrategySnapshot_merge_method --> T25_domain_domain_MergeMethod__self
T27_domain_domain_TrackMetadata_new --o T36_domain_domain_BranchStrategySnapshot__self
T27_domain_domain_TrackMetadata_new --> T27_domain_domain_TrackMetadata__self
T27_domain_domain_TrackMetadata_with_branch --o T36_domain_domain_BranchStrategySnapshot__self
T27_domain_domain_TrackMetadata_with_branch --> T27_domain_domain_TrackMetadata__self
T27_domain_domain_TrackMetadata_branch_strategy_snapshot --> T36_domain_domain_BranchStrategySnapshot__self
R34_usecase_usecase_BranchStrategyPort_merge_method --> T25_domain_domain_MergeMethod__self
T48_usecase_usecase_DryCheckApprovedDriverInteractor_new --> T48_usecase_usecase_DryCheckApprovedDriverInteractor__self
R45_usecase_usecase_DryCheckApprovedDriverService_dry_check_approved --o T43_usecase_usecase_DryCheckApprovedDriverInput__self
R45_usecase_usecase_DryCheckApprovedDriverService_dry_check_approved --> T39_usecase_usecase_DryCheckApprovedOutcome__self
T31_usecase_usecase_DryWriteOutcome_Success --o|findings| T38_usecase_usecase_DryWriteFindingSummary__self
R29_usecase_usecase_DryDriverPort_dry_fix_local --o T38_usecase_usecase_DryFixLocalDriverInput__self
R32_usecase_usecase_DryDriverService_dry_fix_local --o T38_usecase_usecase_DryFixLocalDriverInput__self
T34_usecase_usecase_DryBaseBranchError_MetadataSymlinkRejected --o|detail| T31_usecase_usecase_IoFailureDetail__self
T34_usecase_usecase_DryBaseBranchError_MetadataReadFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T34_usecase_usecase_DryBaseBranchError_MetadataDecodeFailed --o|detail| T43_usecase_usecase_MetadataDecodeFailureDetail__self
T37_usecase_usecase_DryRepoWorkspaceError_GitDiscoveryFailed --o|detail| T41_usecase_usecase_GitDiscoveryFailureDetail__self
T37_usecase_usecase_DryRepoWorkspaceError_RepoRootCanonicalizeFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T37_usecase_usecase_DryRepoWorkspaceError_ItemsDirSymlinkRejected --o|detail| T31_usecase_usecase_IoFailureDetail__self
T41_usecase_usecase_GitDiscoveryFailureDetail_new --> T41_usecase_usecase_GitDiscoveryFailureDetail__self
T31_usecase_usecase_IoFailureDetail_new --> T31_usecase_usecase_IoFailureDetail__self
T43_usecase_usecase_MetadataDecodeFailureDetail_new --> T43_usecase_usecase_MetadataDecodeFailureDetail__self
R33_usecase_usecase_DryBaseBranchPort_resolve_base_branch --> T34_usecase_usecase_DryBaseBranchError__self
R42_usecase_usecase_DryCheckStorageFactoryPort_build --> T37_usecase_usecase_DryCheckStorageHandle__self
R31_usecase_usecase_DryRepoRootPort_resolve --> T32_usecase_usecase_DryRepoWorkspace__self
R31_usecase_usecase_DryRepoRootPort_resolve --> T37_usecase_usecase_DryRepoWorkspaceError__self
T42_usecase_usecase_DryResultsDriverInteractor_new --> T42_usecase_usecase_DryResultsDriverInteractor__self
T33_usecase_usecase_DryResultsOutcome_Success --o|records| T39_usecase_usecase_DryResultsRecordSummary__self
T39_usecase_usecase_DryResultsRecordSummary__self --o|verdict| T40_usecase_usecase_DryResultsVerdictSummary__self
R39_usecase_usecase_DryResultsDriverService_dry_results --o T37_usecase_usecase_DryResultsDriverInput__self
R39_usecase_usecase_DryResultsDriverService_dry_results --> T33_usecase_usecase_DryResultsOutcome__self
T50_usecase_usecase_AgentConfigResolutionFailureDetail_new --> T50_usecase_usecase_AgentConfigResolutionFailureDetail__self
T30_usecase_usecase_CapabilityName_try_new --> T30_usecase_usecase_CapabilityName__self
T44_usecase_usecase_DiffHunkListingFailureDetail_new --> T44_usecase_usecase_DiffHunkListingFailureDetail__self
T43_usecase_usecase_DryCheckServiceFactoryError_EmbeddingModelLoadFailed --o|detail| T47_usecase_usecase_EmbeddingModelLoadFailureDetail__self
T43_usecase_usecase_DryCheckServiceFactoryError_SemanticIndexOpenFailed --o|detail| T46_usecase_usecase_SemanticIndexOpenFailureDetail__self
T43_usecase_usecase_DryCheckServiceFactoryError_AgentConfigResolutionFailed --o|capability_name| T30_usecase_usecase_CapabilityName__self
T43_usecase_usecase_DryCheckServiceFactoryError_AgentConfigResolutionFailed --o|detail| T50_usecase_usecase_AgentConfigResolutionFailureDetail__self
T39_usecase_usecase_DryCorpusFragmentsError_WorkspaceRootSymlinkRejected --o|detail| T31_usecase_usecase_IoFailureDetail__self
T39_usecase_usecase_DryCorpusFragmentsError_DiffHunkListingFailed --o|detail| T44_usecase_usecase_DiffHunkListingFailureDetail__self
T39_usecase_usecase_DryCorpusFragmentsError_FragmentExtractionFailed --o|detail| T45_usecase_usecase_FragmentPipelineFailureDetail__self
T39_usecase_usecase_DryCorpusFragmentsError_FragmentPathNormalizationFailed --o|detail| T45_usecase_usecase_FragmentPipelineFailureDetail__self
T42_usecase_usecase_DryCorpusRootManifestError_RepoRootCanonicalizeFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T42_usecase_usecase_DryCorpusRootManifestError_ManifestSerializeFailed --o|detail| T42_usecase_usecase_SerializationFailureDetail__self
T42_usecase_usecase_DryCorpusRootManifestError_ManifestSymlinkRejected --o|detail| T31_usecase_usecase_IoFailureDetail__self
T42_usecase_usecase_DryCorpusRootManifestError_ManifestParentCreateFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T42_usecase_usecase_DryCorpusRootManifestError_ManifestWriteFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T40_usecase_usecase_DryWriteDriverInteractor_new --> T40_usecase_usecase_DryWriteDriverInteractor__self
T47_usecase_usecase_EmbeddingModelLoadFailureDetail_new --> T47_usecase_usecase_EmbeddingModelLoadFailureDetail__self
T45_usecase_usecase_FragmentPipelineFailureDetail_new --> T45_usecase_usecase_FragmentPipelineFailureDetail__self
T46_usecase_usecase_SemanticIndexOpenFailureDetail_new --> T46_usecase_usecase_SemanticIndexOpenFailureDetail__self
T42_usecase_usecase_SerializationFailureDetail_new --> T42_usecase_usecase_SerializationFailureDetail__self
R42_usecase_usecase_DryCheckServiceFactoryPort_build --o T45_usecase_usecase_DryCheckServiceFactoryCommand__self
R42_usecase_usecase_DryCheckServiceFactoryPort_build --> T43_usecase_usecase_DryCheckServiceFactoryError__self
R42_usecase_usecase_DryCheckServiceFactoryPort_build --> T44_usecase_usecase_DryCheckServiceFactoryOutput__self
R38_usecase_usecase_DryCorpusFragmentsPort_build --> T39_usecase_usecase_DryCorpusFragmentsError__self
R38_usecase_usecase_DryCorpusFragmentsPort_build --> T40_usecase_usecase_DryCorpusFragmentsOutput__self
R47_usecase_usecase_DryCorpusRootManifestWriterPort_write --> T42_usecase_usecase_DryCorpusRootManifestError__self
R40_usecase_usecase_DryWriteConfigLoaderPort_load --> T41_usecase_usecase_DryCheckConfigLoaderError__self
R40_usecase_usecase_DryWriteConfigLoaderPort_load --> T40_usecase_usecase_DryWriteConfigResolution__self
R37_usecase_usecase_DryWriteDriverService_dry_write --o T35_usecase_usecase_DryWriteDriverInput__self
R37_usecase_usecase_DryWriteDriverService_dry_write --> T31_usecase_usecase_DryWriteOutcome__self
T47_usecase_usecase_DryCheckConfigLoadFailureDetail_new --> T47_usecase_usecase_DryCheckConfigLoadFailureDetail__self
T41_usecase_usecase_DryCheckConfigLoaderError_RepoRootCanonicalizeFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T41_usecase_usecase_DryCheckConfigLoaderError_ConfigPathCanonicalizeFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T41_usecase_usecase_DryCheckConfigLoaderError_ConfigSymlinkRejected --o|detail| T31_usecase_usecase_IoFailureDetail__self
T41_usecase_usecase_DryCheckConfigLoaderError_ConfigLoadFailed --o|detail| T47_usecase_usecase_DryCheckConfigLoadFailureDetail__self
T47_usecase_usecase_FixpointResolveDriverInteractor_new --> T47_usecase_usecase_FixpointResolveDriverInteractor__self
T45_usecase_usecase_FixpointWorkspaceContextError_GitDiscoveryFailed --o|detail| T41_usecase_usecase_GitDiscoveryFailureDetail__self
T45_usecase_usecase_FixpointWorkspaceContextError_RepoRootCanonicalizeFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T45_usecase_usecase_FixpointWorkspaceContextError_ItemsDirSymlinkCheckFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T45_usecase_usecase_FixpointWorkspaceContextError_ProjectRootCanonicalizeFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T45_usecase_usecase_FixpointWorkspaceContextError_MetadataSymlinkRejected --o|detail| T31_usecase_usecase_IoFailureDetail__self
T45_usecase_usecase_FixpointWorkspaceContextError_MetadataReadFailed --o|detail| T31_usecase_usecase_IoFailureDetail__self
T45_usecase_usecase_FixpointWorkspaceContextError_MetadataDecodeFailed --o|detail| T43_usecase_usecase_MetadataDecodeFailureDetail__self
R40_usecase_usecase_DryCheckConfigLoaderPort_load --> T41_usecase_usecase_DryCheckConfigLoaderError__self
R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve --o T42_usecase_usecase_FixpointResolveDriverInput__self
R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve --> T44_usecase_usecase_FixpointResolveDriverOutcome__self
R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context --> T40_usecase_usecase_FixpointWorkspaceContext__self
R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context --> T45_usecase_usecase_FixpointWorkspaceContextError__self
T35_usecase_usecase_DryDriverInteractor__self -.impl.-> R32_usecase_usecase_DryDriverService__self
T40_usecase_usecase_DryWriteDriverInteractor__self -.impl.-> R37_usecase_usecase_DryWriteDriverService__self
T42_usecase_usecase_DryResultsDriverInteractor__self -.impl.-> R39_usecase_usecase_DryResultsDriverService__self
T48_usecase_usecase_DryCheckApprovedDriverInteractor__self -.impl.-> R45_usecase_usecase_DryCheckApprovedDriverService__self
T47_usecase_usecase_FixpointResolveDriverInteractor__self -.impl.-> R44_usecase_usecase_FixpointResolveDriverService__self
T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new --> T55_infrastructure_infrastructure_BranchStrategyConfigError__self
T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new --> T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self
T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new --o T36_domain_domain_BranchStrategySnapshot__self
T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new --> T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self
T52_infrastructure_infrastructure_CodexDryFixLocalRunner_new --> T52_infrastructure_infrastructure_CodexDryFixLocalRunner__self
T52_infrastructure_infrastructure_CodexDryFixLocalRunner_dry_run_fix_local --o T38_usecase_usecase_DryFixLocalDriverInput__self
T46_infrastructure_infrastructure_DryDriverAdapter_new --> T46_infrastructure_infrastructure_DryDriverAdapter__self
T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter_new --> T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self
T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter_new --> T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter__self
T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self --o|merge_method| T49_infrastructure_infrastructure_MergeMethodDocument__self
T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new --> T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self
T45_infrastructure_infrastructure_TrackDocumentV2__self --o|branch_strategy_snapshot| T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self
T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self -.impl.-> R34_usecase_usecase_BranchStrategyPort__self
T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self -.impl.-> R34_usecase_usecase_BranchStrategyPort__self
T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self -.impl.-> R35_usecase_usecase_ReviewGateStatePort__self
T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self -.impl.-> R36_usecase_usecase_DiffBaseResolverPort__self
T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter__self -.impl.-> R38_usecase_usecase_DryApprovalFactoryPort__self
T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter__self -.impl.-> R44_usecase_usecase_FixpointWorkspaceContextPort__self
T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter__self -.impl.-> R40_usecase_usecase_DryCheckConfigLoaderPort__self
T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter__self -.impl.-> R42_usecase_usecase_FixpointDryGateFactoryPort__self
T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter__self -.impl.-> R44_usecase_usecase_FixpointGateStateFactoryPort__self
T50_infrastructure_infrastructure_FsDryRepoRootAdapter__self -.impl.-> R31_usecase_usecase_DryRepoRootPort__self
T52_infrastructure_infrastructure_FsDryBaseBranchAdapter__self -.impl.-> R33_usecase_usecase_DryBaseBranchPort__self
T61_infrastructure_infrastructure_FsDryCheckStorageFactoryAdapter__self -.impl.-> R42_usecase_usecase_DryCheckStorageFactoryPort__self
T57_infrastructure_infrastructure_FsDryDiffBaseFactoryAdapter__self -.impl.-> R38_usecase_usecase_DryDiffBaseFactoryPort__self
T59_infrastructure_infrastructure_FsDryWriteConfigLoaderAdapter__self -.impl.-> R40_usecase_usecase_DryWriteConfigLoaderPort__self
T57_infrastructure_infrastructure_FsDryCorpusFragmentsAdapter__self -.impl.-> R38_usecase_usecase_DryCorpusFragmentsPort__self
T59_infrastructure_infrastructure_DryCheckServiceFactoryAdapter__self -.impl.-> R42_usecase_usecase_DryCheckServiceFactoryPort__self
T60_infrastructure_infrastructure_FsDryCorpusRootManifestAdapter__self -.impl.-> R47_usecase_usecase_DryCorpusRootManifestWriterPort__self
T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter__self -.impl.-> R36_usecase_usecase_DryTierTelemetryPort__self
T46_infrastructure_infrastructure_DryDriverAdapter__self -.impl.-> R29_usecase_usecase_DryDriverPort__self
T31_cli_driver_cli_driver_DryDriver_new --> T31_cli_driver_cli_driver_DryDriver__self
T33_cli_driver_cli_driver_TrackDriver_new --> T33_cli_driver_cli_driver_TrackDriver__self
T33_cli_driver_cli_driver_TrackDriver_handle --o T32_cli_driver_cli_driver_TrackInput__self
T50_cli_composition_cli_composition_DryCompositionRoot_new --> T50_cli_composition_cli_composition_DryCompositionRoot__self
T50_cli_composition_cli_composition_DryCompositionRoot_dry_driver --> T31_cli_driver_cli_driver_DryDriver__self
T50_cli_composition_cli_composition_GitCompositionRoot_new --> T50_cli_composition_cli_composition_GitCompositionRoot__self
T52_cli_composition_cli_composition_TrackCompositionRoot_new --> T52_cli_composition_cli_composition_TrackCompositionRoot__self
T52_cli_composition_cli_composition_TrackCompositionRoot_track_driver --> T33_cli_driver_cli_driver_TrackDriver__self
class T36_domain_domain_BranchStrategySnapshot_new method_node
class T36_domain_domain_BranchStrategySnapshot_base_branch method_node
class T36_domain_domain_BranchStrategySnapshot_merge_target method_node
class T36_domain_domain_BranchStrategySnapshot_merge_method method_node
class T36_domain_domain_BranchStrategySnapshot__self value_object
class T25_domain_domain_MergeMethod_Squash variant_node
class T25_domain_domain_MergeMethod_Merge variant_node
class T25_domain_domain_MergeMethod_Rebase variant_node
class T25_domain_domain_MergeMethod__self value_object
class T27_domain_domain_TrackMetadata_new method_node
class T27_domain_domain_TrackMetadata_with_branch method_node
class T27_domain_domain_TrackMetadata_id method_node
class T27_domain_domain_TrackMetadata_branch method_node
class T27_domain_domain_TrackMetadata_is_activated method_node
class T27_domain_domain_TrackMetadata_set_branch method_node
class T27_domain_domain_TrackMetadata_title method_node
class T27_domain_domain_TrackMetadata_status_override method_node
class T27_domain_domain_TrackMetadata_set_status_override method_node
class T27_domain_domain_TrackMetadata_branch_strategy_snapshot method_node
class T27_domain_domain_TrackMetadata__self entity
class R34_usecase_usecase_BranchStrategyPort_base_branch method_node
class R34_usecase_usecase_BranchStrategyPort_merge_target method_node
class R34_usecase_usecase_BranchStrategyPort_merge_method method_node
class R34_usecase_usecase_BranchStrategyPort_track_prefix method_node
class R34_usecase_usecase_BranchStrategyPort__self secondary_port
class T48_usecase_usecase_DryCheckApprovedDriverInteractor_new method_node
class T48_usecase_usecase_DryCheckApprovedDriverInteractor__self interactor
class R45_usecase_usecase_DryCheckApprovedDriverService_dry_check_approved method_node
class R45_usecase_usecase_DryCheckApprovedDriverService__self app_service
class T43_usecase_usecase_DryCheckApprovedDriverInput__self dto
class T39_usecase_usecase_DryCheckApprovedOutcome_Approved variant_node
class T39_usecase_usecase_DryCheckApprovedOutcome_Blocked variant_node
class T39_usecase_usecase_DryCheckApprovedOutcome_Failure variant_node
class T39_usecase_usecase_DryCheckApprovedOutcome__self dto
class T35_usecase_usecase_DryDriverInteractor__self interactor
class T38_usecase_usecase_DryFixLocalDriverInput__self dto
class T37_usecase_usecase_DryResultsDriverInput__self dto
class T35_usecase_usecase_DryWriteDriverInput__self dto
class T38_usecase_usecase_DryWriteFindingSummary__self dto
class T31_usecase_usecase_DryWriteOutcome_Success variant_node
class T31_usecase_usecase_DryWriteOutcome_Failure variant_node
class T31_usecase_usecase_DryWriteOutcome__self dto
class R29_usecase_usecase_DryDriverPort_dry_fix_local method_node
class R29_usecase_usecase_DryDriverPort__self secondary_port
class R32_usecase_usecase_DryDriverService_dry_fix_local method_node
class R32_usecase_usecase_DryDriverService__self app_service
class T34_usecase_usecase_DryBaseBranchError_MetadataPathOutsideRepo variant_node
class T34_usecase_usecase_DryBaseBranchError_MetadataSymlinkRejected variant_node
class T34_usecase_usecase_DryBaseBranchError_MetadataNotFound variant_node
class T34_usecase_usecase_DryBaseBranchError_MetadataReadFailed variant_node
class T34_usecase_usecase_DryBaseBranchError_MetadataDecodeFailed variant_node
class T34_usecase_usecase_DryBaseBranchError__self error_type
class T37_usecase_usecase_DryCheckStorageHandle__self dto
class T32_usecase_usecase_DryRepoWorkspace__self dto
class T37_usecase_usecase_DryRepoWorkspaceError_GitDiscoveryFailed variant_node
class T37_usecase_usecase_DryRepoWorkspaceError_RepoRootCanonicalizeFailed variant_node
class T37_usecase_usecase_DryRepoWorkspaceError_ItemsDirSymlinkRejected variant_node
class T37_usecase_usecase_DryRepoWorkspaceError_ItemsDirInvalid variant_node
class T37_usecase_usecase_DryRepoWorkspaceError__self error_type
class T41_usecase_usecase_GitDiscoveryFailureDetail_new method_node
class T41_usecase_usecase_GitDiscoveryFailureDetail_as_str method_node
class T41_usecase_usecase_GitDiscoveryFailureDetail__self value_object
class T31_usecase_usecase_IoFailureDetail_new method_node
class T31_usecase_usecase_IoFailureDetail_as_str method_node
class T31_usecase_usecase_IoFailureDetail__self value_object
class T43_usecase_usecase_MetadataDecodeFailureDetail_new method_node
class T43_usecase_usecase_MetadataDecodeFailureDetail_as_str method_node
class T43_usecase_usecase_MetadataDecodeFailureDetail__self value_object
class R33_usecase_usecase_DryBaseBranchPort_resolve_base_branch method_node
class R33_usecase_usecase_DryBaseBranchPort__self secondary_port
class R42_usecase_usecase_DryCheckStorageFactoryPort_build method_node
class R42_usecase_usecase_DryCheckStorageFactoryPort__self secondary_port
class R38_usecase_usecase_DryDiffBaseFactoryPort_build method_node
class R38_usecase_usecase_DryDiffBaseFactoryPort__self secondary_port
class R31_usecase_usecase_DryRepoRootPort_resolve method_node
class R31_usecase_usecase_DryRepoRootPort__self secondary_port
class T42_usecase_usecase_DryResultsDriverInteractor_new method_node
class T42_usecase_usecase_DryResultsDriverInteractor__self interactor
class T33_usecase_usecase_DryResultsOutcome_Success variant_node
class T33_usecase_usecase_DryResultsOutcome_Failure variant_node
class T33_usecase_usecase_DryResultsOutcome__self dto
class T39_usecase_usecase_DryResultsRecordSummary__self dto
class T40_usecase_usecase_DryResultsVerdictSummary_NotAViolation variant_node
class T40_usecase_usecase_DryResultsVerdictSummary_Accepted variant_node
class T40_usecase_usecase_DryResultsVerdictSummary_Violation variant_node
class T40_usecase_usecase_DryResultsVerdictSummary__self dto
class R39_usecase_usecase_DryResultsDriverService_dry_results method_node
class R39_usecase_usecase_DryResultsDriverService__self app_service
class T50_usecase_usecase_AgentConfigResolutionFailureDetail_new method_node
class T50_usecase_usecase_AgentConfigResolutionFailureDetail_as_str method_node
class T50_usecase_usecase_AgentConfigResolutionFailureDetail__self value_object
class T30_usecase_usecase_CapabilityName_try_new method_node
class T30_usecase_usecase_CapabilityName_as_str method_node
class T30_usecase_usecase_CapabilityName__self value_object
class T44_usecase_usecase_DiffHunkListingFailureDetail_new method_node
class T44_usecase_usecase_DiffHunkListingFailureDetail_as_str method_node
class T44_usecase_usecase_DiffHunkListingFailureDetail__self value_object
class T45_usecase_usecase_DryCheckServiceFactoryCommand__self dto
class T43_usecase_usecase_DryCheckServiceFactoryError_EmbeddingModelLoadFailed variant_node
class T43_usecase_usecase_DryCheckServiceFactoryError_SemanticIndexOpenFailed variant_node
class T43_usecase_usecase_DryCheckServiceFactoryError_AgentConfigResolutionFailed variant_node
class T43_usecase_usecase_DryCheckServiceFactoryError__self error_type
class T44_usecase_usecase_DryCheckServiceFactoryOutput__self dto
class T39_usecase_usecase_DryCorpusFragmentsError_WorkspaceRootSymlinkRejected variant_node
class T39_usecase_usecase_DryCorpusFragmentsError_WorkspaceRootInvalid variant_node
class T39_usecase_usecase_DryCorpusFragmentsError_DiffHunkListingFailed variant_node
class T39_usecase_usecase_DryCorpusFragmentsError_FragmentExtractionFailed variant_node
class T39_usecase_usecase_DryCorpusFragmentsError_FragmentPathNormalizationFailed variant_node
class T39_usecase_usecase_DryCorpusFragmentsError__self error_type
class T40_usecase_usecase_DryCorpusFragmentsOutput__self dto
class T42_usecase_usecase_DryCorpusRootManifestError_RepoRootCanonicalizeFailed variant_node
class T42_usecase_usecase_DryCorpusRootManifestError_ManifestPathOutsideRepo variant_node
class T42_usecase_usecase_DryCorpusRootManifestError_ManifestSerializeFailed variant_node
class T42_usecase_usecase_DryCorpusRootManifestError_ManifestSymlinkRejected variant_node
class T42_usecase_usecase_DryCorpusRootManifestError_ManifestParentCreateFailed variant_node
class T42_usecase_usecase_DryCorpusRootManifestError_ManifestWriteFailed variant_node
class T42_usecase_usecase_DryCorpusRootManifestError__self error_type
class T40_usecase_usecase_DryWriteConfigResolution__self dto
class T40_usecase_usecase_DryWriteDriverInteractor_new method_node
class T40_usecase_usecase_DryWriteDriverInteractor__self interactor
class T47_usecase_usecase_EmbeddingModelLoadFailureDetail_new method_node
class T47_usecase_usecase_EmbeddingModelLoadFailureDetail_as_str method_node
class T47_usecase_usecase_EmbeddingModelLoadFailureDetail__self value_object
class T45_usecase_usecase_FragmentPipelineFailureDetail_new method_node
class T45_usecase_usecase_FragmentPipelineFailureDetail_as_str method_node
class T45_usecase_usecase_FragmentPipelineFailureDetail__self value_object
class T46_usecase_usecase_SemanticIndexOpenFailureDetail_new method_node
class T46_usecase_usecase_SemanticIndexOpenFailureDetail_as_str method_node
class T46_usecase_usecase_SemanticIndexOpenFailureDetail__self value_object
class T42_usecase_usecase_SerializationFailureDetail_new method_node
class T42_usecase_usecase_SerializationFailureDetail_as_str method_node
class T42_usecase_usecase_SerializationFailureDetail__self value_object
class R42_usecase_usecase_DryCheckServiceFactoryPort_build method_node
class R42_usecase_usecase_DryCheckServiceFactoryPort__self secondary_port
class R38_usecase_usecase_DryCorpusFragmentsPort_build method_node
class R38_usecase_usecase_DryCorpusFragmentsPort__self secondary_port
class R47_usecase_usecase_DryCorpusRootManifestWriterPort_write method_node
class R47_usecase_usecase_DryCorpusRootManifestWriterPort__self secondary_port
class R36_usecase_usecase_DryTierTelemetryPort_record method_node
class R36_usecase_usecase_DryTierTelemetryPort__self secondary_port
class R40_usecase_usecase_DryWriteConfigLoaderPort_load method_node
class R40_usecase_usecase_DryWriteConfigLoaderPort__self secondary_port
class R37_usecase_usecase_DryWriteDriverService_dry_write method_node
class R37_usecase_usecase_DryWriteDriverService__self app_service
class R36_usecase_usecase_DiffBaseResolverPort_resolve_diff_base method_node
class R36_usecase_usecase_DiffBaseResolverPort__self secondary_port
class R38_usecase_usecase_DryApprovalFactoryPort_build_approval method_node
class R38_usecase_usecase_DryApprovalFactoryPort__self secondary_port
class R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate method_node
class R38_usecase_usecase_FixpointDryGateService__self app_service
class R38_usecase_usecase_RefVerifyGateStatePort_ref_verify_status method_node
class R38_usecase_usecase_RefVerifyGateStatePort__self secondary_port
class R35_usecase_usecase_ReviewGateStatePort_review_status method_node
class R35_usecase_usecase_ReviewGateStatePort__self secondary_port
class T47_usecase_usecase_DryCheckConfigLoadFailureDetail_new method_node
class T47_usecase_usecase_DryCheckConfigLoadFailureDetail_as_str method_node
class T47_usecase_usecase_DryCheckConfigLoadFailureDetail__self value_object
class T41_usecase_usecase_DryCheckConfigLoaderError_RepoRootCanonicalizeFailed variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError_ConfigPathCanonicalizeFailed variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError_ConfigPathOutsideRepo variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError_ConfigSymlinkRejected variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError_ConfigLoadFailed variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError_InvalidKnownBadPercent variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError_InvalidMaxParallelism variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError__self error_type
class T42_usecase_usecase_FixpointResolveDriverInput__self dto
class T47_usecase_usecase_FixpointResolveDriverInteractor_new method_node
class T47_usecase_usecase_FixpointResolveDriverInteractor__self interactor
class T44_usecase_usecase_FixpointResolveDriverOutcome_RunDfp variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_RunRfp variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_RunRefVerify variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_Commit variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_Failure variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome__self dto
class T40_usecase_usecase_FixpointWorkspaceContext__self dto
class T45_usecase_usecase_FixpointWorkspaceContextError_GitDiscoveryFailed variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_RepoRootCanonicalizeFailed variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_ItemsDirSymlinkCheckFailed variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_ItemsDirIsSymlink variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_ItemsDirInvalid variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_ProjectRootPatternInvalid variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_ProjectRootCanonicalizeFailed variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_MetadataNotFound variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_MetadataSymlinkRejected variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_MetadataReadFailed variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError_MetadataDecodeFailed variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError__self error_type
class R40_usecase_usecase_DryCheckConfigLoaderPort_load method_node
class R40_usecase_usecase_DryCheckConfigLoaderPort__self secondary_port
class R42_usecase_usecase_FixpointDryGateFactoryPort_build method_node
class R42_usecase_usecase_FixpointDryGateFactoryPort__self secondary_port
class R44_usecase_usecase_FixpointGateStateFactoryPort_build_review_gate method_node
class R44_usecase_usecase_FixpointGateStateFactoryPort_build_ref_verify_gate method_node
class R44_usecase_usecase_FixpointGateStateFactoryPort__self secondary_port
class R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve method_node
class R44_usecase_usecase_FixpointResolveDriverService__self app_service
class R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context method_node
class R44_usecase_usecase_FixpointWorkspaceContextPort__self secondary_port
class R28_usecase_usecase_TrackService_init method_node
class R28_usecase_usecase_TrackService_transition method_node
class R28_usecase_usecase_TrackService_resolve method_node
class R28_usecase_usecase_TrackService_branch_create method_node
class R28_usecase_usecase_TrackService_branch_switch method_node
class R28_usecase_usecase_TrackService_views_validate method_node
class R28_usecase_usecase_TrackService_views_sync method_node
class R28_usecase_usecase_TrackService_add_task method_node
class R28_usecase_usecase_TrackService_set_override method_node
class R28_usecase_usecase_TrackService_clear_override method_node
class R28_usecase_usecase_TrackService_next_task method_node
class R28_usecase_usecase_TrackService_task_counts method_node
class R28_usecase_usecase_TrackService_archive method_node
class R28_usecase_usecase_TrackService_detect_active method_node
class R28_usecase_usecase_TrackService_switch_base method_node
class R28_usecase_usecase_TrackService__self app_service
class T55_infrastructure_infrastructure_BranchStrategyConfigError_Io variant_node
class T55_infrastructure_infrastructure_BranchStrategyConfigError_Parse variant_node
class T55_infrastructure_infrastructure_BranchStrategyConfigError__self error_type
class T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new method_node
class T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self secondary_adapter
class T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new method_node
class T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self secondary_adapter
class T52_infrastructure_infrastructure_CodexDryFixLocalRunner_new method_node
class T52_infrastructure_infrastructure_CodexDryFixLocalRunner_dry_run_fix_local method_node
class T52_infrastructure_infrastructure_CodexDryFixLocalRunner__self secondary_adapter
class T59_infrastructure_infrastructure_DryCheckServiceFactoryAdapter__self secondary_adapter
class T46_infrastructure_infrastructure_DryDriverAdapter_new method_node
class T46_infrastructure_infrastructure_DryDriverAdapter__self secondary_adapter
class T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter_new method_node
class T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self secondary_adapter
class T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter__self secondary_adapter
class T52_infrastructure_infrastructure_FsDryBaseBranchAdapter__self secondary_adapter
class T61_infrastructure_infrastructure_FsDryCheckStorageFactoryAdapter__self secondary_adapter
class T57_infrastructure_infrastructure_FsDryCorpusFragmentsAdapter__self secondary_adapter
class T60_infrastructure_infrastructure_FsDryCorpusRootManifestAdapter__self secondary_adapter
class T57_infrastructure_infrastructure_FsDryDiffBaseFactoryAdapter__self secondary_adapter
class T50_infrastructure_infrastructure_FsDryRepoRootAdapter__self secondary_adapter
class T59_infrastructure_infrastructure_FsDryWriteConfigLoaderAdapter__self secondary_adapter
class T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter_new method_node
class T62_infrastructure_infrastructure_RecordingDryTierTelemetryAdapter__self secondary_adapter
class T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self dto
class T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter__self secondary_adapter
class T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter__self secondary_adapter
class T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter__self secondary_adapter
class T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter__self secondary_adapter
class T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new method_node
class T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self secondary_adapter
class T49_infrastructure_infrastructure_MergeMethodDocument_Squash variant_node
class T49_infrastructure_infrastructure_MergeMethodDocument_Merge variant_node
class T49_infrastructure_infrastructure_MergeMethodDocument_Rebase variant_node
class T49_infrastructure_infrastructure_MergeMethodDocument__self dto
class T45_infrastructure_infrastructure_TrackDocumentV2__self dto
class T31_cli_driver_cli_driver_DryDriver_new method_node
class T31_cli_driver_cli_driver_DryDriver_handle method_node
class T29_cli_driver_cli_driver_PrInput_Push variant_node
class T29_cli_driver_cli_driver_PrInput_Ensure variant_node
class T29_cli_driver_cli_driver_PrInput_Status variant_node
class T29_cli_driver_cli_driver_PrInput_WaitAndMerge variant_node
class T29_cli_driver_cli_driver_PrInput_TriggerReview variant_node
class T29_cli_driver_cli_driver_PrInput_PollReview variant_node
class T29_cli_driver_cli_driver_PrInput_ReviewCycle variant_node
class T29_cli_driver_cli_driver_PrInput__self dto
class T33_cli_driver_cli_driver_TrackDriver_new method_node
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
class T32_cli_driver_cli_driver_TrackInput_SwitchBase variant_node
class T32_cli_driver_cli_driver_TrackInput_FixpointResolve variant_node
class T32_cli_driver_cli_driver_TrackInput__self dto
class T50_cli_composition_cli_composition_DryCompositionRoot_new method_node
class T50_cli_composition_cli_composition_DryCompositionRoot_dry_driver method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_new method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_add_all method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_add_from_file method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_commit_from_file method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_note_from_file method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull_in method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_unstage method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_current_branch_track_id_strict method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_driver method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_new method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_driver method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_detect_active_track_from_branch method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_commit_hash method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_signals method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_graph method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_graph method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_contract_map method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_spec_signals method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_spec_element_hash method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_capture method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_lint method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_impl_signals method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_for_write method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root_for_write method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_validate_id method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_project_root method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_init method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_transition method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_create method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_switch method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_validate method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_sync method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_archive method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_switch_base method_node
class T20_cli_cli_EnsurePrArgs__self dto
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
class T20_cli_cli_TrackCommand_Lint variant_node
class T20_cli_cli_TrackCommand_CatalogueImplSignals variant_node
class T20_cli_cli_TrackCommand_FixpointResolve variant_node
class T20_cli_cli_TrackCommand_SetCommitHash variant_node
class T20_cli_cli_TrackCommand_SwitchBase variant_node
class T20_cli_cli_TrackCommand_items_dir method_node
class T20_cli_cli_TrackCommand__self dto
class T24_cli_cli_WaitAndMergeArgs__self dto
class F54_cli_cli_cli__commands__dry__execute_dry_check_approved free_function
class F54_cli_cli_cli__commands__dry__execute_dry_check_approved function_node
```
